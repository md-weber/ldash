mod app;
mod config;
mod data;
mod events;
mod ui;
mod watcher;

use anyhow::{Context, Result};
use app::App;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetTitle,
    },
    ExecutableCommand,
};
use events::{handle_key, Action};
use ratatui::prelude::*;
use std::{
    io,
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Terminal dashboard for hledger journals.
#[derive(Parser, Debug)]
#[command(
    name = "ldash",
    version = VERSION,
    about = "Terminal dashboard for hledger journals",
    long_about = None,
    disable_help_subcommand = true,
    disable_version_flag = true,
)]
struct Cli {
    /// Print version and exit.
    #[arg(short = 'v', long = "version", action = clap::ArgAction::Version)]
    version: (),

    /// Path to hledger journal file (overrides config and $LEDGER_FILE).
    #[arg(value_name = "FILE")]
    file_pos: Option<PathBuf>,

    /// Same as positional FILE (matches hledger's -f convention).
    #[arg(short = 'f', long = "file", value_name = "FILE")]
    file: Option<PathBuf>,

    /// Path to config file (default: ~/.config/ldash/config.toml).
    #[arg(short = 'c', long = "config", value_name = "PATH")]
    config: Option<PathBuf>,

    /// Start on tab: dashboard, portfolio, accounts, monthly, register (overrides config).
    #[arg(long = "tab", value_name = "TAB", value_parser = parse_tab)]
    tab: Option<String>,

    /// Validate journal + config and exit (no TUI).
    #[arg(long = "check")]
    check: bool,
}

fn parse_tab(s: &str) -> Result<String, String> {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "dashboard" | "start" | "portfolio" | "accounts" | "monthly" | "register" => Ok(lower),
        _ => Err(format!(
            "invalid tab '{s}' (expected: dashboard, portfolio, accounts, monthly, register)"
        )),
    }
}

const EXIT_OK: u8 = 0;
const EXIT_CONFIG: u8 = 1;
const EXIT_NO_HLEDGER: u8 = 2;

pub(crate) fn copy_to_clipboard(text: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("pbcopy");
    #[cfg(not(target_os = "macos"))]
    let mut cmd = {
        let mut c = std::process::Command::new("xclip");
        c.args(["-selection", "clipboard"]);
        c
    };

    let mut child = cmd.stdin(std::process::Stdio::piped()).spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(text.as_bytes())?;
    }
    child.wait()?;
    Ok(())
}

fn find_journal(cli: &Cli, config: &config::Config) -> Result<PathBuf> {
    let cli_file = cli.file.as_ref().or(cli.file_pos.as_ref());
    if let Some(p) = cli_file {
        if p.exists() {
            return Ok(p.clone());
        }
        anyhow::bail!("Journal file not found: {}", p.display());
    }

    if let Some(ref cfg_path) = config.journal {
        let path = PathBuf::from(cfg_path);
        if path.exists() {
            return Ok(path);
        }
        anyhow::bail!("Journal file from config not found: {cfg_path}");
    }

    if let Ok(env_path) = std::env::var("LEDGER_FILE") {
        let path = PathBuf::from(&env_path);
        if path.exists() {
            return Ok(path);
        }
        anyhow::bail!("Journal file from $LEDGER_FILE not found: {env_path}");
    }

    let candidates = ["Finance/all.journal", "all.journal"];

    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Ok(p);
        }
    }

    anyhow::bail!(
        "Could not find journal file.\n\
         Pass one with `ldash -f /path/to/all.journal`,\n\
         set $LEDGER_FILE, or run from a directory containing all.journal."
    )
}

fn check_hledger() -> Result<()> {
    std::process::Command::new("hledger")
        .arg("--version")
        .output()
        .context("hledger not found in PATH. Install it from https://hledger.org")?;
    Ok(())
}

fn run_check(cli: &Cli) -> Result<()> {
    let config = if cli.config.is_some() {
        config::Config::load_strict().context("config validation failed")?
    } else {
        let (cfg, warnings) = config::Config::load();
        for w in &warnings {
            eprintln!("warning: {w}");
        }
        if !warnings.is_empty() {
            anyhow::bail!("config validation failed");
        }
        cfg
    };

    let journal = find_journal(cli, &config)?;
    println!("ok: config loaded");
    println!("ok: journal found at {}", journal.display());
    println!("ok: hledger present");
    Ok(())
}

