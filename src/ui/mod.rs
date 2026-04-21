use ratatui::{prelude::*, widgets::*};

use crate::app::{App, Tab};
use crate::config::Config;

mod accounts;
mod monthly;
mod portfolio;

// ── Color palette ────────────────────────────────────────────────────────────
pub(super) const ACCENT: Color = Color::Cyan;
pub(super) const GREEN: Color = Color::Green;
pub(super) const RED: Color = Color::Red;
pub(super) const GOLD: Color = Color::Yellow;
pub(super) const MUTED: Color = Color::DarkGray;
pub(super) const FG: Color = Color::White;

pub(super) fn parse_color(s: &str) -> Option<Color> {
    match s.to_lowercase().as_str() {
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "blue" => Some(Color::Blue),
        "yellow" => Some(Color::Yellow),
        "cyan" => Some(Color::Cyan),
        "magenta" => Some(Color::Magenta),
        "white" => Some(Color::White),
        "darkgray" => Some(Color::DarkGray),
        s if s.starts_with('#') && s.len() == 7 => {
            let r = u8::from_str_radix(&s[1..3], 16).ok()?;
            let g = u8::from_str_radix(&s[3..5], 16).ok()?;
            let b = u8::from_str_radix(&s[5..7], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

pub(super) fn expense_color(category: &str, app: &App) -> Color {
    if let Some(color_str) = app.config.colors.expenses.get(category) {
        if let Some(c) = parse_color(color_str) {
            return c;
        }
    }

    let cat = category.to_lowercase();
    let top = cat.split(':').next().unwrap_or(&cat);
    match top {
        "wohnen" | "housing" | "hauskauf" => Color::Blue,
        "essen" | "food" | "groceries" | "restaurant" => GOLD,
        "transport" | "car" | "fuel" => Color::Magenta,
        "gesundheit" | "health" | "hygiene" => Color::LightCyan,
        "versicherung" | "insurance" => Color::LightBlue,
        "kommunikation" | "telecom" | "haushalt" => Color::Rgb(180, 140, 255),
        "freizeit" | "entertainment" | "urlaub" => Color::LightGreen,
        "kleider" | "kinder" | "shopping" | "clothing" => Color::Rgb(255, 150, 80),
        "fortbildung" | "education" | "books" => Color::Cyan,
        "steuer" | "tax" | "fees" | "crypto" => Color::Rgb(200, 200, 100),
        "abos" | "amazon" => Color::Rgb(255, 120, 200),
        "spende" | "schenkung" => Color::Rgb(150, 220, 180),
        _ => FG,
    }
}

pub(super) fn coin_color(commodity: &str) -> Color {
    const PALETTE: &[Color] = &[
        GOLD,
        ACCENT,
        GREEN,
        Color::Magenta,
        Color::LightBlue,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
    ];
    let hash: usize = commodity.bytes().map(|b| b as usize).sum();
    PALETTE[hash % PALETTE.len()]
}

fn nice_step(range: f64, ticks: usize) -> f64 {
    let raw = range / ticks as f64;
    let mag = 10.0_f64.powf(raw.log10().floor());
    let frac = raw / mag;
    let nice = if frac <= 1.0 {
        1.0
    } else if frac <= 2.0 {
        2.0
    } else if frac <= 2.5 {
        2.5
    } else if frac <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * mag
}

pub(super) fn nice_y_axis(
    y_min_raw: f64,
    y_max_raw: f64,
    ticks: usize,
    cfg: &Config,
) -> (f64, f64, Vec<Span<'static>>) {
    let range = (y_max_raw - y_min_raw).max(1.0);
    let step = nice_step(range, ticks);
    let lo = (y_min_raw / step).floor() * step;
    let hi = (y_max_raw / step).ceil() * step;
    let n = ((hi - lo) / step).round() as usize;
    let labels = (0..=n)
        .map(|i| {
            let v = lo + step * i as f64;
            let s = if v.abs() >= 100.0 {
                cfg.fmt_amount_compact(v, 0)
            } else {
                cfg.fmt_amount_compact(v, 1)
            };
            Span::styled(s, Style::default().fg(MUTED))
        })
        .collect();
    (lo, hi, labels)
}

// ── Shared helper used by accounts and monthly ────────────────────────────────

pub(super) fn render_detail_with_title(
    f: &mut Frame,
    txns: &[crate::data::Transaction],
    title: &str,
    cfg: &Config,
    area: Rect,
) {
    let rows: Vec<Row> = txns
        .iter()
        .map(|t| {
            let amt_color = if t.amount >= 0.0 { GREEN } else { RED };
            Row::new(vec![
                Cell::from(t.date.format("%Y-%m-%d").to_string()).style(Style::default().fg(MUTED)),
                Cell::from(t.description.clone()).style(Style::default().fg(FG)),
                Cell::from(format!("{:>13}", cfg.fmt_amount(t.amount, 2)))
                    .style(Style::default().fg(amt_color)),
                Cell::from(format!("{:>13}", cfg.fmt_amount(t.running_total, 2)))
                    .style(Style::default().fg(ACCENT)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(12),
        Constraint::Min(24),
        Constraint::Length(14),
        Constraint::Length(14),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["Date", "Description", "Amount", "Balance"])
                .style(Style::default().fg(MUTED).bold())
                .bottom_margin(1),
        )
        .block(
            Block::default()
                .title(Span::styled(title, Style::default().fg(GOLD).bold()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(ACCENT)),
        );

    f.render_widget(table, area);
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let layout = Layout::vertical([
        Constraint::Length(1), // title bar
        Constraint::Length(3), // tab bar
        Constraint::Min(0),    // content
        Constraint::Length(1), // status bar
    ])
    .split(area);

    render_title(f, layout[0]);
    render_tabs(f, app, layout[1]);
    render_content(f, app, layout[2]);

    if app.search_active {
        render_search_overlay(f, app, layout[2]);
    }

    if app.show_alerts {
        render_price_alerts(f, app, layout[2]);
    }

    render_status(f, app, layout[3]);

    if app.loading {
        render_loading_overlay(f, area);
    }

    if app.show_help {
        render_help_popup(f, area);
    }
}

// ── Title bar ─────────────────────────────────────────────────────────────────

fn render_title(f: &mut Frame, area: Rect) {
    let title = Paragraph::new(Line::from(vec![
        Span::raw("  "),
        Span::styled("⬡", Style::default().fg(GOLD)),
        Span::styled(" Ledger Dashboard", Style::default().fg(Color::White).bold()),
        Span::styled(
            format!(" v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(MUTED),
        ),
        Span::raw("  "),
    ]))
    .style(Style::default().bg(Color::Rgb(20, 20, 30)));
    f.render_widget(title, area);
}

// ── Tab bar ───────────────────────────────────────────────────────────────────

fn render_tabs(f: &mut Frame, app: &mut App, area: Rect) {
    let visible = app.visible_tabs();
    let labels: Vec<String> = visible
        .iter()
        .map(|t| match t {
            Tab::Accounts => "  Accounts  ".to_string(),
            Tab::Monthly => "  Monthly  ".to_string(),
            Tab::Portfolio => "  Portfolio  ".to_string(),
        })
        .collect();
    let selected = visible
        .iter()
        .position(|&t| t == app.tab)
        .unwrap_or(0);

    // record geometry for mouse hit-testing
    app.tab_bar_area = area;
    app.tab_rects.clear();
    // labels are rendered inside the block border; x starts at area.x + 1
    // each label is separated by the default Tabs divider "|" (1 char)
    let mut x = area.x + 1;
    for label in &labels {
        let w = label.chars().count() as u16;
        app.tab_rects.push(Rect { x, y: area.y + 1, width: w, height: 1 });
        x += w + 1; // label width + divider
    }

    let tabs = Tabs::new(labels)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(MUTED))
                .border_type(BorderType::Rounded),
        )
        .select(selected)
        .style(Style::default().fg(MUTED))
        .highlight_style(
            Style::default()
                .fg(ACCENT)
                .bold()
                .add_modifier(Modifier::UNDERLINED),
        );
    f.render_widget(tabs, area);
}

// ── Content dispatch ──────────────────────────────────────────────────────────

fn render_content(f: &mut Frame, app: &mut App, area: Rect) {
    match app.tab {
        Tab::Portfolio => portfolio::render_portfolio(f, app, area),
        Tab::Accounts => accounts::render_accounts(f, app, area),
        Tab::Monthly => monthly::render_monthly(f, app, area),
    }
}

// ── Overlays and status ───────────────────────────────────────────────────────

fn render_loading_overlay(f: &mut Frame, area: Rect) {
    let w = 26u16.min(area.width.saturating_sub(4));
    let h = 3u16.min(area.height.saturating_sub(4));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT));
    let text = Paragraph::new(Line::from(vec![
        Span::styled("⏳ ", Style::default().fg(GOLD)),
        Span::styled("Loading data…", Style::default().fg(FG).bold()),
    ]))
    .alignment(Alignment::Center)
    .block(block);
    f.render_widget(text, popup);
}

fn render_help_popup(f: &mut Frame, area: Rect) {
    let w = 66u16.min(area.width.saturating_sub(4));
    let h = 22u16.min(area.height.saturating_sub(4));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };

    f.render_widget(Clear, popup);

    let bindings: &[(&str, &str)] = &[
        ("1 / 2 / 3", "Switch tab"),
        ("Tab / Shift-Tab", "Next / prev tab"),
        ("↑ k / ↓ j", "Scroll / select"),
        ("← h / → l", "Month nav / NW range"),
        ("Enter", "Drill into category detail"),
        ("i", "Toggle income/expense focus (Monthly)"),
        ("/", "Search transactions"),
        ("y / Y", "Year back / forward (Monthly)"),
        ("Y", "Copy view to clipboard (non-Monthly)"),
        ("s", "Toggle chart stacked / unstacked"),
        ("c", "Toggle expense colors"),
        ("r", "Refresh data"),
        ("Esc", "Close detail / back / quit"),
        ("? / q", "Toggle help / quit"),
        ("Mouse click", "Select tab / row"),
        ("Scroll wheel", "Scroll table"),
    ];

    let mut lines = vec![Line::from("")];
    for (key, desc) in bindings {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<20}", key), Style::default().fg(ACCENT).bold()),
            Span::styled(*desc, Style::default().fg(FG)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  P/L shows unrealized gains only.",
        Style::default().fg(MUTED),
    )));

    let block = Block::default()
        .title(Span::styled(
            " Keybindings ",
            Style::default().fg(GOLD).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(Color::Rgb(20, 20, 30)));

    f.render_widget(Paragraph::new(lines).block(block), popup);
}

fn render_search_overlay(f: &mut Frame, app: &mut App, area: Rect) {
    f.render_widget(Clear, area);

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    let input = Paragraph::new(Line::from(vec![
        Span::styled("  / ", Style::default().fg(GOLD).bold()),
        Span::styled(&app.search_query, Style::default().fg(FG)),
        Span::styled("█", Style::default().fg(ACCENT)),
    ]))
    .block(
        Block::default()
            .title(Span::styled(
                " Search Transactions ",
                Style::default().fg(ACCENT).bold(),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT)),
    );
    f.render_widget(input, chunks[0]);

    let rows: Vec<Row> = app
        .search_results
        .iter()
        .map(|t| {
            let amt_color = if t.amount >= 0.0 { GREEN } else { RED };
            Row::new(vec![
                Cell::from(t.date.format("%Y-%m-%d").to_string())
                    .style(Style::default().fg(MUTED)),
                Cell::from(t.description.clone()).style(Style::default().fg(FG)),
                Cell::from(format!("{:>13}", app.config.fmt_amount(t.amount, 2)))
                    .style(Style::default().fg(amt_color)),
                Cell::from(format!("{:>13}", app.config.fmt_amount(t.running_total, 2)))
                    .style(Style::default().fg(ACCENT)),
            ])
        })
        .collect();

    let result_count = app.search_results.len();
    let title = if result_count > 0 {
        format!(" {} results ", result_count)
    } else if app.search_query.is_empty() {
        " Type query, Enter to search ".to_string()
    } else {
        " No results ".to_string()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Min(24),
            Constraint::Length(14),
            Constraint::Length(14),
        ],
    )
    .header(
        Row::new(vec!["Date", "Description", "Amount", "Balance"])
            .style(Style::default().fg(MUTED).bold())
            .bottom_margin(1),
    )
    .row_highlight_style(Style::default().bg(Color::Rgb(40, 40, 60)).bold())
    .highlight_symbol("▶ ")
    .block(
        Block::default()
            .title(Span::styled(title, Style::default().fg(GOLD).bold()))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(MUTED)),
    );

    f.render_stateful_widget(table, chunks[1], &mut app.search_state);

    let hint = Paragraph::new(Line::from(vec![
        Span::styled(
            "  [Enter] search  [↑↓] navigate  [Esc] close",
            Style::default().fg(MUTED),
        ),
        Span::styled(
            "  Supports regex (e.g. \"grocery|supermarket\")",
            Style::default().fg(MUTED),
        ),
    ]));
    f.render_widget(hint, chunks[2]);
}

fn render_price_alerts(f: &mut Frame, app: &App, area: Rect) {
    if !app.show_alerts || app.price_alerts.is_empty() {
        return;
    }

    let h = 3u16.min(area.height);
    let banner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: h,
    };
    f.render_widget(Clear, banner);

    let mut spans = vec![
        Span::styled("  Price moves: ", Style::default().fg(GOLD).bold()),
    ];
    for (i, alert) in app.price_alerts.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(", ", Style::default().fg(MUTED)));
        }
        let (prefix, color) = if alert.change_pct >= 0.0 {
            ("+", GREEN)
        } else {
            ("", RED)
        };
        spans.push(Span::styled(
            format!("{} {prefix}{:.1}%", alert.coin, alert.change_pct),
            Style::default().fg(color).bold(),
        ));
    }
    spans.push(Span::styled("  [any key dismiss]", Style::default().fg(MUTED)));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(GOLD))
        .style(Style::default().bg(Color::Rgb(20, 20, 30)));

    f.render_widget(Paragraph::new(Line::from(spans)).block(block), banner);
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let help = "  [1-3] tab  [↑↓/jk] navigate  [←→/hl] month/range  [r] refresh  [?] help  [q] quit  [mouse] click tab/row";
    let text = Line::from(vec![
        Span::styled(&app.status_msg, Style::default().fg(ACCENT)),
        Span::styled(help, Style::default().fg(MUTED)),
    ]);
    f.render_widget(Paragraph::new(text), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_to_string(app: &mut App) -> String {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let width = buf.area.width;
        let height = buf.area.height;
        let mut rows = Vec::with_capacity(height as usize);
        for y in 0..height {
            let mut row = String::new();
            for x in 0..width {
                row.push_str(buf.cell((x, y)).map_or(" ", |c| c.symbol()));
            }
            rows.push(row.trim_end().to_string());
        }
        rows.join("\n")
    }

    #[test]
    fn snapshot_accounts_empty() {
        let mut app = App::fixture_empty();
        insta::assert_snapshot!(render_to_string(&mut app));
    }

    #[test]
    fn snapshot_accounts_with_data() {
        let mut app = App::fixture_with_accounts();
        insta::assert_snapshot!(render_to_string(&mut app));
    }

    #[test]
    fn snapshot_monthly_tab() {
        let mut app = App::fixture_with_monthly();
        insta::assert_snapshot!(render_to_string(&mut app));
    }
}
