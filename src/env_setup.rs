use crate::{cli::Cli, r_discovery::RInstall};
use std::{env, fs, process::Command};

pub fn apply(cli: &Cli, r: &RInstall) -> anyhow::Result<()> {
    set_env("ORCHARD_VERSION", env!("CARGO_PKG_VERSION"));
    set_env("ORCHARD_COMMAND_ARGS", cli.command_args_env());
    set_env("R_HOME", &r.home);

    if let Some(binary) = &cli.r_binary {
        set_env("R_BINARY", binary);
    }
    if cli.no_environ {
        set_env("R_ENVIRON", "");
        set_env("R_ENVIRON_USER", "");
    }
    if cli.no_site_file {
        set_env("R_PROFILE", "");
    }
    if cli.no_init_file {
        set_env("R_PROFILE_USER", "");
    }
    if cli.local_history && !std::path::Path::new(".orchard_history").exists() {
        fs::File::create(".orchard_history")?;
    }

    let (doc, include, share) = r_home_dirs(r);
    set_env("R_DOC_DIR", doc);
    set_env("R_INCLUDE_DIR", include);
    set_env("R_SHARE_DIR", share);
    Ok(())
}

fn r_home_dirs(r: &RInstall) -> (String, String, String) {
    let doc = r.home_path("doc");
    let include = r.home_path("include");
    let share = r.home_path("share");
    if doc.is_dir() && include.is_dir() && share.is_dir() {
        return (
            doc.display().to_string(),
            include.display().to_string(),
            share.display().to_string(),
        );
    }

    let fallback = Command::new(&r.binary)
        .args([
            "--no-echo",
            "--vanilla",
            "-e",
            "cat(paste(R.home('doc'), R.home('include'), R.home('share'), sep=':'))",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string());

    if let Some(paths) = fallback {
        let mut parts = paths.trim().split(':');
        if let (Some(doc), Some(include), Some(share)) = (parts.next(), parts.next(), parts.next())
        {
            return (doc.to_string(), include.to_string(), share.to_string());
        }
    }

    (
        doc.display().to_string(),
        include.display().to_string(),
        share.display().to_string(),
    )
}

fn set_env(key: &str, value: impl AsRef<std::ffi::OsStr>) {
    // ponytail: single-threaded startup only; switch to command-local env if setup becomes concurrent.
    unsafe { env::set_var(key, value) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;
    use std::{
        path::PathBuf,
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    static _ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn command_args_are_composed() {
        let cli = Cli::parse_from(["orchard", "--quiet", "--vanilla"]).expanded();
        assert!(cli.command_args_env().contains("--quiet"));
        assert!(cli.command_args_env().contains("--no-history"));
    }

    #[test]
    fn home_dirs_fall_back_to_paths() {
        let r = RInstall {
            home: PathBuf::from("/no/such/r"),
            binary: PathBuf::from("/no/such/R"),
        };
        assert_eq!(r_home_dirs(&r).0, "/no/such/r/doc");
    }

    #[test]
    fn apply_sets_phase1_environment_and_local_history() {
        let _guard = _ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let keys = [
            "ORCHARD_VERSION",
            "ORCHARD_COMMAND_ARGS",
            "R_HOME",
            "R_BINARY",
            "R_DOC_DIR",
            "R_INCLUDE_DIR",
            "R_SHARE_DIR",
            "R_ENVIRON",
            "R_ENVIRON_USER",
            "R_PROFILE",
            "R_PROFILE_USER",
        ];
        let saved_env: Vec<_> = keys.iter().map(|key| (*key, env::var_os(key))).collect();
        let saved_dir = env::current_dir().unwrap();
        let test_dir = unique_temp_dir("env-setup");

        fs::create_dir_all(test_dir.join("R/doc")).unwrap();
        fs::create_dir_all(test_dir.join("R/include")).unwrap();
        fs::create_dir_all(test_dir.join("R/share")).unwrap();
        env::set_current_dir(&test_dir).unwrap();

        let cli = Cli::parse_from([
            "orchard",
            "--r-binary",
            "/custom/R",
            "--quiet",
            "--no-environ",
            "--no-site-file",
            "--no-init-file",
            "--local-history",
        ])
        .expanded();
        let r = RInstall {
            home: test_dir.join("R"),
            binary: PathBuf::from("/ignored/R"),
        };

        apply(&cli, &r).unwrap();

        assert_eq!(
            env::var("ORCHARD_VERSION").unwrap(),
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(
            env::var("ORCHARD_COMMAND_ARGS").unwrap(),
            "--quiet --no-environ --no-site-file --no-init-file --local-history"
        );
        assert_eq!(env::var_os("R_HOME"), Some(r.home.clone().into_os_string()));
        assert_eq!(env::var_os("R_BINARY"), Some("/custom/R".into()));
        assert_eq!(
            env::var_os("R_DOC_DIR"),
            Some(r.home.join("doc").into_os_string())
        );
        assert_eq!(
            env::var_os("R_INCLUDE_DIR"),
            Some(r.home.join("include").into_os_string())
        );
        assert_eq!(
            env::var_os("R_SHARE_DIR"),
            Some(r.home.join("share").into_os_string())
        );
        assert_eq!(env::var("R_ENVIRON").unwrap(), "");
        assert_eq!(env::var("R_ENVIRON_USER").unwrap(), "");
        assert_eq!(env::var("R_PROFILE").unwrap(), "");
        assert_eq!(env::var("R_PROFILE_USER").unwrap(), "");
        assert!(test_dir.join(".orchard_history").is_file());

        env::set_current_dir(saved_dir).unwrap();
        for (key, value) in saved_env {
            match value {
                Some(value) => unsafe { env::set_var(key, value) },
                None => unsafe { env::remove_var(key) },
            }
        }
        fs::remove_dir_all(test_dir).unwrap();
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("orchard-{name}-{nanos}"))
    }
}
