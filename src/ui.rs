use chrono::Datelike;
use ratatui::{prelude::*, widgets::*};

use crate::app::{App, Tab};

// ── Color palette ────────────────────────────────────────────────────────────
const ACCENT: Color = Color::Cyan;
const GREEN: Color = Color::Green;
const RED: Color = Color::Red;
const GOLD: Color = Color::Yellow;
const MUTED: Color = Color::DarkGray;
const FG: Color = Color::White;

fn parse_color(s: &str) -> Option<Color> {
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

fn expense_color(category: &str, app: &App) -> Color {
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

fn nice_y_axis(y_min_raw: f64, y_max_raw: f64, ticks: usize) -> (f64, f64, Vec<Span<'static>>) {
    let range = (y_max_raw - y_min_raw).max(1.0);
    let step = nice_step(range, ticks);
    let lo = (y_min_raw / step).floor() * step;
    let hi = (y_max_raw / step).ceil() * step;
    let n = ((hi - lo) / step).round() as usize;
    let labels = (0..=n)
        .map(|i| {
            let v = lo + step * i as f64;
            let s = if v.abs() >= 10_000.0 {
                format!("{:.0}€", v)
            } else if v.abs() >= 100.0 {
                format!("{:.0}€", v)
            } else {
                format!("{:.1}€", v)
            };
            Span::styled(s, Style::default().fg(MUTED))
        })
        .collect();
    (lo, hi, labels)
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
        Span::raw("  "),
    ]))
    .style(Style::default().bg(Color::Rgb(20, 20, 30)));
    f.render_widget(title, area);
}

// ── Tab bar ───────────────────────────────────────────────────────────────────

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let labels = vec!["  Portfolio  ", "  Accounts  ", "  Monthly  "];
    let tabs = Tabs::new(labels)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(MUTED))
                .border_type(BorderType::Rounded),
        )
        .select(app.tab.index())
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
        Tab::Portfolio => render_portfolio(f, app, area),
        Tab::Accounts => render_accounts(f, app, area),
        Tab::Monthly => render_monthly(f, app, area),
    }
}

// ── Status bar ────────────────────────────────────────────────────────────────

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
    let w = 44u16.min(area.width.saturating_sub(4));
    let h = 16u16.min(area.height.saturating_sub(4));
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
        ("Enter", "Drill into detail"),
        ("c", "Toggle expense colors"),
        ("Esc", "Back / quit"),
        ("r", "Refresh data"),
        ("?", "Toggle this help"),
        ("q", "Quit"),
    ];

    let mut lines = vec![Line::from("")];
    for (key, desc) in bindings {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<18}", key), Style::default().fg(ACCENT).bold()),
            Span::styled(*desc, Style::default().fg(FG)),
        ]));
    }

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

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let help = "  [1-3] tab  [↑↓/jk] navigate  [←→/hl] month/range  [r] refresh  [?] help  [q] quit";
    let text = Line::from(vec![
        Span::styled(&app.status_msg, Style::default().fg(ACCENT)),
        Span::styled(help, Style::default().fg(MUTED)),
    ]);
    f.render_widget(Paragraph::new(text), area);
}

// ── Portfolio tab ─────────────────────────────────────────────────────────────

