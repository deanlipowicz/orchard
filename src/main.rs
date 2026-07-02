use anyhow::Context;
use orchard::{cli, dyld, env_setup, history, profile, r_discovery, r_runtime, settings};
use std::io::IsTerminal;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse_args();
    if cli.debug {
        tracing_subscriber::fmt()
            .with_env_filter("orchard=debug")
            .try_init()
            .ok();
    }

    let r = r_discovery::discover(cli.r_binary.as_deref()).context("Cannot find R binary")?;

    if cli.version {
        println!("orchard version: {}", env!("CARGO_PKG_VERSION"));
        println!("r executable: {}", r.binary.display());
        println!(
            "r version: {}",
            r.version().unwrap_or_else(|_| "NA".to_string())
        );
        println!(
            "rust executable: {}",
            std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "NA".to_string())
        );
        return Ok(());
    }

    let cli = cli.expanded();
    env_setup::apply(&cli, &r)?;
    dyld::repair_and_reexec_if_needed(&r.home)?;

    let mut runtime = r_runtime::RRuntime::init(&cli)?;
    runtime.register_console_callbacks();
    runtime.init_repl();

    let settings = if std::io::stdin().is_terminal() {
        // Source profiles directly so R options are available before settings load.
        profile::source_profiles(&mut runtime, &cli)?;
        settings::Settings::load_from_r_options(&mut runtime)?
    } else {
        if let Some(profile) = cli.profile.as_ref().filter(|path| path.exists()) {
            r_runtime::install_startup_inputs(vec![format!(
                "base::source({}, local = base::globalenv())\n",
                r_string(&profile.display().to_string())
            )]);
        }
        settings::Settings::default()
    };
    r_runtime::install_console_settings(&settings);
    r_runtime::install_history(history::History::new(&cli, &settings)?);

    runtime.run_repl();
    Ok(())
}

fn r_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

