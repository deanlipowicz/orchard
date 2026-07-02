use anyhow::{Context, bail};
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug)]
pub struct RInstall {
    pub home: PathBuf,
    pub binary: PathBuf,
}

pub fn discover(r_binary: Option<&Path>) -> anyhow::Result<RInstall> {
    if let Some(binary) = r_binary {
        let home = r_home_from_binary(binary)?;
        return Ok(RInstall {
            home,
            binary: binary.to_path_buf(),
        });
    }

    if let Ok(home) = env::var("R_HOME") {
        let home = PathBuf::from(home);
        if home.exists() {
            return Ok(RInstall {
                binary: home.join("bin/R"),
                home,
            });
        }
    }

    if let Ok(binary) = env::var("R_BINARY") {
        let binary = PathBuf::from(binary);
        let home = r_home_from_binary(&binary)?;
        return Ok(RInstall { home, binary });
    }

    let binary = PathBuf::from("R");
    let home = r_home_from_binary(&binary)?;
    Ok(RInstall { home, binary })
}

fn r_home_from_binary(binary: &Path) -> anyhow::Result<PathBuf> {
    let output = Command::new(binary)
        .arg("RHOME")
        .output()
        .with_context(|| format!("failed to run {}", binary.display()))?;
    if !output.status.success() {
        bail!("{} RHOME failed", binary.display());
    }
    let home = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if home.is_empty() {
        bail!("{} RHOME returned an empty path", binary.display());
    }
    Ok(PathBuf::from(home))
}