fn render_portfolio(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::horizontal([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(area);

    render_holdings_table(f, app, chunks[0]);
    render_price_chart(f, app, chunks[1]);
}

fn render_holdings_table(f: &mut Frame, app: &App, area: Rect) {
    let total = app.total_portfolio_value();

    let mut rows: Vec<Row> = app
        .holdings
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let selected = i == app.selected_holding;

            let coin_style = if selected {
                Style::default().fg(GOLD).bold()
            } else {
                Style::default().fg(FG)
            };

            let price_str = if h.price_eur >= 10_000.0 {
                format!("{:.0} €", h.price_eur)
            } else if h.price_eur >= 100.0 {
                format!("{:.2} €", h.price_eur)
            } else {
                format!("{:.3} €", h.price_eur)
            };

            let amt_str = if h.amount >= 100.0 {
                format!("{:.2}", h.amount)
            } else {
                format!("{:.4}", h.amount)
            };

            let value_str = format!("{:.2} €", h.value_eur);

            let indicator = if selected { "▶ " } else { "  " };

            let pct = if total > 0.0 {
                format!("{:.1}%", h.value_eur / total * 100.0)
            } else {
                "—".into()
            };

            let (pl_pct_str, pl_pct_style, pl_eur_str, pl_eur_style) =
                match app.coin_chart_cache.get(&h.commodity) {
                    Some(series) if !series.investment.is_empty() => {
                        let invested = series.total_invested();
                        if invested > 0.0 {
                            let pl_abs = h.value_eur - invested;
                            let abs_color = if pl_abs >= 0.0 { GREEN } else { RED };
                            let abs_prefix = if pl_abs >= 0.0 { "+" } else { "" };
                            let pct = pl_abs / invested * 100.0;
                            let (prefix, color) = if pct >= 0.0 {
                                ("+", GREEN)
                            } else {
                                ("", RED)
                            };
                            (
                                format!("{prefix}{:.1}%", pct),
                                Style::default().fg(color).bold(),
                                format!("{abs_prefix}{:.2}€", pl_abs),
                                Style::default().fg(abs_color).bold(),
                            )
                        } else if h.value_eur > 0.0 {
                            (
                                "∞".into(),
                                Style::default().fg(GREEN).bold(),
                                format!("+{:.2}€", h.value_eur),
                                Style::default().fg(GREEN).bold(),
                            )
                        } else {
                            (
                                "—".into(),
                                Style::default().fg(MUTED),
                                "—".into(),
                                Style::default().fg(MUTED),
                            )
                        }
                    }
                    _ => (
                        "—".into(),
                        Style::default().fg(MUTED),
                        "—".into(),
                        Style::default().fg(MUTED),
                    ),
                };

            Row::new(vec![
                Cell::from(format!("{indicator}{}", h.commodity)).style(coin_style),
                Cell::from(amt_str).style(Style::default().fg(if selected { GOLD } else { MUTED })),
                Cell::from(price_str).style(Style::default().fg(ACCENT)),
                Cell::from(value_str).style(Style::default().fg(GREEN)),
                Cell::from(pct).style(Style::default().fg(FG)),
                Cell::from(pl_pct_str).style(pl_pct_style),
                Cell::from(pl_eur_str).style(pl_eur_style),
            ])
        })
        .collect();

    let (pl_abs, pl_pct) = app.total_portfolio_pl();
    let pl_color = if pl_abs >= 0.0 { GREEN } else { RED };
    let pl_prefix = if pl_abs >= 0.0 { "+" } else { "" };

    rows.push(
        Row::new(vec![
            Cell::from("──────").style(Style::default().fg(MUTED)),
            Cell::from(""),
            Cell::from("Total").style(Style::default().fg(FG).bold()),
            Cell::from(format!("{:.2} €", total)).style(Style::default().fg(GOLD).bold()),
            Cell::from("100%").style(Style::default().fg(FG).bold()),
            Cell::from(format!("{pl_prefix}{:.1}%", pl_pct))
                .style(Style::default().fg(pl_color).bold()),
            Cell::from(format!("{pl_prefix}{:.2}€", pl_abs))
                .style(Style::default().fg(pl_color).bold()),
        ])
        .height(1),
    );

    let widths = [
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(11),
        Constraint::Length(12),
        Constraint::Length(7),
        Constraint::Length(8),
        Constraint::Length(14),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["Coin", "Amount", "Price", "Value", "Alloc", "P/L %", "P/L €"])
                .style(Style::default().fg(MUTED).bold())
                .bottom_margin(1),
        )
        .block(
            Block::default()
                .title(Span::styled(
                    " Crypto Portfolio ",
                    Style::default().fg(ACCENT).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED)),
        );

    f.render_widget(table, area);

    // Hint at bottom
    let hint_area = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(2),
        width: area.width.saturating_sub(2),
        height: 1,
    };
    f.render_widget(
        Paragraph::new("  ↑↓ select coin for chart").style(Style::default().fg(MUTED)),
        hint_area,
    );
}

