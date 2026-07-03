use anyhow::Context;
use orchard::{
    auto_reload, cli, dyld, editor_bridge, env_setup, history, profile, r_discovery, r_runtime,
    settings, util,
};
use std::io::{IsTerminal, Write};

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

    // --send mode: connect to running orchard, send code, print response, exit.
    // This does NOT initialize R — it's a client, not a server.
    if let Some(code) = &cli.send {
        let path = editor_bridge::resolve_socket_path();
        match editor_bridge::send_code(&path, code) {
            Ok(resp) => {
                write!(std::io::stdout(), "{}", resp.output)?;
                std::io::stdout().flush()?;
                std::process::exit(if resp.status == "ok" { 0 } else { 1 });
            }
            Err(e) => {
                eprintln!("Error sending code to orchard: {e}");
                std::process::exit(1);
            }
        }
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
                util::r_string(&profile.display().to_string())
            )]);
        }
        settings::Settings::default()
    };
    r_runtime::install_console_settings(&settings);
    r_runtime::install_history(history::History::new(&cli, &settings)?);

    // Redirect R's default graphics device to PNG capture for inline display.
    runtime.setup_plot_capture()?;

    // Start editor socket listener so editors can send code to the REPL.
    // The SocketGuard removes the socket file on normal exit.
    let socket_path = editor_bridge::resolve_socket_path();
    let _socket_guard = editor_bridge::SocketGuard {
        path: socket_path.clone(),
    };
    if let Err(e) = editor_bridge::run_listener(&socket_path) {
        eprintln!("Warning: could not start editor socket: {e}");
    }

    // Start filesystem watcher for Revise-style auto-reload of R source files.
    // The guard stops the watcher on drop (process exit).
    if let Ok((_watcher_handle, _watcher_guard)) = auto_reload::start_watcher() {
        // Watcher running in background — auto-reload enabled via R option.
    }

    runtime.run_repl();
    Ok(())
}
