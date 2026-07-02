use anyhow::Context;
use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    process::Command,
    sync::{Mutex, OnceLock},
};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn run_command(command: &str) {
    if let Err(err) = run(command) {
        println!("{err}");
    }
}

fn run(command: &str) -> anyhow::Result<()> {
    if command.trim().is_empty() {
        println!();
        return Ok(());
    }

    let parts = split(command)?;
    if parts.first().is_some_and(|cmd| cmd == "cd") {
        return cd(&parts);
    }

    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    Command::new(shell)
        .arg("-c")
        .arg(command)
        .status()
        .context("failed to run shell command")?;
    Ok(())
}

fn split(command: &str) -> anyhow::Result<Vec<String>> {
    shell_words::split(command).map_err(Into::into)
}

fn cd(parts: &[String]) -> anyhow::Result<()> {
    if parts.len() != 2 {
        println!("cd method takes one argument\n");
        return Ok(());
    }

    let old = env::current_dir()?;
    let target = if parts[1] == "-" {
        env::var_os("OLDPWD")
            .map(PathBuf::from)
            .unwrap_or_else(|| old.clone())
    } else {
        PathBuf::from(crate::util::expand_vars(&crate::util::expand_tilde(
            &parts[1],
        )))
    };
    env::set_current_dir(&target).with_context(|| target.display().to_string())?;
    {
        // Safety: ENV_LOCK is only poisoned if a previous env mutation
        // panicked, which would abort anyway.  Unwrap is acceptable.
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        unsafe { env::set_var("OLDPWD", old) };
    }
    println!("{}", env::current_dir()?.display());
    io::stdout().flush().ok();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn expands_env_vars() {
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        unsafe { env::set_var("ORCHARD_TEST_DIR", "/tmp/orchard-test") };
        assert_eq!(
            crate::util::expand_vars("$ORCHARD_TEST_DIR/x"),
            "/tmp/orchard-test/x"
        );
        assert_eq!(
            crate::util::expand_vars("${ORCHARD_TEST_DIR}/x"),
            "/tmp/orchard-test/x"
        );
    }

    #[test]
    fn cd_and_cd_dash_work() {
        let start = env::current_dir().unwrap();
        let dir = env::temp_dir().join(format!(
            "orchard-shell-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();

        cd(&["cd".into(), dir.display().to_string()]).unwrap();
        assert_eq!(env::current_dir().unwrap(), dir);
        cd(&["cd".into(), "-".into()]).unwrap();
        assert_eq!(env::current_dir().unwrap(), start);
    }

    #[test]
    fn bad_cd_is_not_an_error() {
        cd(&["cd".into()]).unwrap();
    }
}

pub(crate) fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}
