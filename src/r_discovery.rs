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