fn render_price_chart(f: &mut Frame, app: &App, area: Rect) {
    let selected_coin = app.selected_coin().unwrap_or("SOL");

    let block = Block::default()
        .title(Span::styled(
            format!(" {} Portfolio Analysis ", selected_coin),
            Style::default().fg(ACCENT).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED));

    let series = match app.coin_chart_cache.get(selected_coin) {
        Some(s) if !s.investment.is_empty() => s,
        _ => {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "No portfolio data available",
                    Style::default().fg(MUTED),
                )))
                .block(block)
                .alignment(Alignment::Center),
                area,
            );
            return;
        }
    };

    // X bounds from the investment series (all three share the same x values)
    let x_min = series.investment.first().map(|p| p.0).unwrap_or(0.0);
    let x_max = series.investment.last().map(|p| p.0).unwrap_or(1.0);

    // Y bounds across all three series
    let all_y = series
        .investment
        .iter()
        .chain(series.price_growth.iter())
        .chain(series.staking_growth.iter())
        .map(|p| p.1);
    let y_min_raw = all_y.clone().fold(0.0_f64, f64::min);
    let y_max_raw = all_y.fold(0.0_f64, f64::max);
    let (y_min, y_max, y_labels) = nice_y_axis(y_min_raw, y_max_raw, 4);

    let coin_entries: Vec<_> = app
        .price_history
        .iter()
        .filter(|e| e.commodity == selected_coin)
        .collect();
    let x_labels: Vec<Span> = {
        let n = coin_entries.len();
        let count = if area.width < 60 { 3 } else { 5 };
        let indices: Vec<usize> = (0..count).map(|i| i * n.saturating_sub(1) / (count - 1).max(1)).collect();
        indices
            .iter()
            .filter_map(|&i| coin_entries.get(i))
            .map(|e| Span::styled(e.date.format("%b %y").to_string(), Style::default().fg(MUTED)))
            .collect()
    };

    let ds_investment = Dataset::default()
        .name("Invested")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(GOLD))
        .data(&series.investment);

    let ds_price = Dataset::default()
        .name("Price gain")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(ACCENT))
        .data(&series.price_growth);

    let ds_staking = Dataset::default()
        .name("Staking")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(GREEN))
        .data(&series.staking_growth);

    let chart = Chart::new(vec![ds_investment, ds_price, ds_staking])
        .block(block)
        .x_axis(
            Axis::default()
                .style(Style::default().fg(MUTED))
                .bounds([x_min, x_max])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(MUTED))
                .bounds([y_min, y_max])
                .labels(y_labels),
        );

    f.render_widget(chart, area);
}

// ── Accounts tab ──────────────────────────────────────────────────────────────

fn render_net_worth_chart(f: &mut Frame, app: &App, area: Rect) {
    let range_label = app.nw_range.label();
    let block = Block::default()
        .title(Span::styled(
            format!(" Net Worth History [{range_label}]  ◀ ▶ "),
            Style::default().fg(GOLD).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED));

    let series = &app.net_worth_history;
    if series.points.len() < 2 {
        f.render_widget(
            Paragraph::new(Span::styled("Not enough data", Style::default().fg(MUTED)))
                .block(block)
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let x_min = series.points.first().unwrap().0;
    let x_max = series.points.last().unwrap().0;
    let y_min_raw = series.points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let y_max_raw = series.points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let (y_min, y_max, y_labels) = nice_y_axis(y_min_raw, y_max_raw, 4);

    let label_count = if area.width < 60 { 3 } else { 5 };
    let n = series.labels.len();
    let x_labels: Vec<Span> = (0..label_count)
        .map(|i| i * n.saturating_sub(1) / (label_count - 1).max(1))
        .filter_map(|i| series.labels.get(i))
        .map(|(d, _)| Span::styled(d.format("%b %y").to_string(), Style::default().fg(MUTED)))
        .collect();

    let dataset = Dataset::default()
        .name("Net Worth")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(GOLD))
        .data(&series.points);

    let chart = Chart::new(vec![dataset])
        .block(block)
        .x_axis(
            Axis::default()
                .style(Style::default().fg(MUTED))
                .bounds([x_min, x_max])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(MUTED))
                .bounds([y_min, y_max])
                .labels(y_labels),
        );

    f.render_widget(chart, area);
}

fn render_accounts(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Percentage(40), // net worth chart
        Constraint::Length(3),      // net worth number
        Constraint::Min(0),         // accounts table or detail
    ])
    .split(area);

    render_net_worth_chart(f, app, chunks[0]);

    let net_worth = app.total_net_worth();
    let nw_text = Line::from(vec![
        Span::styled("  Net Worth: ", Style::default().fg(MUTED).bold()),
        Span::styled(
            format!("{:.2} €", net_worth),
            Style::default().fg(GOLD).bold(),
        ),
    ]);
    let nw_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED));
    f.render_widget(Paragraph::new(nw_text).block(nw_block), chunks[1]);

    if app.account_detail.is_some() {
        let txns = app.account_detail.as_ref().unwrap();
        let name = app.detail_account_name.as_ref().unwrap();
        render_account_detail(f, txns, name, chunks[2]);
    } else {
        render_accounts_table(f, app, chunks[2]);
    }
}