fn print_err(err: &anyhow::Error) {
    eprintln!("ldash: {err}");
    let mut src = err.source();
    while let Some(s) = src {
        eprintln!("  caused by: {s}");
        src = s.source();
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Some(ref p) = cli.config {
        config::set_config_path_override(p.clone());
    }

    if let Err(e) = check_hledger() {
        print_err(&e);
        return ExitCode::from(EXIT_NO_HLEDGER);
    }

    if cli.check {
        return match run_check(&cli) {
            Ok(()) => ExitCode::from(EXIT_OK),
            Err(e) => {
                print_err(&e);
                ExitCode::from(EXIT_CONFIG)
            }
        };
    }

    let (mut config, config_warnings) = config::Config::load();

    if let Some(ref tab) = cli.tab {
        config.default_tab = tab.clone();
    }

    let journal_path = match find_journal(&cli, &config) {
        Ok(p) => p,
        Err(e) => {
            print_err(&e);
            return ExitCode::from(EXIT_CONFIG);
        }
    };

    match run_tui(journal_path, config, config_warnings) {
        Ok(()) => ExitCode::from(EXIT_OK),
        Err(e) => {
            print_err(&e);
            ExitCode::from(EXIT_CONFIG)
        }
    }
}

fn run_tui(
    journal_path: PathBuf,
    config: config::Config,
    config_warnings: Vec<String>,
) -> Result<()> {
    enable_raw_mode()?;

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = io::stdout().execute(DisableMouseCapture);
        let _ = io::stdout().execute(LeaveAlternateScreen);
        original_hook(info);
    }));

    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    // swallow error on dumb terminals that don't support mouse
    let _ = stdout.execute(EnableMouseCapture);
    stdout.execute(SetTitle("Ledger Dashboard"))?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, journal_path, config, config_warnings);

    let _ = disable_raw_mode();
    let _ = io::stdout().execute(DisableMouseCapture);
    let _ = io::stdout().execute(LeaveAlternateScreen);

    result
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    journal_path: PathBuf,
    mut config: config::Config,
    config_warnings: Vec<String>,
) -> Result<()> {
    // Auto-detect the journal's primary fiat currency and commodity format.
    // When the configured symbol doesn't match what the journal actually uses
    // (e.g. default "€" against a USD journal), every parsed amount would be
    // silently dropped. Detecting up-front keeps the app usable without
    // requiring the user to touch their config for simple single-currency
    // journals.
    //
    // We also detect whether the journal uses prefix notation ("Eur 100") and,
    // when it does, adopt the raw display form and prefix flag — unless the
    // user has explicitly configured those fields.
    let currency_warning = if let Some(det) = data::detect_journal_currency(&journal_path) {
        // Apply detected prefix format when the user hasn't overridden it.
        if det.prefix && !config.currency_prefix {
            config.currency_prefix = true;
        }
        if !data::currency_matches(&config.currency_symbol, &det.canonical) {
            // Symbol mismatch: replace with detected display form.
            let old = std::mem::replace(&mut config.currency_symbol, det.display.clone());
            Some(format!(
                "Currency auto-detected: {} (was {}). Set currency_symbol in config to override.",
                config.currency_symbol, old
            ))
        } else if det.prefix && config.currency_symbol == config::Config::default().currency_symbol
        {
            // Same canonical currency but prefix format — adopt the raw form
            // (e.g. "Eur") so it mirrors the journal's own notation.
            config.currency_symbol = det.display;
            None
        } else {
            None
        }
    } else {
        None
    };

    let mut app = App::new(journal_path, config).context("Failed to initialize app")?;
    let startup_msg = currency_warning.or_else(|| config_warnings.last().cloned());
    if let Some(w) = startup_msg {
        app.status_msg = w;
    }

    terminal.draw(|f| ui::render(f, &mut app))?;
    app.ensure_tab_loaded(app.tab);
    app.maybe_auto_fetch_prices();

    let tick = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        app.check_refresh();
        app.check_background();
        app.check_alert_timeout();

        terminal.draw(|f| ui::render(f, &mut app))?;

        let timeout = tick.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            let ev = event::read()?;
            if matches!(ev, Event::Resize(_, _)) {
                continue;
            }
            if let Event::Mouse(mouse) = ev {
                app.handle_mouse(mouse);
                continue;
            }

            if let Event::Key(key) = ev {
                if let Action::Quit = handle_key(&mut app, key) {
                    break;
                }
            }
        }

        if last_tick.elapsed() >= tick {
            last_tick = Instant::now();
        }

        let check_interval = if app.watcher.is_some() {
            Duration::from_secs(1)
        } else {
            app.config.refresh_duration()
        };
        if app.last_refresh.elapsed() >= check_interval {
            app.auto_refresh();
        }
    }

    Ok(())
}
