use crate::{
    cli::Cli,
    editing_hook,
    history::{History, OrchardHistoryBackend},
    magic::{self, Output as MagicOutput},
    prompt::{PromptSession, ReadResult},
    settings::{CustomKeyBinding, Settings},
    shell,
    util::r_string,
};
use anyhow::{Context, bail};
use libc::{c_char, c_int, c_uchar};
use regex::Regex;
use std::{
    collections::VecDeque,
    ffi::{CStr, CString},
    io::{self, IsTerminal, Write},
    path::Path,
    ptr, slice,
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex, OnceLock},
};

/// Interactive mode for the current prompt.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum PromptMode {
    /// Normal R REPL.
    R,
    /// `browser()` session.
    Browse,
    /// `;` shell mode (future).
    Shell,
    /// Fallback for unrecognized prompts.
    #[default]
    Unknown,
}

impl PromptMode {
    /// Detect the mode from the raw R prompt string.
    pub fn detect(prompt: &str, settings: &ConsoleSettings) -> Self {
        if browse_level(prompt).is_some() {
            PromptMode::Browse
        } else if prompt == settings.prompt || prompt == "> " {
            PromptMode::R
        } else {
            PromptMode::Unknown
        }
    }

    /// Whether multiline input is allowed in this mode.
    pub fn multiline_allowed(&self, settings: &ConsoleSettings) -> bool {
        matches!(self, PromptMode::R | PromptMode::Browse | PromptMode::Shell)
            && settings.indent_lines
    }

    /// Whether input should be accepted without prompting again.
    pub fn accept_inline(&self) -> bool {
        matches!(self, PromptMode::R | PromptMode::Browse)
    }

    /// History book label used by the history file format.
    pub fn history_book(&self) -> &'static str {
        match self {
            PromptMode::R | PromptMode::Browse => "r",
            PromptMode::Shell => "shell",
            PromptMode::Unknown => "unknown",
        }
    }

    /// Canonical mode string stored in history file entries.
    /// This is the actual mode, not the history book grouping.
    pub fn mode_string(&self) -> &'static str {
        match self {
            PromptMode::R => "r",
            PromptMode::Browse => "browse",
            PromptMode::Shell => "shell",
            PromptMode::Unknown => "unknown",
        }
    }
}

#[allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    clippy::upper_case_acronyms,
    unnecessary_transmutes,
    clippy::ptr_offset_with_cast
)]
mod ffi {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

/// Periodic R event processing via SIGALRM.
///
/// Installs a 33 ms `setitimer` that fires SIGALRM and calls
/// `R_PolledEvents()` from the signal handler.  `R_PolledEvents` is
/// signal-safe per R documentation and is the same mechanism Python
/// orchard's inputhook uses.
#[cfg(unix)]
pub(crate) mod input_hook {
    use std::sync::atomic::{AtomicBool, Ordering};

    static HOOK_ACTIVE: AtomicBool = AtomicBool::new(false);
    static REENTRY_GUARD: AtomicBool = AtomicBool::new(false);

    /// Install the periodic `R_PolledEvents` timer.
    /// Safe to call multiple times — subsequent calls are no-ops.
    pub fn install() {
        if HOOK_ACTIVE.swap(true, Ordering::SeqCst) {
            return;
        }
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_flags = libc::SA_RESTART;
            // sa_sigaction and sa_handler share the same union slot in C;
            // libc exposes the field as sa_sigaction (type usize) on this
            // platform, so we cast the function pointer directly.
            action.sa_sigaction = polled_events_handler as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(libc::SIGALRM, &action, std::ptr::null_mut());

            let mut itv: libc::itimerval = std::mem::zeroed();
            itv.it_interval.tv_sec = 0;
            itv.it_interval.tv_usec = 33000;
            itv.it_value.tv_sec = 0;
            itv.it_value.tv_usec = 33000;
            libc::setitimer(libc::ITIMER_REAL, &itv, std::ptr::null_mut());
        }
    }

    /// Remove the periodic timer and restore the default SIGALRM handler.
    /// Safe to call multiple times — subsequent calls are no-ops.
    pub fn remove() {
        if !HOOK_ACTIVE.swap(false, Ordering::SeqCst) {
            return;
        }
        unsafe {
            let zero: libc::itimerval = std::mem::zeroed();
            libc::setitimer(libc::ITIMER_REAL, &zero, std::ptr::null_mut());
            libc::signal(libc::SIGALRM, libc::SIG_DFL);
        }
    }

    extern "C" fn polled_events_handler(
        _sig: libc::c_int,
        _info: *mut libc::siginfo_t,
        _ctx: *mut libc::c_void,
    ) {
        // Reentrancy guard: SIGALRM fires every 33 ms and R_PolledEvents
        // can trigger arbitrary R internal code.  If we re-enter while
        // still processing (e.g. R_PolledEvents itself triggers another
        // signal), we skip to avoid corrupting R's internal state.
        if REENTRY_GUARD.swap(true, Ordering::SeqCst) {
            return;
        }
        // R_PolledEvents is a function pointer from the generated bindings.
        // It may be null if no handlers are registered — that is fine.
        unsafe {
            if let Some(polled) = super::ffi::R_PolledEvents {
                polled();
            }
        }
        REENTRY_GUARD.store(false, Ordering::SeqCst);
    }
}

unsafe extern "C" {
    static mut ptr_R_ReadConsole:
        Option<extern "C" fn(*const c_char, *mut c_uchar, c_int, c_int) -> c_int>;
    static mut ptr_R_WriteConsole: Option<extern "C" fn(*const c_char, c_int)>;
    static mut ptr_R_WriteConsoleEx: Option<extern "C" fn(*const c_char, c_int, c_int)>;
    static mut ptr_R_ShowMessage: Option<extern "C" fn(*const c_char)>;
    static mut ptr_R_FlushConsole: Option<extern "C" fn()>;
    static mut ptr_R_ClearerrConsole: Option<extern "C" fn()>;
    static mut ptr_R_ResetConsole: Option<extern "C" fn()>;
    static mut ptr_R_Busy: Option<extern "C" fn(c_int)>;
    static mut ptr_R_Suicide: Option<extern "C" fn(*const c_char)>;
    static mut ptr_R_CleanUp: Option<extern "C" fn(c_int, c_int, c_int)>;
}