fn render_detail_with_title(f: &mut Frame, txns: &[crate::data::Transaction], title: &str, area: Rect) {
    let rows: Vec<Row> = txns
        .iter()
        .map(|t| {
            let amt_color = if t.amount >= 0.0 { GREEN } else { RED };
            Row::new(vec![
                Cell::from(t.date.format("%Y-%m-%d").to_string()).style(Style::default().fg(MUTED)),
                Cell::from(t.description.clone()).style(Style::default().fg(FG)),
                Cell::from(format!("{:>10.2} €", t.amount)).style(Style::default().fg(amt_color)),
                Cell::from(format!("{:>10.2} €", t.running_total))
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

fn render_account_detail(f: &mut Frame, txns: &[crate::data::Transaction], name: &str, area: Rect) {
    let rows: Vec<Row> = txns
        .iter()
        .map(|t| {
            let amt_color = if t.amount >= 0.0 { GREEN } else { RED };
            Row::new(vec![
                Cell::from(t.date.format("%Y-%m-%d").to_string()).style(Style::default().fg(MUTED)),
                Cell::from(t.description.clone()).style(Style::default().fg(FG)),
                Cell::from(format!("{:>10.2} €", t.amount)).style(Style::default().fg(amt_color)),
                Cell::from(format!("{:>10.2} €", t.running_total))
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

    let title = format!(" {} — Recent Transactions  [Esc back] ", name);
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

fn render_accounts_table(f: &mut Frame, app: &mut App, area: Rect) {
    let max_amount = app
        .account_balances
        .iter()
        .map(|b| b.amount.abs())
        .fold(0.0_f64, f64::max);

    let bar_width = (area.width as f64 * 0.2) as usize;

    let rows: Vec<Row> = app
        .account_balances
        .iter()
        .map(|b| {
            let amount_style = if b.amount >= 0.0 {
                Style::default().fg(GREEN)
            } else {
                Style::default().fg(RED)
            };

            let amount_str = format!("{:>12.2} €", b.amount);

            let bar_len = if max_amount > 0.0 {
                ((b.amount.abs() / max_amount) * bar_width as f64) as usize
            } else {
                0
            };
            let bar = "█".repeat(bar_len);

            Row::new(vec![
                Cell::from(format!("  {}", b.account)).style(Style::default().fg(FG)),
                Cell::from(amount_str).style(amount_style),
                Cell::from(bar).style(Style::default().fg(Color::Rgb(0, 130, 130))),
            ])
        })
        .collect();

    let widths = [Constraint::Min(38), Constraint::Length(16), Constraint::Min(10)];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["Account", "Balance (EUR)", ""])
                .style(Style::default().fg(MUTED).bold())
                .bottom_margin(1),
        )
        .row_highlight_style(Style::default().bg(Color::Rgb(40, 40, 60)).bold())
        .highlight_symbol("▶ ")
        .block(
            Block::default()
                .title(Span::styled(
                    " Asset Balances  [Enter drill-down] ",
                    Style::default().fg(ACCENT).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED)),
        );

    f.render_stateful_widget(table, area, &mut app.account_state);
}

// ── Monthly tab ───────────────────────────────────────────────────────────────

fn render_monthly_chart(f: &mut Frame, app: &App, area: Rect) {
    use ratatui::widgets::{Bar, BarChart, BarGroup};

    let groups: Vec<BarGroup> = app
        .monthly
        .months
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let selected = i == app.monthly.selected;
            let (inc_color, exp_color) = if selected {
                (Color::LightGreen, Color::LightRed)
            } else {
                (GREEN, RED)
            };

            let label = &m.month_name[..3];
            BarGroup::default()
                .label(Line::from(label.to_string()).style(Style::default().fg(if selected { ACCENT } else { MUTED })))
                .bars(&[
                    Bar::default()
                        .value(m.total_income as u64)
                        .style(Style::default().fg(inc_color)),
                    Bar::default()
                        .value(m.total_expenses as u64)
                        .style(Style::default().fg(exp_color)),
                ])
        })
        .collect();

    let mut chart = BarChart::default()
        .block(
            Block::default()
                .title(Span::styled(
                    " Income vs Expenses ",
                    Style::default().fg(ACCENT).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED))
                .padding(Padding::new(1, 1, 1, 0)),
        )
        .bar_width(3)
        .bar_gap(0)
        .group_gap(2)
        .bar_style(Style::default().fg(GREEN));

    for g in groups {
        chart = chart.data(g);
    }

    f.render_widget(chart, area);
}

fn render_monthly(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(12), // bar chart
        Constraint::Length(10), // summary
        Constraint::Min(0),     // detail tables
    ])
    .split(area);

    render_monthly_chart(f, app, chunks[0]);
    render_monthly_summary(f, app, chunks[1]);

    let detail_chunks =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2]);

    render_monthly_income(f, app, detail_chunks[0]);

    if let (Some(txns), Some(name)) = (&app.expense_detail, &app.detail_expense_name) {
        let short = name.strip_prefix("expenses:").unwrap_or(name);
        let month = app.current_month().map(|m| m.month_name.as_str()).unwrap_or("");
        let title = format!(" {} — {}  [Esc back] ", short, month);
        render_detail_with_title(f, txns, &title, detail_chunks[1]);
    } else {
        render_monthly_expenses(f, app, detail_chunks[1]);
    }
}

fn render_monthly_summary(f: &mut Frame, app: &App, area: Rect) {
    let empty = crate::data::SingleMonth::default();
    let m = app.current_month().unwrap_or(&empty);
    let net = m.total_income - m.total_expenses;
    let net_color = if net >= 0.0 { GREEN } else { RED };
    let net_prefix = if net >= 0.0 { "+" } else { "" };

    let savings_rate = if m.total_income > 0.0 {
        (net / m.total_income * 100.0).max(0.0).min(100.0) as u16
    } else {
        0
    };

    let chunks = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(30),
        Constraint::Percentage(40),
    ])
    .split(area);

    let nav_title = format!(" ◀ {} ▶ ", m.month_name);

    let mut text = vec![
        Line::from(vec![
            Span::styled("  Income    ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{:.2} €", m.total_income),
                Style::default().fg(GREEN).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Expenses  ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{:.2} €", m.total_expenses),
                Style::default().fg(RED).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Net       ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{net_prefix}{:.2} €", net),
                Style::default().fg(net_color).bold(),
            ),
        ]),
    ];

    if let Some(ly) = app.last_year_match() {
        let short = &m.month_name[..3];
        let year_ago = chrono::Local::now().year() - 1;
        text.push(Line::from(""));

        let incomplete = ly.expenses.len() < 3
            || (m.total_expenses > 0.0 && ly.total_expenses / m.total_expenses < 0.25);

        if incomplete {
            text.push(Line::from(vec![
                Span::styled("  vs ", Style::default().fg(MUTED)),
                Span::styled(
                    format!("{short} '{}", year_ago % 100),
                    Style::default().fg(MUTED),
                ),
                Span::styled("  (partial data)", Style::default().fg(MUTED)),
            ]));
        } else {
            let ly_net = ly.total_income - ly.total_expenses;
            let diff = m.total_expenses - ly.total_expenses;
            let pct = if ly.total_expenses > 0.0 {
                diff / ly.total_expenses * 100.0
            } else {
                0.0
            };
            let (arrow, color) = if diff <= 0.0 {
                ("↓", GREEN)
            } else {
                ("↑", RED)
            };
            text.push(Line::from(vec![
                Span::styled("  vs ", Style::default().fg(MUTED)),
                Span::styled(
                    format!("{short} '{}: {:.0}€", year_ago % 100, ly_net),
                    Style::default().fg(MUTED),
                ),
                Span::styled(
                    format!("  {arrow}{:.0}%", pct.abs()),
                    Style::default().fg(color).bold(),
                ),
            ]));
        }
    }

    let left_block = Block::default()
        .title(Span::styled(nav_title, Style::default().fg(ACCENT).bold()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED));

    f.render_widget(Paragraph::new(text).block(left_block), chunks[0]);

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(Span::styled(
                    " Savings Rate ",
                    Style::default().fg(ACCENT).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED)),
        )
        .gauge_style(Style::default().fg(net_color).bg(Color::Rgb(30, 30, 30)))
        .percent(savings_rate)
        .label(Span::styled(
            format!("{savings_rate}% saved"),
            Style::default().fg(FG).bold(),
        ));

    f.render_widget(gauge, chunks[1]);

    let ytd = app.ytd_stats();
    let ytd_net = ytd.total_income - ytd.total_expenses;
    let ytd_net_color = if ytd_net >= 0.0 { GREEN } else { RED };
    let ytd_prefix = if ytd_net >= 0.0 { "+" } else { "" };
    let ytd_text = vec![
        Line::from(vec![
            Span::styled("  Net YTD   ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{ytd_prefix}{:.0} €", ytd_net),
                Style::default().fg(ytd_net_color).bold(),
            ),
            Span::styled(
                format!("  ({:.0}% saved)", ytd.avg_savings_rate),
                Style::default().fg(MUTED),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Best      ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{} (+{:.0} €)", ytd.best_month.get(..3).unwrap_or(&ytd.best_month), ytd.best_net),
                Style::default().fg(GREEN),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Worst     ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{} ({:.0} €)", ytd.worst_month.get(..3).unwrap_or(&ytd.worst_month), ytd.worst_net),
                Style::default().fg(RED),
            ),
        ]),
    ];
    let ytd_block = Block::default()
        .title(Span::styled(
            " Year to Date ",
            Style::default().fg(GOLD).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED));
    f.render_widget(Paragraph::new(ytd_text).block(ytd_block), chunks[2]);
}

