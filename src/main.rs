mod app;
mod data;
mod ui;

use anyhow::{Context, Result};
use app::App;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use std::{
    io,
    path::PathBuf,
    time::{Duration, Instant},
};

fn find_journal() -> Result<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let path = PathBuf::from(&args[1]);
        if path.exists() {
            return Ok(path);
        }
        anyhow::bail!("Journal file not found: {}", args[1]);
    }

    if let Ok(env_path) = std::env::var("LEDGER_FILE") {
        let path = PathBuf::from(&env_path);
        if path.exists() {
            return Ok(path);
        }
        anyhow::bail!("Journal file from $LEDGER_FILE not found: {}", env_path);
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
        "Could not find journal file.\nUsage: ldash /path/to/all.journal\n\
         Or set $LEDGER_FILE, or run from a directory containing all.journal"
    )
}

fn check_hledger() -> Result<()> {
    std::process::Command::new("hledger")
        .arg("--version")
        .output()
        .context("hledger not found in PATH. Install it from https://hledger.org")?;
    Ok(())
}

fn main() -> Result<()> {
    let journal_path = find_journal().context("Journal file lookup failed")?;
    check_hledger()?;

    enable_raw_mode()?;

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = io::stdout().execute(LeaveAlternateScreen);
        original_hook(info);
    }));

    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, journal_path);

    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, journal_path: PathBuf) -> Result<()> {
    let mut app = App::new(journal_path).context("Failed to load data")?;

    let tick = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui::render(f, &mut app))?;

        let timeout = tick.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            let ev = event::read()?;
            if matches!(ev, Event::Resize(_, _)) {
                continue;
            }
            if let Event::Key(key) = ev {
                if key.kind == KeyEventKind::Press {
                    if app.show_help {
                        match key.code {
                            KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => {
                                app.show_help = false;
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Esc => {
                                if app.account_detail.is_some() {
                                    app.close_account_detail();
                                } else {
                                    break;
                                }
                            }
                            KeyCode::Enter => app.open_account_detail(),
                            KeyCode::Char('?') => app.show_help = true,
                            KeyCode::Tab => app.next_tab(),
                            KeyCode::BackTab => app.prev_tab(),
                            KeyCode::Char('1') => app.select_tab(0),
                            KeyCode::Char('2') => app.select_tab(1),
                            KeyCode::Char('3') => app.select_tab(2),
                            KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
                            KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
                            KeyCode::Left | KeyCode::Char('h') => app.month_left(),
                            KeyCode::Right | KeyCode::Char('l') => app.month_right(),
                            KeyCode::Char('c') => app.expense_colors = !app.expense_colors,
                            KeyCode::Char('r') => {
                                if let Err(e) = app.refresh() {
                                    app.status_msg = format!("Refresh error: {e}");
                                }
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

        if app.last_refresh.elapsed() >= Duration::from_secs(300) {
            match app.refresh() {
                Ok(()) => {
                    let now = chrono::Local::now().format("%H:%M:%S");
                    app.status_msg = format!("Auto-refreshed at {now}");
                }
                Err(e) => {
                    app.status_msg = format!("Auto-refresh error: {e}");
                }
            }
        }
    }

    Ok(())
}
