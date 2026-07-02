use anyhow::Context;
use std::{
    env,
    os::unix::process::CommandExt,
    path::Path,
    process::Command,
    sync::{Mutex, OnceLock},
};

pub fn repair_and_reexec_if_needed(r_home: &Path) -> anyhow::Result<()> {
    if env::var_os("ORCHARD_LD_REEXEC").is_some() {
        return Ok(());
    }

    reset_dyld_insert_blas_dylib();

    let lib_path = r_home.join("lib");
    let lib_path = lib_path.display().to_string();
    let current = env::var("R_LD_LIBRARY_PATH").unwrap_or_default();
    if current.split(':').any(|p| p == lib_path) {
        return Ok(());
    }

    let r_ld = compute_r_ld_library_path(r_home);
    set_env("R_LD_LIBRARY_PATH", &r_ld);

    let loader_var = if cfg!(target_os = "macos") {
        "DYLD_FALLBACK_LIBRARY_PATH"
    } else {
        "LD_LIBRARY_PATH"
    };
    let loader = prepend_loader_path(&r_ld, env::var(loader_var).ok().as_deref());
    set_env(loader_var, loader);
    set_dyld_insert_blas_dylib(r_home);
    set_env("ORCHARD_LD_REEXEC", "1");

    let exe = env::current_exe().context("failed to locate current executable")?;
    let err = Command::new(exe).args(env::args_os().skip(1)).exec();
    Err(err).context("failed to re-exec after setting R loader paths")
}

fn compute_r_ld_library_path(r_home: &Path) -> String {
    let lib_path = r_home.join("lib").display().to_string();
    let ldpaths = r_home.join("etc/ldpaths");
    let mut value = if ldpaths.exists() {
        Command::new("sh")
            .arg("-c")
            .arg(format!(
                ". '{}'; printf %s \"$R_LD_LIBRARY_PATH\"",
                ldpaths.display()
            ))
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| lib_path.clone())
    } else {
        env::var("R_LD_LIBRARY_PATH").unwrap_or_else(|_| lib_path.clone())
    };
    if !value.split(':').any(|p| p == lib_path) {
        value = format!("{lib_path}:{value}");
    }
    value
}

fn prepend_loader_path(r_ld: &str, existing: Option<&str>) -> String {
    match existing {
        Some(existing) if !existing.is_empty() => format!("{r_ld}:{existing}"),
        _ => r_ld.to_string(),
    }
}

#[cfg(any(test, target_os = "macos"))]
fn remove_path_entry(value: &str, remove: &str) -> String {
    value
        .split(':')
        .filter(|entry| *entry != remove)
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(any(test, target_os = "macos"))]
fn cleanup_dyld_insert_value(
    current: Option<&str>,
    inserted: Option<&str>,
) -> (Option<String>, bool) {
    let Some(inserted) = inserted else {
        return (current.map(str::to_string), false);
    };
    let cleaned = current.map(|value| remove_path_entry(value, inserted));
    (cleaned.filter(|value| !value.is_empty()), true)
}

#[cfg(target_os = "macos")]
fn reset_dyld_insert_blas_dylib() {
    let current = env::var("DYLD_INSERT_LIBRARIES").ok();
    let inserted = env::var("R_DYLD_INSERT_LIBRARIES").ok();
    let (cleaned, clear_marker) =
        cleanup_dyld_insert_value(current.as_deref(), inserted.as_deref());
    if clear_marker {
        if let Some(cleaned) = cleaned {
            set_env("DYLD_INSERT_LIBRARIES", cleaned);
        } else {
            remove_env("DYLD_INSERT_LIBRARIES");
        }
        remove_env("R_DYLD_INSERT_LIBRARIES");
    }
}

#[cfg(not(target_os = "macos"))]
fn reset_dyld_insert_blas_dylib() {}

#[cfg(target_os = "macos")]
fn set_dyld_insert_blas_dylib(r_home: &Path) {
    let Some(blas) = blas_dylib_path_if_exists(r_home) else {
        return;
    };

    let injected = blas.display().to_string();
    let value =
        append_dyld_insert_library(env::var("DYLD_INSERT_LIBRARIES").ok().as_deref(), &injected);
    set_env("DYLD_INSERT_LIBRARIES", value);
    set_env("R_DYLD_INSERT_LIBRARIES", injected);
}

#[cfg(not(target_os = "macos"))]
fn set_dyld_insert_blas_dylib(_r_home: &Path) {}