impl RInstall {
    pub fn version(&self) -> anyhow::Result<String> {
        let output = Command::new(&self.binary).arg("--version").output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().next().unwrap_or("NA").to_string())
    }

    pub fn home_path(&self, name: &str) -> PathBuf {
        self.home.join(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes tests that mutate process-wide env vars (`R_HOME`, `R_BINARY`).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const ENV_VARS: &[&str] = &["R_HOME", "R_BINARY"];

    struct EnvGuard {
        saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn new() -> Self {
            let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let saved = ENV_VARS.iter().map(|&k| (k, env::var_os(k))).collect();
            for &k in ENV_VARS {
                unsafe { env::remove_var(k) };
            }
            Self { saved, _lock: lock }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.saved {
                match value {
                    Some(v) => unsafe { env::set_var(key, v) },
                    None => unsafe { env::remove_var(key) },
                }
            }
        }
    }

    /// Create a fake `R` executable in a temp dir that prints `fake_home`
    /// when called with `RHOME`, and prints a version string for `--version`.
    fn make_fake_r_binary(fake_home: &Path) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "orchard-r-discovery-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("R");
        let home_str = fake_home.to_str().unwrap();
        let content = format!(
            "#!/bin/sh\n\
             if [ \"$1\" = \"RHOME\" ]; then\n\
             \x20   echo '{home_str}'\n\
             elif [ \"$1\" = \"--version\" ]; then\n\
             \x20   echo 'R version 99.9.9 (fake)'\n\
             else\n\
             \x20   exit 1\n\
             fi\n"
        );
        std::fs::write(&script, content).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        script
    }

    /// Create a fake `R` executable that exits non-zero for `RHOME`.
    fn make_failing_r_binary() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "orchard-r-discovery-fail-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("R");
        std::fs::write(&script, "#!/bin/sh\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        script
    }

    /// Create a fake `R` executable that prints an empty string for `RHOME`.
    fn make_empty_r_binary() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "orchard-r-discovery-empty-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("R");
        std::fs::write(&script, "#!/bin/sh\necho ''\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        script
    }

    #[test]
    fn discover_with_explicit_binary_resolves_home() {
        let _env = EnvGuard::new();
        let fake_home = std::env::temp_dir().join("orchard-fake-r-home-explicit");
        std::fs::create_dir_all(&fake_home).unwrap();
        let binary = make_fake_r_binary(&fake_home);

        let install = discover(Some(&binary)).unwrap();
        assert_eq!(install.binary, binary);
        assert_eq!(install.home, fake_home);

        std::fs::remove_dir_all(fake_home).ok();
        std::fs::remove_file(&binary).ok();
    }

    #[test]
    fn discover_with_r_home_env_uses_it_directly() {
        let _env = EnvGuard::new();
        let fake_home = std::env::temp_dir().join("orchard-fake-r-home-env");
        std::fs::create_dir_all(&fake_home).unwrap();
        unsafe { env::set_var("R_HOME", &fake_home) };

        let install = discover(None).unwrap();
        assert_eq!(install.home, fake_home);
        assert_eq!(install.binary, fake_home.join("bin/R"));

        std::fs::remove_dir_all(&fake_home).ok();
    }

    #[test]
    fn discover_with_r_home_env_ignores_nonexistent_path() {
        let _env = EnvGuard::new();
        unsafe { env::set_var("R_HOME", "/nonexistent/orchard/path/xyz") };
        // Point R_BINARY at a nonexistent path too, so discover can't fall
        // through to a real R on PATH.
        unsafe { env::set_var("R_BINARY", "/nonexistent/orchard/binary/R") };

        // R_HOME doesn't exist, R_BINARY doesn't exist, so discover should error.
        let result = discover(None);
        assert!(
            result.is_err(),
            "should error when R_HOME is nonexistent and no R binary is available"
        );

        unsafe { env::remove_var("R_HOME") };
        unsafe { env::remove_var("R_BINARY") };
    }

    #[test]
    fn discover_with_r_binary_env_resolves_home() {
        let _env = EnvGuard::new();
        let fake_home = std::env::temp_dir().join("orchard-fake-r-binary-env");
        std::fs::create_dir_all(&fake_home).unwrap();
        let binary = make_fake_r_binary(&fake_home);
        unsafe { env::set_var("R_BINARY", &binary) };

        let install = discover(None).unwrap();
        assert_eq!(install.binary, binary);
        assert_eq!(install.home, fake_home);

        std::fs::remove_dir_all(&fake_home).ok();
        std::fs::remove_file(&binary).ok();
    }

    #[test]
    fn discover_explicit_binary_takes_precedence_over_env() {
        let _env = EnvGuard::new();
        let env_home = std::env::temp_dir().join("orchard-fake-r-home-prec-env");
        let explicit_home = std::env::temp_dir().join("orchard-fake-r-home-prec-explicit");
        std::fs::create_dir_all(&env_home).unwrap();
        std::fs::create_dir_all(&explicit_home).unwrap();
        unsafe { env::set_var("R_HOME", &env_home) };

        let binary = make_fake_r_binary(&explicit_home);
        let install = discover(Some(&binary)).unwrap();
        // Explicit binary arg wins over R_HOME
        assert_eq!(install.home, explicit_home);
        assert_eq!(install.binary, binary);

        std::fs::remove_dir_all(&env_home).ok();
        std::fs::remove_dir_all(&explicit_home).ok();
        std::fs::remove_file(&binary).ok();
    }

    #[test]
    fn discover_fails_when_binary_rhome_exits_nonzero() {
        let _env = EnvGuard::new();
        let binary = make_failing_r_binary();
        let result = discover(Some(&binary));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("RHOME failed"),
            "error should mention RHOME failure: {msg}"
        );
        std::fs::remove_file(&binary).ok();
    }

    #[test]
    fn discover_fails_when_binary_rhome_returns_empty() {
        let _env = EnvGuard::new();
        let binary = make_empty_r_binary();
        let result = discover(Some(&binary));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("empty path"),
            "error should mention empty path: {msg}"
        );
        std::fs::remove_file(&binary).ok();
    }

    #[test]
    fn discover_fails_when_binary_does_not_exist() {
        let _env = EnvGuard::new();
        let result = discover(Some(Path::new("/nonexistent/orchard/binary/R")));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("failed to run"),
            "error should mention failure to run: {msg}"
        );
    }

    #[test]
    fn version_returns_first_line_of_output() {
        let _env = EnvGuard::new();
        let fake_home = std::env::temp_dir().join("orchard-fake-r-version-home");
        std::fs::create_dir_all(&fake_home).unwrap();
        let binary = make_fake_r_binary(&fake_home);

        let install = RInstall {
            home: fake_home.clone(),
            binary: binary.clone(),
        };
        let version = install.version().unwrap();
        assert_eq!(version, "R version 99.9.9 (fake)");

        std::fs::remove_dir_all(&fake_home).ok();
        std::fs::remove_file(&binary).ok();
    }

    #[test]
    fn version_returns_na_for_empty_output() {
        let _env = EnvGuard::new();
        // Create a binary that prints nothing for --version
        let dir = std::env::temp_dir().join(format!(
            "orchard-r-version-empty-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("R");
        std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let install = RInstall {
            home: dir.clone(),
            binary: script.clone(),
        };
        let version = install.version().unwrap();
        assert_eq!(version, "NA");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn home_path_joins_subpath() {
        let install = RInstall {
            home: PathBuf::from("/fake/R/home"),
            binary: PathBuf::from("/fake/R/home/bin/R"),
        };
        assert_eq!(install.home_path("doc"), PathBuf::from("/fake/R/home/doc"));
        assert_eq!(
            install.home_path("lib/libRblas.dylib"),
            PathBuf::from("/fake/R/home/lib/libRblas.dylib")
        );
        assert_eq!(install.home_path(""), PathBuf::from("/fake/R/home"));
    }
}