pub struct RRuntime {
    repl_initialized: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConsoleReadRoute {
    Native,
    Interactive,
    Piped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShellPromptResult {
    ReturnToR,
    Eof,
}

/// RAII guard that protects an R SEXP via `R_PreserveObject` and
/// automatically releases it on drop.
///
/// Uses R's global (stack-agnostic) protection list rather than the
/// per-frame protect stack, so it is immune to protect-stack ordering
/// bugs when multiple `ProtectedSexp` objects are live across scope
/// boundaries.
struct ProtectedSexp(ffi::SEXP);

impl ProtectedSexp {
    /// Wrap a SEXP and register it with R's global protection list.
    fn new(sexp: ffi::SEXP) -> Self {
        unsafe { ffi::R_PreserveObject(sexp) };
        ProtectedSexp(sexp)
    }

    fn get(&self) -> ffi::SEXP {
        self.0
    }
}

impl Drop for ProtectedSexp {
    fn drop(&mut self) {
        unsafe { ffi::R_ReleaseObject(self.0) };
    }
}

#[derive(Clone)]
pub struct ConsoleSettings {
    pub prompt: String,
    pub browse_prompt: String,
    pub shell_prompt: String,
    pub insert_new_line: bool,
    pub indent_lines: bool,
    pub stderr_format: String,
    pub show_vi_mode_prompt: bool,
    pub vi_mode_prompt: String,
    pub editing_mode: String,
    pub completion_prefix_length: i32,
    pub completion_timeout: f64,
    pub completion_adding_spaces_around_equals: bool,
    pub auto_width: bool,
    pub automagic: bool,
    pub tab_size: i32,
    pub auto_match: bool,
    pub auto_indentation: bool,
    pub auto_suggest: bool,
    pub escape_key_map: Vec<CustomKeyBinding>,
    pub ctrl_key_map: Vec<CustomKeyBinding>,
    pub highlight_matching_bracket: bool,
}

impl From<Settings> for ConsoleSettings {
    fn from(s: Settings) -> Self {
        Self {
            prompt: s.prompt,
            browse_prompt: s.browse_prompt,
            shell_prompt: s.shell_prompt,
            insert_new_line: s.insert_new_line,
            indent_lines: s.indent_lines,
            stderr_format: s.stderr_format,
            show_vi_mode_prompt: s.show_vi_mode_prompt,
            vi_mode_prompt: s.vi_mode_prompt,
            editing_mode: s.editing_mode,
            completion_prefix_length: s.completion_prefix_length,
            completion_timeout: s.completion_timeout,
            completion_adding_spaces_around_equals: s.completion_adding_spaces_around_equals,
            auto_width: s.auto_width,
            automagic: s.automagic,
            tab_size: s.tab_size,
            auto_match: s.auto_match,
            auto_indentation: s.auto_indentation,
            auto_suggest: s.auto_suggest,
            escape_key_map: s.escape_key_map,
            ctrl_key_map: s.ctrl_key_map,
            highlight_matching_bracket: s.highlight_matching_bracket,
        }
    }
}

struct ConsoleState {
    settings: ConsoleSettings,
    terminal_cursor_at_beginning: bool,
    startup_inputs: VecDeque<String>,
    pending_inputs: VecDeque<String>,
    prompt_active: bool,
    history: Option<History>,
    mode_arc: Arc<Mutex<PromptMode>>,
    prompt_session: Option<PromptSession>,
    last_terminal_width: Option<i32>,
}

impl Default for ConsoleState {
    fn default() -> Self {
        Self {
            settings: Settings::default().into(),
            terminal_cursor_at_beginning: true,
            startup_inputs: VecDeque::new(),
            pending_inputs: VecDeque::new(),
            prompt_active: false,
            history: None,
            mode_arc: Arc::new(Mutex::new(PromptMode::default())),
            prompt_session: None,
            last_terminal_width: None,
        }
    }
}

static CONSOLE: OnceLock<Mutex<ConsoleState>> = OnceLock::new();
static SUPPRESS_STDOUT: AtomicBool = AtomicBool::new(false);
static SUPPRESS_STDERR: AtomicBool = AtomicBool::new(false);
static INTERRUPTED: AtomicBool = AtomicBool::new(false);
/// Whether the R runtime has been initialized via `Rf_initEmbeddedR`.
/// Free functions that call into R's C API MUST check this flag before
/// making FFI calls to avoid SIGSEGV when R is not available (e.g. in
/// unit tests).
static R_AVAILABLE: AtomicBool = AtomicBool::new(false);

pub fn install_console_settings(settings: &Settings) {
    let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
    let mut state = console.lock().unwrap();
    state.settings = ConsoleSettings::from(settings.clone());
    state.prompt_session = None;
}

pub fn install_startup_inputs(inputs: Vec<String>) {
    let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
    console.lock().unwrap().startup_inputs = inputs.into();
}

pub fn install_history(history: History) {
    let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
    console.lock().unwrap().history = Some(history);
}

/// Returns a snapshot of current history entry texts (for magics).
pub fn history_text_snapshot() -> Vec<String> {
    let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
    let state = console.lock().unwrap();
    match &state.history {
        Some(h) => h.entries().iter().map(|e| e.text.clone()).collect(),
        None => Vec::new(),
    }
}

/// Returns a snapshot of current history entries (for magics).
pub fn history_entries_snapshot() -> Vec<super::history::Entry> {
    let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
    let state = console.lock().unwrap();
    match &state.history {
        Some(h) => h.entries().to_vec(),
        None => Vec::new(),
    }
}

pub fn set_suppress_stdout(suppress: bool) {
    SUPPRESS_STDOUT.store(suppress, Ordering::SeqCst);
}

pub fn set_suppress_stderr(suppress: bool) {
    SUPPRESS_STDERR.store(suppress, Ordering::SeqCst);
}

/// Toggle automagic on or off. When enabled, registered magic names can be
/// used without the `%` prefix (unless followed by `(` which indicates an R
/// function call).
pub fn set_automagic(enabled: bool) {
    let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
    let mut state = console.lock().unwrap();
    state.settings.automagic = enabled;
}

/// Get the current automagic setting.
pub fn get_automagic() -> bool {
    let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
    let state = console.lock().unwrap();
    state.settings.automagic
}

pub fn eval_string_raw_global(code: &str) -> anyhow::Result<String> {
    if !R_AVAILABLE.load(Ordering::SeqCst) {
        bail!("R is not initialized — cannot evaluate code: {code}");
    }
    unsafe {
        let protected = eval_code(code)?;
        sexp_to_string(protected.get())
    }
}

pub fn with_suppressed_stderr<T>(f: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    set_suppress_stderr(true);
    let result = f();
    set_suppress_stderr(false);
    result
}

/// Check and reset the interrupted flag. Returns true if a Ctrl-C was
/// received since the last call.
pub fn interrupted_flag(clear: bool) -> bool {
    if clear {
        INTERRUPTED.swap(false, Ordering::SeqCst)
    } else {
        INTERRUPTED.load(Ordering::SeqCst)
    }
}

/// Set the interrupted flag (raised by Ctrl-C in read_console).
pub fn set_interrupted_flag() {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

fn raise_r_interrupt() -> c_int {
    set_interrupted_flag();
    interrupted_flag(true);
    unsafe { ffi::Rf_onintrNoResume() };
    0
}

impl RRuntime {
    pub fn init(cli: &Cli) -> anyhow::Result<Self> {
        let mut args = vec![
            CString::new("orchard")?,
            CString::new("--no-readline")?,
            CString::new("--no-restore-history")?,
        ];
        if cli.quiet {
            args.push(CString::new("--quiet")?);
        }
        if cli.save || cli.ask_save {
            args.push(CString::new("--save")?);
        } else {
            args.push(CString::new("--no-save")?);
        }
        if !cli.restore_data {
            args.push(CString::new("--no-restore")?);
        }

        let mut raw: Vec<*mut c_char> = args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        unsafe {
            let rc = ffi::Rf_initEmbeddedR(raw.len() as c_int, raw.as_mut_ptr());
            if rc < 0 {
                bail!("R initialization failed with status {rc}");
            }
        }
        R_AVAILABLE.store(true, Ordering::SeqCst);
        Ok(Self {
            repl_initialized: false,
        })
    }

    pub fn register_console_callbacks(&mut self) {
        unsafe {
            ptr_R_ReadConsole = Some(read_console);
            ptr_R_WriteConsole = Some(write_console);
            ptr_R_WriteConsoleEx = Some(write_console_ex);
            ptr_R_ShowMessage = Some(show_message);
            ptr_R_FlushConsole = Some(flush_console);
            ptr_R_ClearerrConsole = Some(clearerr_console);
            ptr_R_ResetConsole = Some(reset_console);
            ptr_R_Busy = Some(busy);
            ptr_R_Suicide = Some(suicide);
            ptr_R_CleanUp = Some(clean_up);
        }
    }

    pub fn source_file(&mut self, path: &Path) -> anyhow::Result<()> {
        self.eval_void(&format!(
            "base::source({}, local = base::new.env())",
            r_string(&path.display().to_string())
        ))
    }

    pub fn get_option_string(
        &mut self,
        name: &str,
        default: Option<&str>,
    ) -> anyhow::Result<Option<String>> {
        let default = default.map(r_string).unwrap_or_else(|| "NULL".to_string());
        let code = format!("getOption({}, {})", r_string(name), default);
        unsafe {
            let protected = eval_code(&code)?;
            let value = protected.get();
            if ffi::Rf_isNull(value) != 0 || ffi::Rf_length(value) == 0 {
                return Ok(None);
            }
            let text = ffi::R_CHAR(ffi::Rf_asChar(value));
            if text.is_null() {
                Ok(None)
            } else {
                Ok(Some(CStr::from_ptr(text).to_string_lossy().into_owned()))
            }
        }
    }

    pub fn get_option_bool(&mut self, name: &str, default: bool) -> anyhow::Result<bool> {
        let code = format!(
            "getOption({}, {})",
            r_string(name),
            if default { "TRUE" } else { "FALSE" }
        );
        unsafe { Ok(ffi::Rf_asLogical(eval_code(&code)?.get()) == 1) }
    }

    pub fn get_option_int(&mut self, name: &str, default: i32) -> anyhow::Result<i32> {
        let code = format!("getOption({}, {}L)", r_string(name), default);
        unsafe { Ok(ffi::Rf_asInteger(eval_code(&code)?.get())) }
    }

    pub fn get_option_real(&mut self, name: &str, default: f64) -> anyhow::Result<f64> {
        let code = format!("getOption({}, {})", r_string(name), default);
        unsafe { Ok(ffi::Rf_asReal(eval_code(&code)?.get())) }
    }

    pub fn set_option_string(&mut self, name: &str, value: &str) -> anyhow::Result<()> {
        self.eval_void(&format!("options({} = {})", name, r_string(value)))
    }

    pub fn set_option_bool(&mut self, name: &str, value: bool) -> anyhow::Result<()> {
        self.eval_void(&format!(
            "options({} = {})",
            name,
            if value { "TRUE" } else { "FALSE" }
        ))
    }

    pub fn parse_complete(&mut self, code: &str) -> bool {
        text_looks_complete(code)
    }

    pub fn eval_void(&mut self, code: &str) -> anyhow::Result<()> {
        unsafe {
            eval_code(code)?;
        }
        Ok(())
    }

    pub fn eval_string_raw(&mut self, code: &str) -> anyhow::Result<String> {
        unsafe {
            let protected = eval_code(code)?;
            sexp_to_string(protected.get())
        }
    }

    pub fn run_repl(&mut self) {
        self.init_repl();
        unsafe { while ffi::R_ReplDLLdo1() > 0 {} }
        input_hook::remove();
    }

    /// Set R's default graphics device to a PNG capture that writes to
    /// orchard's plot temp directory.  All R `plot()` / `ggplot()` etc.
    /// calls will create PNG files instead of opening X11/Cairo windows.
    pub fn setup_plot_capture(&mut self) -> anyhow::Result<()> {
        // Use eval_void so this runs as R code inside the embedded session.
        // We point the default device at a png() call that writes a
        // timestamped file into the system temp dir under orchard_plots/.
        self.eval_void(
            r#"options(device = function() {
  png(file.path(tempdir(), paste0("orchard_plot_", as.integer(Sys.time() * 1e6), ".png")),
      width = 800, height = 600)
})"#,
        )
    }

    pub fn init_repl(&mut self) {
        if !self.repl_initialized {
            unsafe { ffi::R_ReplDLLinit() };
            input_hook::install();
            self.repl_initialized = true;
        }
    }
}

unsafe fn sexp_to_string(result: ffi::SEXP) -> anyhow::Result<String> {
    unsafe {
        if ffi::Rf_isNull(result) != 0 || ffi::Rf_length(result) == 0 {
            return Ok(String::new());
        }
        let text = ffi::R_CHAR(ffi::Rf_asChar(result));
        if text.is_null() {
            Ok(String::new())
        } else {
            Ok(CStr::from_ptr(text).to_string_lossy().into_owned())
        }
    }
}

unsafe fn eval_code(code: &str) -> anyhow::Result<ProtectedSexp> {
    let code = CString::new(code)?;
    let mut status = ffi::ParseStatus_PARSE_NULL;
    unsafe {
        let input = ProtectedSexp::new(ffi::Rf_mkString(code.as_ptr()));
        let expr = ProtectedSexp::new(ffi::R_ParseVector(
            input.get(),
            -1,
            &mut status,
            ffi::R_NilValue,
        ));
        if status != ffi::ParseStatus_PARSE_OK {
            bail!("R parse failed with status {status}: {code:?}");
        }
        let mut result = ffi::R_NilValue;
        for i in 0..ffi::Rf_length(expr.get()) {
            let mut error = 0;
            // R_ParseVector returns an EXPRSXP; its elements must be
            // accessed with VECTOR_ELT, not Rf_elt (which walks a
            // pairlist shape and returns garbage for EXPRSXPs, leading
            // to a segfault inside R_tryEval).
            result = ffi::R_tryEval(
                ffi::orchard_VECTOR_ELT(expr.get(), i as ffi::R_xlen_t),
                ffi::R_GlobalEnv,
                &mut error,
            );
            if error != 0 {
                let message = r_error_message();
                bail!("R evaluation failed: {code:?}: {message}");
            }
        }
        // Protect result with its own RAII guard; input and expr are
        // dropped at the end of this block, releasing them from protection.
        Ok(ProtectedSexp::new(result))
    }
}

unsafe fn r_error_message() -> String {
    unsafe {
        let message = ffi::R_curErrorBuf();
        if message.is_null() {
            "unknown R error".to_string()
        } else {
            CStr::from_ptr(message).to_string_lossy().into_owned()
        }
    }
}

pub(crate) fn text_looks_complete(code: &str) -> bool {
    let trimmed = code.trim_end();
    if trimmed.is_empty() {
        return true;
    }
    if matches!(
        trimmed.chars().last(),
        Some('+' | '-' | '*' | '/' | '^' | '=' | '<' | '>' | '|' | '&' | ',' | '(' | '[' | '{')
    ) {
        return false;
    }

    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut comment = false;
    for ch in trimmed.chars() {
        if comment {
            comment = ch != '\n';
            continue;
        }
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '#' => comment = true,
            '"' | '\'' => quote = Some(ch),
            '(' | '[' | '{' => stack.push(ch),
            ')' => {
                if stack.pop() != Some('(') {
                    return true;
                }
            }
            ']' if stack.pop() != Some('[') => return true,
            '}' if stack.pop() != Some('{') => return true,
            _ => {}
        }
    }
    quote.is_none() && stack.is_empty()
}

