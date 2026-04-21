use ratatui::{prelude::*, widgets::*};

use crate::app::{App, Tab};
use crate::config::Config;

mod accounts;
mod monthly;
mod portfolio;

// ── Theme ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub(super) struct Theme {
    pub accent:       Color,
    pub positive:     Color,
    pub negative:     Color,
    pub muted:        Color,
    pub fg:           Color,
    pub gold:         Color,
    pub background:   Color,
    pub highlight_bg: Color,
}

impl Theme {
    pub fn from_config(cfg: &Config) -> Self {
        let t = &cfg.theme;

        let (accent, positive, negative, muted, fg, gold, background, highlight_bg) =
            match t.preset.as_deref().unwrap_or("dark") {
                "light" => (
                    Color::Blue,
                    Color::Rgb(0, 110, 0),
                    Color::Rgb(180, 0, 0),
                    Color::Rgb(90, 90, 90),
                    Color::Black,
                    Color::Rgb(160, 100, 0),
                    Color::Rgb(245, 245, 245),
                    Color::Rgb(195, 215, 240),
                ),
                "solarized" => (
                    Color::Rgb(38, 139, 210),
                    Color::Rgb(133, 153, 0),
                    Color::Rgb(220, 50, 47),
                    Color::Rgb(101, 123, 131),
                    Color::Rgb(131, 148, 150),
                    Color::Rgb(181, 137, 0),
                    Color::Rgb(0, 43, 54),
                    Color::Rgb(7, 54, 66),
                ),
                _ => (
                    Color::Cyan,
                    Color::Green,
                    Color::Red,
                    Color::DarkGray,
                    Color::White,
                    Color::Yellow,
                    Color::Rgb(20, 20, 30),
                    Color::Rgb(40, 40, 60),
                ),
            };

        let accent     = t.accent.as_deref().and_then(parse_color).unwrap_or(accent);
        let positive   = t.positive.as_deref().and_then(parse_color).unwrap_or(positive);
        let negative   = t.negative.as_deref().and_then(parse_color).unwrap_or(negative);
        let muted      = t.muted.as_deref().and_then(parse_color).unwrap_or(muted);
        let fg         = t.fg.as_deref().and_then(parse_color).unwrap_or(fg);
        let gold       = t.gold.as_deref().and_then(parse_color).unwrap_or(gold);
        let background = t.background.as_deref().and_then(parse_color).unwrap_or(background);

        Self { accent, positive, negative, muted, fg, gold, background, highlight_bg }
    }
}

// ── Color helpers ─────────────────────────────────────────────────────────────

