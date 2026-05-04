use ratatui::{prelude::*, widgets::*};

use crate::app::{App, Tab};
use crate::config::Config;

#[macro_use]
mod style_macros;

mod accounts;
mod monthly;
mod overlays;
mod portfolio;

pub(crate) use monthly::MONTHLY_GROUP_WIDTH;

use overlays::{
    render_file_prompt, render_help_popup, render_loading_overlay, render_price_alerts,
    render_search_overlay, render_status,
};

// ── Theme ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub(super) struct Theme {
    pub accent: Color,
    pub positive: Color,
    pub negative: Color,
    pub muted: Color,
    pub fg: Color,
    pub gold: Color,
    pub background: Color,
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

        let accent = t.accent.as_deref().and_then(parse_color).unwrap_or(accent);
        let positive = t
            .positive
            .as_deref()
            .and_then(parse_color)
            .unwrap_or(positive);
        let negative = t
            .negative
            .as_deref()
            .and_then(parse_color)
            .unwrap_or(negative);
        let muted = t.muted.as_deref().and_then(parse_color).unwrap_or(muted);
        let fg = t.fg.as_deref().and_then(parse_color).unwrap_or(fg);
        let gold = t.gold.as_deref().and_then(parse_color).unwrap_or(gold);
        let background = t
            .background
            .as_deref()
            .and_then(parse_color)
            .unwrap_or(background);

        Self {
            accent,
            positive,
            negative,
            muted,
            fg,
            gold,
            background,
            highlight_bg,
        }
    }
}

// ── Color helpers ─────────────────────────────────────────────────────────────

