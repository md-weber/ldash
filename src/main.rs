mod app;
mod config;
mod data;
mod ui;
mod watcher;

use anyhow::{Context, Result};
use app::App;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetTitle,
    },
    ExecutableCommand,
};
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

    /// Start on tab: portfolio, accounts, monthly (overrides config).
    #[arg(long = "tab", value_name = "TAB", value_parser = parse_tab)]
    tab: Option<String>,

    /// Validate journal + config and exit (no TUI).
    #[arg(long = "check")]
    check: bool,
}

fn parse_tab(s: &str) -> Result<String, String> {
    match s {
        "portfolio" | "accounts" | "monthly" => Ok(s.to_string()),
        _ => Err(format!(
            "invalid tab '{s}' (expected: portfolio, accounts, monthly)"
        )),
    }
}

const EXIT_OK: u8 = 0;
const EXIT_CONFIG: u8 = 1;
const EXIT_NO_HLEDGER: u8 = 2;

fn copy_to_clipboard(text: &str) -> Result<()> {
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

    let candidates = [
        "tmp/Finance/all.journal",
        "Finance/all.journal",
        "all.journal",
    ];

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
    config: config::Config,
    config_warnings: Vec<String>,
) -> Result<()> {
    let mut app = App::new(journal_path, config).context("Failed to initialize app")?;
    if let Some(w) = config_warnings.last() {
        app.status_msg = w.clone();
    }

    terminal.draw(|f| ui::render(f, &mut app))?;
    app.ensure_tab_loaded(app.tab);

    let tick = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        app.check_refresh();
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
                if key.kind == KeyEventKind::Press {
                    if app.export_prompt_active {
                        match key.code {
                            KeyCode::Esc => app.cancel_export_prompt(),
                            KeyCode::Enter => {
                                let path = app.export_prompt_path.clone();
                                app.export_prompt_active = false;
                                match app.export_to_file(&path) {
                                    Ok(written) => app.status_msg = format!("Exported → {written}"),
                                    Err(e) => app.status_msg = format!("Export error: {e}"),
                                }
                            }
                            KeyCode::Backspace => {
                                app.export_prompt_path.pop();
                            }
                            KeyCode::Char(c) => app.export_prompt_path.push(c),
                            _ => {}
                        }
                    } else if app.search_active {
                        match key.code {
                            KeyCode::Esc => app.close_search(),
                            KeyCode::Enter => app.execute_search(),
                            KeyCode::Backspace => {
                                app.search_query.pop();
                            }
                            KeyCode::Char(c) => app.search_query.push(c),
                            KeyCode::Up => app.search_scroll_up(),
                            KeyCode::Down => app.search_scroll_down(),
                            _ => {}
                        }
                    } else if app.show_alerts {
                        app.show_alerts = false;
                        app.alert_dismissed = true;
                        continue;
                    } else if app.show_help {
                        match key.code {
                            KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => {
                                app.show_help = false;
                            }
                            _ => {}
                        }
                    } else if app.account_filter_active {
                        match key.code {
                            KeyCode::Esc => {
                                if app.account_detail.is_some() {
                                    app.close_account_detail();
                                } else {
                                    app.close_account_filter();
                                }
                            }
                            KeyCode::Backspace => app.account_filter_backspace(),
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.has_open_detail() {
                                    app.detail_scroll_up();
                                } else {
                                    app.scroll_up();
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.has_open_detail() {
                                    app.detail_scroll_down();
                                } else {
                                    app.scroll_down();
                                }
                            }
                            KeyCode::Enter => app.open_account_detail(),
                            KeyCode::Char(c) => app.account_filter_push(c),
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Esc => {
                                if app.account_detail.is_some() {
                                    app.close_account_detail();
                                } else if app.income_detail.is_some() {
                                    app.close_income_detail();
                                } else if app.expense_detail.is_some() {
                                    app.close_expense_detail();
                                } else if !app.account_filter.is_empty() {
                                    app.close_account_filter();
                                } else {
                                    break;
                                }
                            }
                            KeyCode::Enter => match app.tab {
                                app::Tab::Accounts => app.open_account_detail(),
                                app::Tab::Monthly => match app.monthly_focus {
                                    app::MonthlyFocus::Income => app.open_income_detail(),
                                    app::MonthlyFocus::Expenses => app.open_expense_detail(),
                                },
                                _ => {}
                            },
                            KeyCode::Char('i') if app.tab == app::Tab::Monthly => {
                                app.toggle_monthly_focus()
                            }
                            KeyCode::Char('/') => app.open_search(),
                            KeyCode::Char('?') => app.show_help = true,
                            KeyCode::Tab => app.next_tab(),
                            KeyCode::BackTab => app.prev_tab(),
                            KeyCode::Char('1') => app.select_tab(0),
                            KeyCode::Char('2') => app.select_tab(1),
                            KeyCode::Char('3') => app.select_tab(2),
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.has_open_detail() {
                                    app.detail_scroll_up();
                                } else {
                                    app.scroll_up();
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.has_open_detail() {
                                    app.detail_scroll_down();
                                } else {
                                    app.scroll_down();
                                }
                            }
                            KeyCode::PageUp => app.scroll_page_up(),
                            KeyCode::PageDown => app.scroll_page_down(),
                            KeyCode::Home => app.scroll_home(),
                            KeyCode::End => app.scroll_end(),
                            KeyCode::Left | KeyCode::Char('h') => match app.tab {
                                app::Tab::Portfolio => app.portfolio_range_left(),
                                app::Tab::Accounts => app.nw_range_left(),
                                app::Tab::Monthly => app.month_left(),
                            },
                            KeyCode::Right | KeyCode::Char('l') => match app.tab {
                                app::Tab::Portfolio => app.portfolio_range_right(),
                                app::Tab::Accounts => app.nw_range_right(),
                                app::Tab::Monthly => app.month_right(),
                            },
                            KeyCode::Char('y') if app.tab == app::Tab::Monthly => {
                                app.cycle_year_back();
                            }
                            KeyCode::Char('Y') => {
                                if app.tab == app::Tab::Monthly {
                                    app.cycle_year_forward();
                                } else {
                                    let data = app.export_current_view();
                                    match copy_to_clipboard(&data) {
                                        Ok(()) => {
                                            app.status_msg = "Copied to clipboard".to_string()
                                        }
                                        Err(e) => app.status_msg = format!("Clipboard error: {e}"),
                                    }
                                }
                            }
                            KeyCode::Char('e') => app.open_export_prompt(),
                            KeyCode::Char('s') => app.chart_stacked = !app.chart_stacked,
                            KeyCode::Char('c') => app.expense_colors = !app.expense_colors,
                            KeyCode::Char('r') => app.start_refresh(),
                            KeyCode::Char(c)
                                if app.tab == app::Tab::Accounts
                                    && !matches!(
                                        c,
                                        'q' | '/'
                                            | '?'
                                            | 'r'
                                            | 's'
                                            | 'c'
                                            | 'y'
                                            | 'Y'
                                            | 'e'
                                            | '1'
                                            | '2'
                                            | '3'
                                    ) =>
                            {
                                app.account_filter_active = true;
                                app.account_filter.push(c);
                                app.account_state.select(Some(0));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick {
            last_tick = Instant::now();
        }

        let check_interval = if app.has_watcher() {
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