extern "C" fn read_console(
    prompt: *const c_char,
    buf: *mut c_uchar,
    len: c_int,
    _add_to_history: c_int,
) -> c_int {
    let raw_prompt = if prompt.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(prompt) }
            .to_string_lossy()
            .into_owned()
    };
    if let Some(text) = {
        let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
        console.lock().unwrap().pending_inputs.pop_front()
    } {
        return queue_prepared_input(&text, buf, len);
    }
    if let Some(text) = {
        let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
        console.lock().unwrap().startup_inputs.pop_front()
    } {
        return queue_input(&text, &PromptMode::Unknown, buf, len);
    }
    // If a Ctrl-C was received since the last read, signal interruption.
    if interrupted_flag(true) {
        return raise_r_interrupt();
    }
    let (settings, prompt_active) = {
        let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
        let mut state = console.lock().unwrap();
        if !state.terminal_cursor_at_beginning
            || (state.settings.insert_new_line
                && is_r_or_browse_prompt(&raw_prompt, &state.settings))
        {
            println!();
            state.terminal_cursor_at_beginning = true;
        }
        (state.settings.clone(), state.prompt_active)
    };

    let shown_prompt = display_prompt(&raw_prompt, &settings);
    let continuation_prompt = "+ ";
    let mode = PromptMode::detect(&raw_prompt, &settings);
    sync_terminal_width(&settings);
    match console_read_route(prompt_active, io::stdin().is_terminal()) {
        ConsoleReadRoute::Native => return read_console_native(&shown_prompt, &mode, buf, len),
        ConsoleReadRoute::Interactive => {
            return read_console_interactive(
                &settings,
                &shown_prompt,
                continuation_prompt,
                &mode,
                buf,
                len,
            );
        }
        ConsoleReadRoute::Piped => {}
    }

    loop {
        let mut text = String::new();
        let mut is_first_line = true;
        loop {
            let prompt = if is_first_line {
                &shown_prompt
            } else {
                continuation_prompt
            };
            is_first_line = false;
            print!("{prompt}");
            io::stdout().flush().ok();

            let mut line = String::new();
            match io::stdin().read_line(&mut line) {
                Ok(0) => return 0,
                Ok(_) => text.push_str(&line),
                Err(err) if err.kind() == io::ErrorKind::Interrupted => {
                    println!();
                    return raise_r_interrupt();
                }
                Err(_) => return 0,
            }

            if !mode.multiline_allowed(&settings) {
                break;
            }
            if text_looks_complete(&text) {
                break;
            }
        }

        if text.is_empty() {
            return 0;
        }
        if mode.accept_inline()
            && let Some(command) = shell_command(&text)
        {
            shell::run_command(command);
            append_history(&PromptMode::Shell, command);
            continue;
        }
        // ? modal help: route ?name → %pdoc, ??name → %psource
        if let Some(rest) = text.trim_start().strip_prefix('?') {
            if rest.starts_with('?') {
                // ??name → show source code
                let source_query = rest
                    .strip_prefix('?')
                    .unwrap_or(rest)
                    .trim()
                    .trim_end_matches('\n');
                if source_query.is_empty() {
                    println!("Show source code for an R function.\nUsage: ??function_name");
                } else if let Err(e) = dispatch_source(source_query) {
                    eprintln!("{e}");
                }
            } else {
                // ?name → show documentation
                let doc_query = rest.trim().trim_end_matches('\n');
                if doc_query.is_empty() {
                    println!("Show documentation for an R function.\nUsage: ?function_name");
                } else if let Err(e) = dispatch_doc(doc_query) {
                    eprintln!("{e}");
                }
            }
            io::stdout().flush().ok();
            continue;
        }
        if let Some(magic_cmd) = magic::parse_magic(&text, settings.automagic) {
            match handle_magic_output(magic::dispatch(&magic_cmd), &mode, &text, buf, len) {
                MagicDispatchResult::Continue => continue,
                MagicDispatchResult::Return(val) => return val,
            }
        }
        append_history(&mode, &text);
        return queue_input(&text, &mode, buf, len);
    }
}