fn render_monthly_income(f: &mut Frame, app: &App, area: Rect) {
    let empty = crate::data::SingleMonth::default();
    let m = app.current_month().unwrap_or(&empty);
    let max_val = m.income.first().map(|i| i.1).unwrap_or(1.0);
    let bar_width = area.width.saturating_sub(40) as usize;

    let rows: Vec<Row> = m
        .income
        .iter()
        .map(|(name, amount)| {
            let short = name.strip_prefix("income:").unwrap_or(name);
            let bar_len = ((amount / max_val) * bar_width as f64) as usize;
            let bar = "█".repeat(bar_len.min(bar_width));

            Row::new(vec![
                Cell::from(short.to_string()).style(Style::default().fg(FG)),
                Cell::from(format!("{:.2} €", amount)).style(Style::default().fg(GREEN)),
                Cell::from(bar).style(Style::default().fg(Color::Rgb(0, 160, 80))),
            ])
        })
        .collect();

    let widths = [Constraint::Min(20), Constraint::Length(12), Constraint::Min(4)];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["Source", "Amount", ""])
                .style(Style::default().fg(MUTED).bold())
                .bottom_margin(1),
        )
        .block(
            Block::default()
                .title(Span::styled(
                    " Income ",
                    Style::default().fg(GREEN).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED)),
        );

    f.render_widget(table, area);
}