pub(super) fn parse_color(s: &str) -> Option<Color> {
    match s.to_lowercase().as_str() {
        "red"      => Some(Color::Red),
        "green"    => Some(Color::Green),
        "blue"     => Some(Color::Blue),
        "yellow"   => Some(Color::Yellow),
        "cyan"     => Some(Color::Cyan),
        "magenta"  => Some(Color::Magenta),
        "white"    => Some(Color::White),
        "gray"     => Some(Color::Gray),
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

pub(super) fn expense_color(category: &str, app: &App, theme: &Theme) -> Color {
    if let Some(color_str) = app.config.colors.expenses.get(category) {
        if let Some(c) = parse_color(color_str) {
            return c;
        }
    }

    let cat = category.to_lowercase();
    let top = cat.split(':').next().unwrap_or(&cat);
    match top {
        "wohnen" | "housing" | "hauskauf" => Color::Blue,
        "essen" | "food" | "groceries" | "restaurant" => theme.gold,
        "transport" | "car" | "fuel" => Color::Magenta,
        "gesundheit" | "health" | "hygiene" => Color::LightCyan,
        "versicherung" | "insurance" => Color::LightBlue,
        "kommunikation" | "telecom" | "haushalt" => Color::Rgb(180, 140, 255),
        "freizeit" | "entertainment" | "urlaub" => Color::LightGreen,
        "kleider" | "kinder" | "shopping" | "clothing" => Color::Rgb(255, 150, 80),
        "fortbildung" | "education" | "books" => theme.accent,
        "steuer" | "tax" | "fees" | "crypto" => Color::Rgb(200, 200, 100),
        "abos" | "amazon" => Color::Rgb(255, 120, 200),
        "spende" | "schenkung" => Color::Rgb(150, 220, 180),
        _ => theme.fg,
    }
}

pub(super) fn coin_color(commodity: &str, theme: &Theme) -> Color {
    let palette = [
        theme.gold,
        theme.accent,
        theme.positive,
        Color::Magenta,
        Color::LightBlue,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
    ];
    let hash: usize = commodity.bytes().map(|b| b as usize).sum();
    palette[hash % palette.len()]
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
    muted: Color,
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
            Span::styled(s, Style::default().fg(muted))
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
    theme: &Theme,
    state: &mut ratatui::widgets::TableState,
) {
    let rows: Vec<Row> = txns
        .iter()
        .map(|t| {
            let amt_color = if t.amount >= 0.0 { theme.positive } else { theme.negative };
            Row::new(vec![
                Cell::from(t.date.format("%Y-%m-%d").to_string())
                    .style(Style::default().fg(theme.muted)),
                Cell::from(t.description.clone()).style(Style::default().fg(theme.fg)),
                Cell::from(format!("{:>13}", cfg.fmt_amount(t.amount, 2)))
                    .style(Style::default().fg(amt_color)),
                Cell::from(format!("{:>13}", cfg.fmt_amount(t.running_total, 2)))
                    .style(Style::default().fg(theme.accent)),
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
                .style(Style::default().fg(theme.muted).bold())
                .bottom_margin(1),
        )
        .row_highlight_style(Style::default().bg(theme.highlight_bg).bold())
        .highlight_symbol("▶ ")
        .block(
            Block::default()
                .title(Span::styled(title, Style::default().fg(theme.gold).bold()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.accent)),
        );

    f.render_stateful_widget(table, area, state);
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, app: &mut App) {
    let theme = Theme::from_config(&app.config);
    let area = f.area();

    // Paint the entire frame with the theme background so non-widget cells
    // inherit the right colour (essential for light / solarized presets).
    f.render_widget(
        Block::default().style(Style::default().bg(theme.background)),
        area,
    );

    let layout = Layout::vertical([
        Constraint::Length(1), // title bar
        Constraint::Length(3), // tab bar
        Constraint::Min(0),    // content
        Constraint::Length(1), // status bar
    ])
    .split(area);

    render_title(f, layout[0], &theme);
    render_tabs(f, app, layout[1], &theme);
    render_content(f, app, layout[2], &theme);

    if app.search_active {
        render_search_overlay(f, app, layout[2], &theme);
    }

    if app.show_alerts {
        render_price_alerts(f, app, layout[2], &theme);
    }

    render_status(f, app, layout[3], &theme);

    if app.loading {
        render_loading_overlay(f, area, &theme);
    }

    if app.show_help {
        render_help_popup(f, area, &theme);
    }
}

// ── Title bar ─────────────────────────────────────────────────────────────────

fn render_title(f: &mut Frame, area: Rect, theme: &Theme) {
    let title = Paragraph::new(Line::from(vec![
        Span::raw("  "),
        Span::styled("⬡", Style::default().fg(theme.gold)),
        Span::styled(" Ledger Dashboard", Style::default().fg(theme.fg).bold()),
        Span::styled(
            format!(" v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(theme.muted),
        ),
        Span::raw("  "),
    ]))
    .style(Style::default().bg(theme.background));
    f.render_widget(title, area);
}

// ── Tab bar ───────────────────────────────────────────────────────────────────

fn render_tabs(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let visible = app.visible_tabs();
    let labels: Vec<String> = visible
        .iter()
        .map(|t| match t {
            Tab::Accounts  => "  Accounts  ".to_string(),
            Tab::Monthly   => "  Monthly  ".to_string(),
            Tab::Portfolio => "  Portfolio  ".to_string(),
        })
        .collect();
    let selected = visible
        .iter()
        .position(|&t| t == app.tab)
        .unwrap_or(0);

    app.tab_bar_area = area;
    app.tab_rects.clear();
    let mut x = area.x + 1;
    for label in &labels {
        let w = label.chars().count() as u16;
        app.tab_rects.push(Rect { x, y: area.y + 1, width: w, height: 1 });
        x += w + 1;
    }

    let tabs = Tabs::new(labels)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.muted))
                .border_type(BorderType::Rounded),
        )
        .select(selected)
        .style(Style::default().fg(theme.muted))
        .highlight_style(
            Style::default()
                .fg(theme.accent)
                .bold()
                .add_modifier(Modifier::UNDERLINED),
        );
    f.render_widget(tabs, area);
}

// ── Content dispatch ──────────────────────────────────────────────────────────

fn render_content(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    match app.tab {
        Tab::Portfolio => portfolio::render_portfolio(f, app, area, theme),
        Tab::Accounts  => accounts::render_accounts(f, app, area, theme),
        Tab::Monthly   => monthly::render_monthly(f, app, area, theme),
    }
}

// ── Overlays and status ───────────────────────────────────────────────────────

fn render_loading_overlay(f: &mut Frame, area: Rect, theme: &Theme) {
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
        .border_style(Style::default().fg(theme.accent));
    let text = Paragraph::new(Line::from(vec![
        Span::styled("⏳ ", Style::default().fg(theme.gold)),
        Span::styled("Loading data…", Style::default().fg(theme.fg).bold()),
    ]))
    .alignment(Alignment::Center)
    .block(block);
    f.render_widget(text, popup);
}

fn render_help_popup(f: &mut Frame, area: Rect, theme: &Theme) {
    let w = 68u16.min(area.width.saturating_sub(4));
    let h = 30u16.min(area.height.saturating_sub(4));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };

    f.render_widget(Clear, popup);

    // ("§", "Title") → section header   ("", "") → blank spacer
    let bindings: &[(&str, &str)] = &[
        ("", ""),
        ("§", "Navigation"),
        ("1/2/3  Tab/⇧Tab",      "Switch tab"),
        ("↑k / ↓j",              "Scroll / select row"),
        ("PgUp/PgDn  Home/End",  "Page up/down · first/last"),
        ("←h / →l",              "Month · NW range · chart range"),
        ("Enter  /",             "Open detail · search"),
        ("", ""),
        ("§", "Tab-specific"),
        ("y / Y",                "Year back / forward  (Monthly)"),
        ("i",                    "Income/expense focus  (Monthly)"),
        ("a–z  Backspace",       "Filter · clear char  (Accounts)"),
        ("", ""),
        ("§", "Data & view"),
        ("e",                    "Export view to file"),
        ("Y",                    "Copy view to clipboard  (non-Monthly)"),
        ("s",                    "Toggle chart stacked / unstacked"),
        ("c",                    "Toggle expense colors"),
        ("r",                    "Refresh data"),
        ("", ""),
        ("§", "General"),
        ("? / q",                "Toggle help / quit"),
        ("Esc",                  "Close detail / back"),
        ("Mouse click",          "Select tab / row"),
        ("Scroll wheel",         "Scroll table"),
    ];

    let mut lines: Vec<Line> = Vec::new();
    for (key, desc) in bindings {
        if *key == "§" {
            lines.push(Line::from(Span::styled(
                format!("  {}", desc),
                Style::default().fg(theme.gold).bold(),
            )));
        } else if key.is_empty() {
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(vec![
                Span::styled(format!("    {:<22}", key), Style::default().fg(theme.accent).bold()),
                Span::styled(*desc, Style::default().fg(theme.fg)),
            ]));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  P/L shows unrealized gains only.",
        Style::default().fg(theme.muted),
    )));

    let block = Block::default()
        .title(Span::styled(
            " Keybindings ",
            Style::default().fg(theme.gold).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .style(Style::default().bg(theme.background));

    f.render_widget(Paragraph::new(lines).block(block), popup);
}

fn render_search_overlay(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    f.render_widget(Clear, area);

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    let input = Paragraph::new(Line::from(vec![
        Span::styled("  / ", Style::default().fg(theme.gold).bold()),
        Span::styled(&app.search_query, Style::default().fg(theme.fg)),
        Span::styled("█", Style::default().fg(theme.accent)),
    ]))
    .block(
        Block::default()
            .title(Span::styled(
                " Search Transactions ",
                Style::default().fg(theme.accent).bold(),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.accent)),
    );
    f.render_widget(input, chunks[0]);

    let rows: Vec<Row> = app
        .search_results
        .iter()
        .map(|t| {
            let amt_color = if t.amount >= 0.0 { theme.positive } else { theme.negative };
            Row::new(vec![
                Cell::from(t.date.format("%Y-%m-%d").to_string())
                    .style(Style::default().fg(theme.muted)),
                Cell::from(t.description.clone()).style(Style::default().fg(theme.fg)),
                Cell::from(format!("{:>13}", app.config.fmt_amount(t.amount, 2)))
                    .style(Style::default().fg(amt_color)),
                Cell::from(format!("{:>13}", app.config.fmt_amount(t.running_total, 2)))
                    .style(Style::default().fg(theme.accent)),
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
            .style(Style::default().fg(theme.muted).bold())
            .bottom_margin(1),
    )
    .row_highlight_style(Style::default().bg(theme.highlight_bg).bold())
    .highlight_symbol("▶ ")
    .block(
        Block::default()
            .title(Span::styled(title, Style::default().fg(theme.gold).bold()))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.muted)),
    );

    f.render_stateful_widget(table, chunks[1], &mut app.search_state);

    let hint = Paragraph::new(Line::from(vec![
        Span::styled(
            "  [Enter] search  [↑↓] navigate  [Esc] close",
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            "  Supports regex (e.g. \"grocery|supermarket\")",
            Style::default().fg(theme.muted),
        ),
    ]));
    f.render_widget(hint, chunks[2]);
}

fn render_price_alerts(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
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
        Span::styled("  Price moves: ", Style::default().fg(theme.gold).bold()),
    ];
    for (i, alert) in app.price_alerts.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(", ", Style::default().fg(theme.muted)));
        }
        let (prefix, color) = if alert.change_pct >= 0.0 {
            ("+", theme.positive)
        } else {
            ("", theme.negative)
        };
        spans.push(Span::styled(
            format!("{} {prefix}{:.1}%", alert.coin, alert.change_pct),
            Style::default().fg(color).bold(),
        ));
    }
    spans.push(Span::styled("  [any key dismiss]", Style::default().fg(theme.muted)));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.gold))
        .style(Style::default().bg(theme.background));

    f.render_widget(Paragraph::new(Line::from(spans)).block(block), banner);
}

fn render_status(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    if app.export_prompt_active {
        let text = Line::from(vec![
            Span::styled("  Export to: ", Style::default().fg(theme.gold).bold()),
            Span::styled(&app.export_prompt_path, Style::default().fg(theme.fg)),
            Span::styled("█", Style::default().fg(theme.accent)),
            Span::styled("  [Enter] confirm  [Esc] cancel", Style::default().fg(theme.muted)),
        ]);
        f.render_widget(Paragraph::new(text), area);
        return;
    }

    let help = "  [1-3] tab  [↑↓/jk] navigate  [PgUp/PgDn/Home/End] scroll  [←→/hl] month/range  [r] refresh  [?] help  [q] quit";
    let text = Line::from(vec![
        Span::styled(&app.status_msg, Style::default().fg(theme.accent)),
        Span::styled(help, Style::default().fg(theme.muted)),
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