fn sync_terminal_width(settings: &ConsoleSettings) {
    let Some(cols) = detected_terminal_width() else {
        return;
    };
    let width = {
        let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
        let mut state = console.lock().unwrap();
        terminal_width_update(&mut state.last_terminal_width, settings, cols)
    };
    if let Some(width) = width {
        unsafe {
            eval_code(&format!("options(width = {width}L)")).ok();
        }
    }
}

fn terminal_width_update(
    last_width: &mut Option<i32>,
    settings: &ConsoleSettings,
    detected_cols: i32,
) -> Option<i32> {
    if !settings.auto_width {
        return None;
    }
    let width = detected_cols.max(20);
    if *last_width == Some(width) {
        None
    } else {
        *last_width = Some(width);
        Some(width)
    }
}

#[cfg(unix)]
fn detected_terminal_width() -> Option<i32> {
    let mut size = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let ok = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) == 0 };
    (ok && size.ws_col > 0).then_some(i32::from(size.ws_col))
}

fn console_read_route(prompt_active: bool, stdin_is_terminal: bool) -> ConsoleReadRoute {
    if prompt_active {
        ConsoleReadRoute::Native
    } else if stdin_is_terminal {
        ConsoleReadRoute::Interactive
    } else {
        ConsoleReadRoute::Piped
    }
}