pub(super) fn parse_color(s: &str) -> Option<Color> {
    match s.to_lowercase().as_str() {
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "blue" => Some(Color::Blue),
        "yellow" => Some(Color::Yellow),
        "cyan" => Some(Color::Cyan),
        "magenta" => Some(Color::Magenta),
        "white" => Some(Color::White),
        "gray" => Some(Color::Gray),
        "darkgray" => Some(Color::DarkGray),
        "lightred" => Some(Color::LightRed),
        "lightgreen" => Some(Color::LightGreen),
        "lightblue" => Some(Color::LightBlue),
        "lightyellow" => Some(Color::LightYellow),
        "lightcyan" => Some(Color::LightCyan),
        "lightmagenta" => Some(Color::LightMagenta),
        s if s.starts_with('#') && s.len() == 7 => {
            let r = u8::from_str_radix(&s[1..3], 16).ok()?;
            let g = u8::from_str_radix(&s[3..5], 16).ok()?;
            let b = u8::from_str_radix(&s[5..7], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

/// Map an expense category to a display color. Config overrides via
/// `[colors.expenses]` take precedence; use those to add locale-specific
/// aliases (e.g. `Wohnen = "blue"` for German journals).
///
/// Lookup is case-insensitive and prefix-aware: a key like `wohnen` will
/// match `Wohnen:Miete`, and the most specific (longest) matching key wins.
pub(super) fn expense_color(category: &str, app: &App, theme: &Theme) -> Color {
    if let Some(color_str) = lookup_expense_color_override(category, &app.config.colors.expenses) {
        if let Some(c) = parse_color(color_str) {
            return c;
        }
    }

    let cat = category.to_lowercase();
    let top = cat.split(':').next().unwrap_or(&cat);
    match top {
        "housing" => Color::Blue,
        "food" | "groceries" | "restaurant" => theme.gold,
        "transport" | "car" | "fuel" => Color::Magenta,
        "health" => Color::LightCyan,
        "insurance" => Color::LightBlue,
        "telecom" => Color::Rgb(180, 140, 255),
        "entertainment" => Color::LightGreen,
        "shopping" | "clothing" => Color::Rgb(255, 150, 80),
        "education" | "books" => theme.accent,
        "tax" | "fees" | "crypto" => Color::Rgb(200, 200, 100),
        "amazon" => Color::Rgb(255, 120, 200),
        _ => theme.fg,
    }
}

/// Find the configured color override for `category` in `[colors.expenses]`.
///
/// Matching rules:
/// - case-insensitive
/// - exact match OR `category` starts with `key:` (so `wohnen` matches
///   `Wohnen:Miete` and any deeper sub-account)
/// - longest (most specific) matching key wins
fn lookup_expense_color_override<'a>(
    category: &str,
    map: &'a std::collections::HashMap<String, String>,
) -> Option<&'a str> {
    let cat_lower = category.to_lowercase();
    let mut best: Option<(usize, &'a str)> = None;
    for (key, val) in map {
        let key_lower = key.to_lowercase();
        let matches = cat_lower == key_lower || cat_lower.starts_with(&format!("{key_lower}:"));
        if matches && best.is_none_or(|(len, _)| key_lower.len() > len) {
            best = Some((key_lower.len(), val.as_str()));
        }
    }
    best.map(|(_, v)| v)
}

/// Return a configured color override for an income category, or `None` if
/// no override is set. Caller decides the fallback (name → `theme.fg`,
/// amount/bar → `theme.positive`). Same case-insensitive prefix matching
/// as `expense_color`.
pub(super) fn income_color(category: &str, app: &App, _theme: &Theme) -> Option<Color> {
    lookup_expense_color_override(category, &app.config.colors.income).and_then(parse_color)
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
    let hi = ((y_max_raw / step).ceil() * step).max(lo + step);
    let n = ((hi - lo) / step).round() as usize;
    let labels = (0..=n)
        .map(|i| {
            let v = lo + step * i as f64;
            let s = if v.abs() >= 100.0 {
                cfg.fmt_compact(v, 0)
            } else {
                cfg.fmt_compact(v, 1)
            };
            Span::styled(s, crate::fg!(muted))
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
            let amt_color = if t.amount >= 0.0 {
                theme.positive
            } else {
                theme.negative
            };
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

    if app.file_prompt_active {
        render_file_prompt(f, app, layout[2], &theme);
    }

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
            Tab::Accounts => "  Accounts  ".to_string(),
            Tab::Monthly => "  Monthly  ".to_string(),
            Tab::Portfolio => "  Portfolio  ".to_string(),
        })
        .collect();
    let selected = visible.iter().position(|&t| t == app.tab).unwrap_or(0);

    app.geometry.tab_bar_area = area;
    app.geometry.tab_rects.clear();
    let mut x = area.x + 1;
    for label in &labels {
        let w = label.chars().count() as u16;
        app.geometry.tab_rects.push(Rect {
            x,
            y: area.y + 1,
            width: w,
            height: 1,
        });
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
                .bg(theme.highlight_bg)
                .bold()
                .add_modifier(Modifier::UNDERLINED),
        );
    f.render_widget(tabs, area);
}

// ── Content dispatch ──────────────────────────────────────────────────────────

fn render_content(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    match app.tab {
        Tab::Portfolio => portfolio::render_portfolio(f, app, area, theme),
        Tab::Accounts => accounts::render_accounts(f, app, area, theme),
        Tab::Monthly => monthly::render_monthly(f, app, area, theme),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

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

    #[test]
    fn color_override_exact_match() {
        let mut map = std::collections::HashMap::new();
        map.insert("essen".to_string(), "yellow".to_string());
        assert_eq!(lookup_expense_color_override("essen", &map), Some("yellow"));
    }

    #[test]
    fn color_override_case_insensitive() {
        let mut map = std::collections::HashMap::new();
        map.insert("wohnen:miete".to_string(), "blue".to_string());
        assert_eq!(
            lookup_expense_color_override("Wohnen:Miete", &map),
            Some("blue")
        );
    }

    #[test]
    fn color_override_prefix_match() {
        // top-level key colors all sub-accounts
        let mut map = std::collections::HashMap::new();
        map.insert("essen".to_string(), "yellow".to_string());
        assert_eq!(
            lookup_expense_color_override("Essen:Restaurant", &map),
            Some("yellow")
        );
    }

    #[test]
    fn color_override_longest_key_wins() {
        let mut map = std::collections::HashMap::new();
        map.insert("wohnen".to_string(), "blue".to_string());
        map.insert("wohnen:miete".to_string(), "red".to_string());
        assert_eq!(
            lookup_expense_color_override("wohnen:miete", &map),
            Some("red")
        );
        assert_eq!(
            lookup_expense_color_override("wohnen:strom", &map),
            Some("blue")
        );
    }

    #[test]
    fn color_override_no_partial_segment_match() {
        // "foo" must not match "food" — only full segment boundary
        let mut map = std::collections::HashMap::new();
        map.insert("foo".to_string(), "red".to_string());
        assert_eq!(lookup_expense_color_override("food", &map), None);
    }

    #[test]
    fn color_override_returns_none_when_unmatched() {
        let mut map = std::collections::HashMap::new();
        map.insert("transport".to_string(), "magenta".to_string());
        assert_eq!(lookup_expense_color_override("essen", &map), None);
    }

    #[test]
    fn income_color_returns_override() {
        use crate::app::App;
        let mut app = App::fixture_empty();
        app.config
            .colors
            .income
            .insert("gehalt".to_string(), "cyan".to_string());
        let theme = Theme::from_config(&app.config);
        assert_eq!(income_color("gehalt", &app, &theme), Some(Color::Cyan));
    }

    #[test]
    fn income_color_prefix_match() {
        use crate::app::App;
        let mut app = App::fixture_empty();
        app.config
            .colors
            .income
            .insert("gehalt".to_string(), "cyan".to_string());
        let theme = Theme::from_config(&app.config);
        assert_eq!(
            income_color("gehalt:bonus", &app, &theme),
            Some(Color::Cyan)
        );
    }

    #[test]
    fn income_color_falls_back_to_none() {
        use crate::app::App;
        let app = App::fixture_empty();
        let theme = Theme::from_config(&app.config);
        assert_eq!(income_color("gehalt", &app, &theme), None);
    }
}
