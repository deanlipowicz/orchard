use clap::Parser;
use std::path::PathBuf;

#[derive(Clone, Debug, Parser)]
#[command(name = "orchard", disable_help_subcommand = true)]
pub struct Cli {
    #[arg(short = 'v', long)]
    pub version: bool,
    #[arg(long = "r-binary")]
    pub r_binary: Option<PathBuf>,
    #[arg(long)]
    pub profile: Option<PathBuf>,
    #[arg(short = 'q', long, alias = "silent")]
    pub quiet: bool,
    #[arg(long = "no-environ")]
    pub no_environ: bool,
    #[arg(long = "no-site-file")]
    pub no_site_file: bool,
    #[arg(long = "no-init-file")]
    pub no_init_file: bool,
    #[arg(long = "local-history")]
    pub local_history: bool,
    #[arg(long = "global-history")]
    pub global_history: bool,
    #[arg(long = "no-history")]
    pub no_history: bool,
    #[arg(long)]
    pub vanilla: bool,
    #[arg(long)]
    pub save: bool,
    #[arg(long = "ask-save")]
    pub ask_save: bool,
    #[arg(long = "restore-data")]
    pub restore_data: bool,
    #[arg(long)]
    pub debug: bool,
    #[arg(long, hide = true)]
    pub coverage: bool,
    #[arg(long, hide = true)]
    pub cprofile: bool,
    #[arg(long = "no-save", hide = true)]
    pub no_save: bool,
    #[arg(long = "no-restore-data", hide = true)]
    pub no_restore_data: bool,
    #[arg(long = "no-restore-history", hide = true)]
    pub no_restore_history: bool,
    #[arg(long = "no-restore", hide = true)]
    pub no_restore: bool,
    #[arg(long = "no-readline", hide = true)]
    pub no_readline: bool,
    #[arg(long, hide = true)]
    pub interactive: bool,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    pub fn expanded(mut self) -> Self {
        if self.vanilla {
            self.no_history = true;
            self.no_environ = true;
            self.no_site_file = true;
            self.no_init_file = true;
        }
        self
    }

    pub fn command_args_env(&self) -> String {
        let mut args = Vec::new();
        macro_rules! flag {
            ($field:ident, $name:literal) => {
                if self.$field {
                    args.push($name);
                }
            };
        }
        flag!(version, "--version");
        flag!(quiet, "--quiet");
        flag!(no_environ, "--no-environ");
        flag!(no_site_file, "--no-site-file");
        flag!(no_init_file, "--no-init-file");
        flag!(local_history, "--local-history");
        flag!(global_history, "--global-history");
        flag!(no_history, "--no-history");
        flag!(vanilla, "--vanilla");
        flag!(save, "--save");
        flag!(ask_save, "--ask-save");
        flag!(restore_data, "--restore-data");
        flag!(debug, "--debug");
        args.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn phase1_flags_parse() {
        let cases: &[(&str, fn(&Cli) -> bool)] = &[
            ("--version", |cli| cli.version),
            ("--quiet", |cli| cli.quiet),
            ("--silent", |cli| cli.quiet),
            ("--no-environ", |cli| cli.no_environ),
            ("--no-site-file", |cli| cli.no_site_file),
            ("--no-init-file", |cli| cli.no_init_file),
            ("--local-history", |cli| cli.local_history),
            ("--global-history", |cli| cli.global_history),
            ("--no-history", |cli| cli.no_history),
            ("--vanilla", |cli| cli.vanilla),
            ("--save", |cli| cli.save),
            ("--ask-save", |cli| cli.ask_save),
            ("--restore-data", |cli| cli.restore_data),
            ("--debug", |cli| cli.debug),
            ("--coverage", |cli| cli.coverage),
            ("--cprofile", |cli| cli.cprofile),
        ];

        for (flag, is_set) in cases {
            let cli = Cli::parse_from(["orchard", flag]);
            assert!(is_set(&cli), "{flag} did not set its field");
        }

        let cli = Cli::parse_from(["orchard", "-q"]);
        assert!(cli.quiet);
    }

    #[test]
    fn phase1_value_flags_parse() {
        let cli = Cli::parse_from([
            "orchard",
            "--r-binary",
            "/opt/R/bin/R",
            "--profile",
            "/tmp/profile.R",
        ]);

        assert_eq!(cli.r_binary, Some(PathBuf::from("/opt/R/bin/R")));
        assert_eq!(cli.profile, Some(PathBuf::from("/tmp/profile.R")));
    }

    #[test]
    fn vanilla_expands() {
        let cli = Cli::parse_from(["orchard", "--vanilla"]).expanded();
        assert!(cli.no_history);
        assert!(cli.no_environ);
        assert!(cli.no_site_file);
        assert!(cli.no_init_file);
    }

    #[test]
    fn accepts_ignored_flags() {
        let cli = Cli::parse_from([
            "orchard",
            "--no-save",
            "--no-restore-data",
            "--no-restore-history",
            "--no-restore",
            "--no-readline",
            "--interactive",
        ]);
        assert!(cli.no_save && cli.no_restore_data && cli.no_restore_history);
        assert!(cli.no_restore && cli.no_readline && cli.interactive);
    }
}