fn read_console_interactive(
    settings: &ConsoleSettings,
    shown_prompt: &str,
    continuation_prompt: &str,
    mode: &PromptMode,
    buf: *mut c_uchar,
    len: c_int,
) -> c_int {
    loop {
        let mut session = {
            let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
            let mut state = console.lock().unwrap();
            state.prompt_active = true;
            state.prompt_session.take().unwrap_or_else(|| {
                let mode_arc = state.mode_arc.clone();
                let entries = state
                    .history
                    .as_ref()
                    .map(|h| h.entries().to_vec())
                    .unwrap_or_default();
                if entries.is_empty() {
                    PromptSession::new(settings)
                } else {
                    let backend = OrchardHistoryBackend::new(&entries, mode_arc.clone());
                    PromptSession::with_arc_history(settings, backend, mode_arc)
                }
            })
        };
        let result = session.read_line(
            shown_prompt.to_string(),
            continuation_prompt.to_string(),
            mode.clone(),
        );
        let mut text = match result {
            ReadResult::Line(line) => line,
            ReadResult::CtrlC => {
                store_prompt_session(session);
                return raise_r_interrupt();
            }
            ReadResult::Eof | ReadResult::Error => {
                store_prompt_session(session);
                return 0;
            }
        };
        if !text.ends_with('\n') {
            text.push('\n');
        }
        if text.is_empty() {
            store_prompt_session(session);
            return 0;
        }
        if mode.accept_inline()
            && let Some(command) = shell_command(&text)
        {
            if command.is_empty() {
                let result = read_shell_prompt(&mut session, settings);
                store_prompt_session(session);
                match result {
                    ShellPromptResult::ReturnToR => continue,
                    ShellPromptResult::Eof => return 0,
                }
            } else {
                shell::run_command(command);
                append_history(&PromptMode::Shell, command);
                store_prompt_session(session);
            }
            continue;
        }
        // ? modal help: route ?name → %pdoc, ??name → %psource
        if let Some(rest) = text.trim_start().strip_prefix('?') {
            if rest.starts_with('?') {
                // ??name → show source code
                let source_query = rest
                    .strip_prefix('?')
                    .unwrap_or(rest)
                    .trim()
                    .trim_end_matches('\n');
                if source_query.is_empty() {
                    println!("Show source code for an R function.\nUsage: ??function_name");
                } else if let Err(e) = dispatch_source(source_query) {
                    eprintln!("{e}");
                }
            } else {
                // ?name → show documentation
                let doc_query = rest.trim().trim_end_matches('\n');
                if doc_query.is_empty() {
                    println!("Show documentation for an R function.\nUsage: ?function_name");
                } else if let Err(e) = dispatch_doc(doc_query) {
                    eprintln!("{e}");
                }
            }
            io::stdout().flush().ok();
            store_prompt_session(session);
            continue;
        }
        if let Some(magic_cmd) = magic::parse_magic(&text, settings.automagic) {
            match handle_magic_output(magic::dispatch(&magic_cmd), mode, &text, buf, len) {
                MagicDispatchResult::Continue => {
                    store_prompt_session(session);
                    continue;
                }
                MagicDispatchResult::Return(val) => {
                    store_prompt_session(session);
                    return val;
                }
            }
        }
        store_prompt_session(session);
        append_history(mode, &text);
        return queue_input(&text, mode, buf, len);
    }
}

fn store_prompt_session(session: PromptSession) {
    let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
    let mut state = console.lock().unwrap();
    state.prompt_session = Some(session);
    state.prompt_active = false;
}

fn read_shell_prompt(session: &mut PromptSession, settings: &ConsoleSettings) -> ShellPromptResult {
    editing_hook::set_shell_mode(true);
    let result = loop {
        match session.read_line(
            settings.shell_prompt.clone(),
            String::new(),
            PromptMode::Shell,
        ) {
            ReadResult::Line(line) => {
                let command = line.trim_end_matches('\n');
                if command.is_empty() {
                    break ShellPromptResult::ReturnToR;
                }
                shell::run_command(command);
                append_history(&PromptMode::Shell, command);
            }
            ReadResult::CtrlC => break ShellPromptResult::ReturnToR,
            ReadResult::Eof | ReadResult::Error => break ShellPromptResult::Eof,
        }
    };
    editing_hook::set_shell_mode(false);
    result
}

fn shell_command(text: &str) -> Option<&str> {
    text.trim_end_matches('\n').strip_prefix(';')
}

fn read_console_native(
    shown_prompt: &str,
    mode: &PromptMode,
    buf: *mut c_uchar,
    len: c_int,
) -> c_int {
    print!("{shown_prompt}");
    io::stdout().flush().ok();

    let mut text = String::new();
    match io::stdin().read_line(&mut text) {
        Ok(0) => 0,
        Ok(_) => queue_input(&text, mode, buf, len),
        Err(err) if err.kind() == io::ErrorKind::Interrupted => {
            println!();
            raise_r_interrupt()
        }
        Err(_) => 0,
    }
}

fn copy_input(text: &str, buf: *mut c_uchar, len: c_int) -> c_int {
    if len <= 1 {
        return 0;
    }
    let bytes = text.as_bytes();
    let n = utf8_chunk_len(text, (len as usize) - 1);
    if n == 0 {
        return 0;
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), buf, n);
        *buf.add(n) = 0;
    }
    1
}

fn queue_input(text: &str, mode: &PromptMode, buf: *mut c_uchar, len: c_int) -> c_int {
    if len <= 1 {
        return 0;
    }
    let max = (len as usize) - 1;
    let prepared = prepare_console_input(text, mode, max);
    queue_prepared_input(&prepared, buf, len)
}

fn queue_prepared_input(text: &str, buf: *mut c_uchar, len: c_int) -> c_int {
    if len <= 1 {
        return 0;
    }
    let (head, tail) = split_console_input(text, (len as usize) - 1);
    if !tail.is_empty() {
        let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
        console.lock().unwrap().pending_inputs.push_back(tail);
    }
    copy_input(&head, buf, len)
}

fn prepare_console_input(text: &str, mode: &PromptMode, max: usize) -> String {
    let text = ensure_trailing_newline(text);
    if matches!(mode, PromptMode::R | PromptMode::Browse)
        && text.len() > max
        && !text.is_ascii()
        && text.trim_end_matches(['\r', '\n']).contains('\n')
    {
        let inner = text.trim_end_matches(['\r', '\n']);
        format!("{{\n{inner}\n}}\n")
    } else {
        text
    }
}

fn ensure_trailing_newline(text: &str) -> String {
    if text.ends_with('\n') {
        text.to_string()
    } else {
        format!("{text}\n")
    }
}

fn split_console_input(text: &str, max: usize) -> (String, String) {
    let n = utf8_chunk_len(text, max);
    (text[..n].to_string(), text[n..].to_string())
}

fn utf8_chunk_len(text: &str, max: usize) -> usize {
    if text.len() <= max {
        return text.len();
    }
    let mut n = max;
    while n > 0 && !text.is_char_boundary(n) {
        n -= 1;
    }
    n
}

extern "C" fn write_console(buf: *const c_char, len: c_int) {
    if !SUPPRESS_STDOUT.load(Ordering::Relaxed) {
        write_bytes(buf, len, &mut io::stdout()).ok();
    }
}

extern "C" fn write_console_ex(buf: *const c_char, len: c_int, output_type: c_int) {
    if output_type == 0 {
        if !SUPPRESS_STDOUT.load(Ordering::Relaxed) {
            write_bytes(buf, len, &mut io::stdout()).ok();
        }
    } else {
        if !SUPPRESS_STDERR.load(Ordering::Relaxed) {
            write_stderr(buf, len).ok();
        }
    }
}

extern "C" fn show_message(msg: *const c_char) {
    if !msg.is_null() {
        unsafe {
            let text = CStr::from_ptr(msg).to_string_lossy();
            let _ = io::stderr().write_all(text.as_bytes());
        }
    }
}

extern "C" fn flush_console() {
    io::stdout().flush().ok();
}

extern "C" fn clearerr_console() {}

extern "C" fn reset_console() {}

extern "C" fn busy(_which: c_int) {}

extern "C" fn suicide(msg: *const c_char) {
    if !msg.is_null() {
        unsafe {
            let text = CStr::from_ptr(msg).to_string_lossy();
            let _ = io::stderr().write_all(text.as_bytes());
        }
    }
}

extern "C" fn clean_up(_saveact: c_int, _status: c_int, _runlast: c_int) {}

fn write_bytes<W: Write>(buf: *const c_char, len: c_int, out: &mut W) -> anyhow::Result<()> {
    if buf.is_null() || len <= 0 {
        return Ok(());
    }
    let bytes = unsafe { slice::from_raw_parts(buf as *const u8, len as usize) };
    out.write_all(bytes)
        .context("failed to write R console output")?;
    out.flush().ok();
    update_cursor(bytes);
    Ok(())
}

