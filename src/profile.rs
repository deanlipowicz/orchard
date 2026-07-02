use crate::{cli::Cli, r_runtime::RRuntime};
use anyhow::Context;
use std::path::{Path, PathBuf};

pub fn source_profiles(runtime: &mut RRuntime, cli: &Cli) -> anyhow::Result<Vec<PathBuf>> {
    let paths = profile_paths(cli);
    for path in &paths {
        if path.exists() {
            runtime
                .source_file(path)
                .with_context(|| format!("failed to source {}", path.display()))?;
        }
    }
    Ok(paths.into_iter().filter(|p| p.exists()).collect())
}

pub fn source_commands(cli: &Cli) -> Vec<String> {
    profile_paths(cli)
        .into_iter()
        .filter(|p| p.exists())
        .map(|p| {
            format!(
                "base::source({}, local = base::new.env())\n",
                r_string(&p.display().to_string())
            )
        })
        .collect()
}

pub fn profile_paths(cli: &Cli) -> Vec<PathBuf> {
    if let Some(path) = &cli.profile {
        return vec![expand_home(path)];
    }

    let mut paths = Vec::new();
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        paths.push(PathBuf::from(xdg).join("orchard").join("profile"));
    } else if cfg!(windows) {
        paths.push(home().join("orchard").join("profile"));
    } else {
        paths.push(home().join(".config").join("orchard").join("profile"));
    }

    let global = home().join(".orchard_profile");
    paths.push(global.clone());

    let local = PathBuf::from(".orchard_profile");
    if local != global {
        paths.push(local);
    }
    paths
}

fn expand_home(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s == "~" {
        return home();
    }
    if let Some(rest) = s.strip_prefix("~/") {
        return home().join(rest);
    }
    path.to_path_buf()
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn r_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn explicit_profile_is_the_only_path() {
        let cli = Cli::parse_from(["orchard", "--profile", "~/custom.R"]);
        assert_eq!(profile_paths(&cli).len(), 1);
        assert!(profile_paths(&cli)[0].ends_with("custom.R"));
    }
}