#[cfg(any(test, target_os = "macos"))]
fn blas_dylib_path_if_exists(r_home: &Path) -> Option<std::path::PathBuf> {
    let blas = r_home.join("lib/libRBlas.dylib");
    blas.exists().then_some(blas)
}

#[cfg(any(test, target_os = "macos"))]
fn append_dyld_insert_library(existing: Option<&str>, injected: &str) -> String {
    match existing {
        Some(existing) if !existing.is_empty() => format!("{existing}:{injected}"),
        _ => injected.to_string(),
    }
}

/// Global mutex for all environment variable mutations.
/// Avoids data races when tests manipulate env vars in parallel.
pub(crate) static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn set_env(key: &str, value: impl AsRef<std::ffi::OsStr>) {
    let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    unsafe { env::set_var(key, value) }
}

#[cfg(any(test, target_os = "macos"))]
fn remove_env(key: &str) {
    let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    unsafe { env::remove_var(key) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf, time::SystemTime};

    const ENV_VARS: &[&str] = &[
        "R_LD_LIBRARY_PATH",
        "LD_LIBRARY_PATH",
        "DYLD_FALLBACK_LIBRARY_PATH",
        "DYLD_INSERT_LIBRARIES",
        "R_DYLD_INSERT_LIBRARIES",
        "ORCHARD_LD_REEXEC",
    ];

    struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);

    impl EnvGuard {
        fn new() -> Self {
            Self(
                ENV_VARS
                    .iter()
                    .map(|key| (*key, env::var_os(key)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                match value {
                    Some(value) => set_env(key, value),
                    None => remove_env(key),
                }
            }
        }
    }

    #[test]
    fn includes_r_lib_path() {
        let _env = EnvGuard::new();
        remove_env("R_LD_LIBRARY_PATH");
        let value = compute_r_ld_library_path(Path::new("/tmp/example-r"));
        assert!(value.split(':').any(|p| p == "/tmp/example-r/lib"));
    }

    #[test]
    fn prepends_r_paths_to_existing_loader_paths() {
        assert_eq!(
            prepend_loader_path("/r/lib:/r/extra", Some("/usr/lib")),
            "/r/lib:/r/extra:/usr/lib"
        );
        assert_eq!(
            prepend_loader_path("/r/lib:/r/extra", Some("")),
            "/r/lib:/r/extra"
        );
    }

    #[test]
    fn dyld_cleanup_removes_prior_inserted_entry() {
        assert_eq!(
            cleanup_dyld_insert_value(
                Some("/keep/one:/r/lib/libRBlas.dylib:/keep/two"),
                Some("/r/lib/libRBlas.dylib")
            ),
            (Some("/keep/one:/keep/two".to_string()), true)
        );
        assert_eq!(
            cleanup_dyld_insert_value(Some("/r/lib/libRBlas.dylib"), Some("/r/lib/libRBlas.dylib")),
            (None, true)
        );
    }

    #[test]
    fn blas_injection_is_skipped_when_dylib_is_absent() {
        let _env = EnvGuard::new();
        let r_home = temp_r_home();
        fs::create_dir_all(r_home.join("lib")).unwrap();
        remove_env("DYLD_INSERT_LIBRARIES");
        remove_env("R_DYLD_INSERT_LIBRARIES");

        assert_eq!(blas_dylib_path_if_exists(&r_home), None);
        set_dyld_insert_blas_dylib(&r_home);

        assert_eq!(env::var_os("DYLD_INSERT_LIBRARIES"), None);
        assert_eq!(env::var_os("R_DYLD_INSERT_LIBRARIES"), None);
        fs::remove_dir_all(r_home).unwrap();
    }

    #[test]
    fn blas_injection_appends_existing_insert_libraries() {
        let _env = EnvGuard::new();
        let r_home = temp_r_home();
        fs::create_dir_all(r_home.join("lib")).unwrap();
        let blas = r_home.join("lib/libRBlas.dylib");
        fs::write(&blas, "").unwrap();

        assert_eq!(blas_dylib_path_if_exists(&r_home), Some(blas.clone()));
        assert_eq!(
            append_dyld_insert_library(Some("/already.dylib"), blas.to_str().unwrap()),
            format!("/already.dylib:{}", blas.display())
        );
        assert_eq!(
            append_dyld_insert_library(None, blas.to_str().unwrap()),
            blas.display().to_string()
        );
        fs::remove_dir_all(r_home).unwrap();
    }

    fn temp_r_home() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("orchard-dyld-test-{nanos}"))
    }
}