fn write_stderr(buf: *const c_char, len: c_int) -> anyhow::Result<()> {
    if buf.is_null() || len <= 0 {
        return Ok(());
    }
    let bytes = unsafe { slice::from_raw_parts(buf as *const u8, len as usize) };
    let text = String::from_utf8_lossy(bytes);
    let format = {
        let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
        console.lock().unwrap().settings.stderr_format.clone()
    };
    let formatted = format.replace("{}", &text);
    io::stderr()
        .write_all(formatted.as_bytes())
        .context("failed to write R stderr")?;
    io::stderr().flush().ok();
    update_cursor(bytes);
    Ok(())
}

fn display_prompt(prompt: &str, settings: &ConsoleSettings) -> String {
    let mode = PromptMode::detect(prompt, settings);
    let base = match &mode {
        PromptMode::Browse => {
            if let Some(level) = browse_level(prompt) {
                settings.browse_prompt.replace("{}", &level)
            } else {
                settings.browse_prompt.replace("{}", "?")
            }
        }
        PromptMode::R => settings.prompt.clone(),
        PromptMode::Shell => settings.shell_prompt.clone(),
        PromptMode::Unknown => prompt.to_string(),
    };
    if settings.show_vi_mode_prompt && settings.editing_mode == "vi" {
        format!("{}{}", settings.vi_mode_prompt.replace("{}", "I"), base)
    } else {
        base
    }
}

fn is_r_or_browse_prompt(prompt: &str, settings: &ConsoleSettings) -> bool {
    matches!(
        PromptMode::detect(prompt, settings),
        PromptMode::R | PromptMode::Browse
    )
}

fn append_history(mode: &PromptMode, text: &str) {
    if matches!(mode, PromptMode::Unknown) {
        return;
    }
    let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
    if let Some(history) = &mut console.lock().unwrap().history {
        history.append(mode.mode_string(), text).ok();
    }
}

fn browse_level(prompt: &str) -> Option<String> {
    prompt
        .strip_prefix("Browse[")
        .and_then(|s| s.strip_suffix("]> "))
        .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
        .map(ToString::to_string)
}

/// Strip ANSI escape sequences from a string.
fn strip_ansi(s: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Safe: hardcoded regex pattern is valid; will never fail to compile
    let re = RE.get_or_init(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap());
    re.replace_all(s, "").to_string()
}

fn update_cursor(bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes).replace("\r\n", "\n");
    if text.is_empty() {
        return;
    }
    let stripped = strip_ansi(&text);
    let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
    console.lock().unwrap().terminal_cursor_at_beginning = stripped.ends_with('\n');
}