fn render_monthly_expenses(f: &mut Frame, app: &mut App, area: Rect) {
    let empty = crate::data::SingleMonth::default();
    let m = app.current_month().unwrap_or(&empty);
    let max_val = m.expenses.first().map(|e| e.1).unwrap_or(1.0);
    let bar_width = area.width.saturating_sub(40) as usize;

    let rows: Vec<Row> = m
        .expenses
        .iter()
        .map(|(name, amount)| {
            let short = name.strip_prefix("expenses:").unwrap_or(name);
            let color = if app.expense_colors { expense_color(short, app) } else { FG };
            let bar_len = ((amount / max_val) * bar_width as f64) as usize;
            let bar = "█".repeat(bar_len.min(bar_width));

            Row::new(vec![
                Cell::from(short.to_string()).style(Style::default().fg(color)),
                Cell::from(format!("{:.2} €", amount)).style(Style::default().fg(RED)),
                Cell::from(bar).style(Style::default().fg(if app.expense_colors { color } else { Color::Rgb(180, 50, 50) })),
            ])
        })
        .collect();

    let widths = [Constraint::Min(20), Constraint::Length(12), Constraint::Min(4)];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["Category", "Amount", ""])
                .style(Style::default().fg(MUTED).bold())
                .bottom_margin(1),
        )
        .row_highlight_style(Style::default().bg(Color::Rgb(40, 40, 60)).bold())
        .highlight_symbol("▶ ")
        .block(
            Block::default()
                .title(Span::styled(
                    " Expenses ",
                    Style::default().fg(RED).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED)),
        );

    f.render_stateful_widget(table, area, &mut app.expense_state);
}