/// Dispatch `?name` to `%pdoc name`.
fn dispatch_doc(topic: &str) -> Result<(), String> {
    let cmd = magic::MagicLine {
        name: "pdoc".into(),
        args: topic.to_string(),
        is_cell: false,
    };
    match magic::dispatch(&cmd) {
        Ok(MagicOutput::Text(msg)) => {
            print!("{msg}");
            Ok(())
        }
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Dispatch `??name` to `%psource name`.
fn dispatch_source(topic: &str) -> Result<(), String> {
    let cmd = magic::MagicLine {
        name: "psource".into(),
        args: topic.to_string(),
        is_cell: false,
    };
    match magic::dispatch(&cmd) {
        Ok(MagicOutput::Text(msg)) => {
            print!("{msg}");
            Ok(())
        }
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Result of handling a magic dispatch — tells the caller whether to continue
/// the loop or return a value.
enum MagicDispatchResult {
    /// Continue the REPL loop (output was displayed or action was silent).
    Continue,
    /// Return from the reader function with the given value.
    Return(c_int),
}

/// Handle the result of a magic dispatch.  Shared by the piped and interactive
/// read paths so that the five-arm `match` on `MagicOutput` is written once.
///
/// The caller adds `store_prompt_session` around the result when running in
/// interactive mode (where a session has been taken out of `CONSOLE`); the piped
/// path has no session to store.
fn handle_magic_output(
    result: Result<MagicOutput, magic::MagicError>,
    mode: &PromptMode,
    text: &str,
    buf: *mut c_uchar,
    len: c_int,
) -> MagicDispatchResult {
    match result {
        Ok(MagicOutput::Eval(code)) => {
            append_history(mode, text);
            MagicDispatchResult::Return(queue_input(&code, mode, buf, len))
        }
        Ok(MagicOutput::Text(msg)) => {
            print!("{msg}");
            io::stdout().flush().ok();
            MagicDispatchResult::Continue
        }
        Ok(MagicOutput::DisplayAndEval(code)) => {
            print!("{code}");
            io::stdout().flush().ok();
            append_history(mode, text);
            MagicDispatchResult::Return(queue_input(&code, mode, buf, len))
        }
        Ok(MagicOutput::Silent) => MagicDispatchResult::Continue,
        Err(err) => {
            eprintln!("{err}");
            MagicDispatchResult::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_settings() -> ConsoleSettings {
        Settings::default().into()
    }

    /// Serializes cursor-tracking tests so they don't race on the shared
    /// `CONSOLE` static when running in parallel.  Only the four tests
    /// below acquire this lock.
    static CURSOR_LOCK: Mutex<()> = Mutex::new(());

    // --- PromptMode detection ---

    #[test]
    fn detects_r_mode() {
        let s = test_settings();
        assert_eq!(PromptMode::detect("> ", &s), PromptMode::R);
        assert_eq!(PromptMode::detect("\x1b[34mr$>\x1b[0m ", &s), PromptMode::R);
    }

    #[test]
    fn detects_browse_mode() {
        let s = test_settings();
        assert_eq!(PromptMode::detect("Browse[1]> ", &s), PromptMode::Browse);
        assert_eq!(PromptMode::detect("Browse[42]> ", &s), PromptMode::Browse);
    }

    #[test]
    fn detects_unknown_mode() {
        let s = test_settings();
        assert_eq!(PromptMode::detect("? ", &s), PromptMode::Unknown);
        assert_eq!(PromptMode::detect("", &s), PromptMode::Unknown);
        assert_eq!(PromptMode::detect("Continue> ", &s), PromptMode::Unknown);
    }

    // --- display_prompt ---

    #[test]
    fn formats_browse_prompt() {
        let settings = test_settings();
        assert_eq!(
            display_prompt("Browse[12]> ", &settings),
            "\x1b[33mBrowse[12]>\x1b[0m "
        );
    }

    #[test]
    fn formats_r_prompt() {
        let s = test_settings();
        assert_eq!(display_prompt("> ", &s), "\x1b[34mr$>\x1b[0m ");
    }

    #[test]
    fn formats_unknown_prompt() {
        let s = test_settings();
        assert_eq!(display_prompt("Continue> ", &s), "Continue> ");
    }

    #[test]
    fn prepends_vi_mode_prompt() {
        let mut s = test_settings();
        s.show_vi_mode_prompt = true;
        s.editing_mode = "vi".to_string();
        // vi_mode_prompt has a trailing space, base prompt also has one.
        assert_eq!(
            display_prompt("> ", &s),
            "\x1b[34m[I]\x1b[0m \x1b[34mr$>\x1b[0m "
        );
    }

    #[test]
    fn does_not_prepend_vi_mode_in_emacs() {
        let mut s = test_settings();
        s.show_vi_mode_prompt = true;
        s.editing_mode = "emacs".to_string();
        // Should NOT prepend vi prompt in emacs mode
        assert_eq!(display_prompt("> ", &s), "\x1b[34mr$>\x1b[0m ");
    }

    // --- r_string ---

    #[test]
    fn quotes_r_strings() {
        assert_eq!(r_string(r#"a\b"c"#), r#""a\\b\"c""#);
    }

    // --- PromptMode method tests ---

    #[test]
    fn accept_inline_returns_true_for_r_and_browse() {
        assert!(PromptMode::R.accept_inline());
        assert!(PromptMode::Browse.accept_inline());
        assert!(!PromptMode::Shell.accept_inline());
        assert!(!PromptMode::Unknown.accept_inline());
    }

    #[test]
    fn multiline_allowed_gated_on_indent_lines() {
        let mut s = test_settings();
        s.indent_lines = true;
        assert!(PromptMode::R.multiline_allowed(&s));
        assert!(PromptMode::Browse.multiline_allowed(&s));
        assert!(PromptMode::Shell.multiline_allowed(&s));
        assert!(!PromptMode::Unknown.multiline_allowed(&s));

        s.indent_lines = false;
        assert!(!PromptMode::R.multiline_allowed(&s));
        assert!(!PromptMode::Browse.multiline_allowed(&s));
        assert!(!PromptMode::Shell.multiline_allowed(&s));
        assert!(!PromptMode::Unknown.multiline_allowed(&s));
    }

    #[test]
    fn history_book_maps_correctly() {
        assert_eq!(PromptMode::R.history_book(), "r");
        assert_eq!(PromptMode::Browse.history_book(), "r");
        assert_eq!(PromptMode::Shell.history_book(), "shell");
        assert_eq!(PromptMode::Unknown.history_book(), "unknown");
    }

    #[test]
    fn mode_string_matches_history_file_format() {
        assert_eq!(PromptMode::R.mode_string(), "r");
        assert_eq!(PromptMode::Browse.mode_string(), "browse");
        assert_eq!(PromptMode::Shell.mode_string(), "shell");
        assert_eq!(PromptMode::Unknown.mode_string(), "unknown");
    }

    // --- strip_ansi ---

    #[test]
    fn strip_ansi_passthrough_plain_text() {
        assert_eq!(strip_ansi("hello world"), "hello world");
        assert_eq!(strip_ansi(""), "");
        assert_eq!(strip_ansi("no escapes here\n"), "no escapes here\n");
    }

    #[test]
    fn strip_ansi_removes_sgr_codes() {
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
        assert_eq!(strip_ansi("\x1b[34mblue\x1b[0m"), "blue");
        assert_eq!(strip_ansi("\x1b[1mbold\x1b[22m"), "bold");
    }

    #[test]
    fn strip_ansi_removes_cursor_position_codes() {
        assert_eq!(strip_ansi("\x1b[2J\x1b[Hclear"), "clear");
        assert_eq!(strip_ansi("\x1b[10;20Hmove"), "move");
    }

    #[test]
    fn strip_ansi_preserves_newlines() {
        assert_eq!(
            strip_ansi("\x1b[31mline1\nline2\x1b[0m\n"),
            "line1\nline2\n"
        );
    }

    // --- strip_ansi property tests ---

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn strip_ansi_idempotent(s in ".*") {
            let once = strip_ansi(&s);
            let twice = strip_ansi(&once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn strip_ansi_output_never_contains_csi_letter(s in ".*") {
            let stripped = strip_ansi(&s);
            // The regex removes \x1b[[0-9;]*[a-zA-Z]. After stripping, no
            // such sequence should remain.
            // Safe: hardcoded regex pattern is valid; will never fail to compile
            let re = regex::Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap();
            prop_assert!(
                !re.is_match(&stripped),
                "stripped output still contains a CSI escape: {:?}",
                stripped
            );
        }

        #[test]
        fn strip_ansi_plain_text_unchanged(s in "[a-zA-Z0-9 \n.,;:!?()]+") {
            let stripped = strip_ansi(&s);
            prop_assert_eq!(stripped, s);
        }

        #[test]
        fn strip_ansi_preserves_length_of_plain(s in "[a-zA-Z0-9 ]+") {
            let stripped = strip_ansi(&s);
            prop_assert_eq!(stripped.len(), s.len());
        }
    }

    // --- update_cursor ---

    #[test]
    fn tracks_cursor_after_newline() {
        let _lock = CURSOR_LOCK.lock().unwrap();
        let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
        // Start with cursor at beginning
        console.lock().unwrap().terminal_cursor_at_beginning = true;

        // Output without trailing newline → cursor not at beginning
        update_cursor(b"hello");
        assert!(!console.lock().unwrap().terminal_cursor_at_beginning);

        // Output with trailing newline → cursor at beginning
        update_cursor(b"world\n");
        assert!(console.lock().unwrap().terminal_cursor_at_beginning);
    }

    #[test]
    fn normalizes_crlf_to_lf_for_cursor_tracking() {
        let _lock = CURSOR_LOCK.lock().unwrap();
        let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
        console.lock().unwrap().terminal_cursor_at_beginning = true;

        // CRLF without trailing newline → cursor NOT at beginning
        update_cursor(b"hello\r\nworld");
        assert!(!console.lock().unwrap().terminal_cursor_at_beginning);

        // CRLF with trailing newline → cursor at beginning
        update_cursor(b"hello\r\nworld\n");
        assert!(console.lock().unwrap().terminal_cursor_at_beginning);
    }

    #[test]
    fn empty_bytes_do_not_change_cursor() {
        let _lock = CURSOR_LOCK.lock().unwrap();
        let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
        console.lock().unwrap().terminal_cursor_at_beginning = true;
        update_cursor(b"");
        assert!(console.lock().unwrap().terminal_cursor_at_beginning);
    }

    #[test]
    fn ansi_in_output_does_not_confuse_cursor_tracking() {
        let _lock = CURSOR_LOCK.lock().unwrap();
        let console = CONSOLE.get_or_init(|| Mutex::new(ConsoleState::default()));
        // Reset cursor state for deterministic test
        console.lock().unwrap().terminal_cursor_at_beginning = false;

        // ANSI codes before newline — cursor should detect the newline
        update_cursor(b"\x1b[31mhello\x1b[0m\n");
        assert!(
            console.lock().unwrap().terminal_cursor_at_beginning,
            "ANSI-wrapped text with trailing newline should set cursor to beginning"
        );

        // ANSI codes without newline
        update_cursor(b"\x1b[34mhello\x1b[0m");
        assert!(
            !console.lock().unwrap().terminal_cursor_at_beginning,
            "ANSI-wrapped text without newline should leave cursor mid-line"
        );
    }

    // --- suppress flags ---

    #[test]
    fn set_suppress_stdout_is_reflected_in_console_state() {
        set_suppress_stdout(true);
        assert!(SUPPRESS_STDOUT.load(Ordering::SeqCst));
        set_suppress_stdout(false);
        assert!(!SUPPRESS_STDOUT.load(Ordering::SeqCst));
    }

    #[test]
    fn set_suppress_stderr_is_reflected_in_console_state() {
        set_suppress_stderr(true);
        assert!(SUPPRESS_STDERR.load(Ordering::SeqCst));
        set_suppress_stderr(false);
        assert!(!SUPPRESS_STDERR.load(Ordering::SeqCst));
    }

    // --- interrupted flag ---

    #[test]
    fn interrupted_flag_set_and_clear_cycle() {
        // Should start false
        assert!(!interrupted_flag(false));

        // Set it
        set_interrupted_flag();
        assert!(interrupted_flag(false));

        // Clear on read
        assert!(interrupted_flag(true));
        assert!(!interrupted_flag(false));
    }

    #[test]
    fn interrupted_flag_survives_non_clearing_reads() {
        // Start from a known state: clear any prior interrupt.
        let _ = interrupted_flag(true);
        assert!(!interrupted_flag(false));

        // Set once, read multiple times without clearing — should stay true.
        set_interrupted_flag();
        assert!(interrupted_flag(false));
        assert!(interrupted_flag(false));
        assert!(interrupted_flag(false));

        // Setting again is idempotent — still true, no spurious false.
        set_interrupted_flag();
        assert!(interrupted_flag(false));

        // Clear exactly once, then back to false.
        assert!(interrupted_flag(true));
        assert!(!interrupted_flag(false));
    }

    #[test]
    fn interrupted_flag_concurrent_set_and_clear_is_consistent() {
        // This test verifies the flag behaves as a simple atomic toggle under
        // repeated set/clear cycles without relying on R or signal delivery.
        for _ in 0..100 {
            // Clear any stale state.
            let _ = interrupted_flag(true);
            assert!(!interrupted_flag(false), "flag should be clear after reset");

            // Set and immediately clear.
            set_interrupted_flag();
            assert!(interrupted_flag(true), "flag should be true after set");
            assert!(!interrupted_flag(false), "flag should be false after clear");
        }
    }

    // --- browse_level ---

    #[test]
    fn browse_level_extracts_level() {
        assert_eq!(browse_level("Browse[1]> "), Some("1".to_string()));
        assert_eq!(browse_level("Browse[42]> "), Some("42".to_string()));
    }

    #[test]
    fn browse_level_returns_none_for_non_browse_prompts() {
        assert_eq!(browse_level("> "), None);
        assert_eq!(browse_level("r$> "), None);
        assert_eq!(browse_level(""), None);
        assert_eq!(browse_level("Continue> "), None);
    }

    // --- display_prompt for shell mode ---

    #[test]
    fn display_shell_prompt() {
        let s = test_settings();
        let displayed = display_prompt("\x1b[31m#!>\x1b[0m ", &s);
        assert_eq!(displayed, "\x1b[31m#!>\x1b[0m ");
    }

    #[test]
    fn shell_command_detects_one_shot_and_persistent_activation() {
        assert_eq!(shell_command(";pwd\n"), Some("pwd"));
        assert_eq!(shell_command(";\n"), Some(""));
        assert_eq!(shell_command("1 + 1\n"), None);
    }

    // --- text_looks_complete ---

    #[test]
    fn detects_obvious_multiline_input() {
        assert!(!text_looks_complete("1 +\n"));
        assert!(!text_looks_complete("function() {\n"));
        assert!(text_looks_complete("1 +\n1\n"));
        assert!(text_looks_complete("function() {\n1\n}\n"));
    }

    // --- is_r_or_browse_prompt ---

    #[test]
    fn r_or_browse_prompt_true_for_r() {
        let s = test_settings();
        assert!(is_r_or_browse_prompt("> ", &s));
    }

    #[test]
    fn r_or_browse_prompt_true_for_browse() {
        let s = test_settings();
        assert!(is_r_or_browse_prompt("Browse[3]> ", &s));
    }

    #[test]
    fn r_or_browse_prompt_false_for_unknown() {
        let s = test_settings();
        assert!(!is_r_or_browse_prompt("? ", &s));
    }

    #[test]
    fn prompt_active_uses_native_read_route() {
        assert_eq!(console_read_route(true, true), ConsoleReadRoute::Native);
        assert_eq!(console_read_route(true, false), ConsoleReadRoute::Native);
    }

    #[test]
    fn inactive_prompt_uses_terminal_shape() {
        assert_eq!(
            console_read_route(false, true),
            ConsoleReadRoute::Interactive
        );
        assert_eq!(console_read_route(false, false), ConsoleReadRoute::Piped);
    }

    #[test]
    fn terminal_width_clamps_to_minimum() {
        let mut last = None;
        let settings = test_settings();
        assert_eq!(terminal_width_update(&mut last, &settings, 10), Some(20));
        assert_eq!(last, Some(20));
    }

    #[test]
    fn unchanged_terminal_width_skips_update() {
        let mut last = Some(80);
        let settings = test_settings();
        assert_eq!(terminal_width_update(&mut last, &settings, 80), None);
        assert_eq!(last, Some(80));
    }

    #[test]
    fn changed_terminal_width_requests_update() {
        let mut last = Some(80);
        let settings = test_settings();
        assert_eq!(terminal_width_update(&mut last, &settings, 120), Some(120));
        assert_eq!(last, Some(120));
    }

    #[test]
    fn disabled_auto_width_skips_update() {
        let mut last = Some(80);
        let mut settings = test_settings();
        settings.auto_width = false;
        assert_eq!(terminal_width_update(&mut last, &settings, 120), None);
        assert_eq!(last, Some(80));
    }

    #[test]
    fn console_input_short_ascii_stays_one_chunk() {
        assert_eq!(
            prepare_console_input("1 + 1", &PromptMode::R, 100),
            "1 + 1\n"
        );
        assert_eq!(
            split_console_input("1 + 1\n", 100),
            ("1 + 1\n".to_string(), String::new())
        );
    }

    #[test]
    fn console_input_long_ascii_splits_and_drains() {
        let (head, _tail) = split_console_input("abcdef\n", 3);
        assert_eq!(head, "abc");
    }

    #[test]
    fn question_detects_single_at_line_start() {
        let text = "?lm\n";
        let rest = text.trim_start().strip_prefix('?').unwrap();
        assert!(!rest.starts_with('?'));
        let query = rest.trim_end_matches('\n');
        assert_eq!(query, "lm");
    }

    #[test]
    fn question_detects_double_at_line_start() {
        let text = "??lm\n";
        let rest = text.trim_start().strip_prefix('?').unwrap();
        assert!(rest.starts_with('?'));
        let double_rest = rest.strip_prefix('?').unwrap();
        let query = double_rest.trim_end_matches('\n');
        assert_eq!(query, "lm");
    }

    #[test]
    fn question_bare_question_shows_usage() {
        let text = "?\n";
        let rest = text.trim_start().strip_prefix('?').unwrap();
        let query = rest.trim_end_matches('\n');
        assert!(query.is_empty());
    }

    #[test]
    fn question_not_at_line_start_ignored() {
        let text = "x ? y\n";
        assert!(text.trim_start().strip_prefix('?').is_none());
    }

    #[test]
    fn question_with_leading_whitespace_still_detected() {
        let text = "  ?lm\n";
        assert!(text.trim_start().strip_prefix('?').is_some());
    }
}
