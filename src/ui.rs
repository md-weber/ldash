use chrono::Datelike;
use ratatui::{prelude::*, widgets::*};

use crate::app::{budget_matches, budget_spent, App, Tab};
use crate::config::Config;

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

fn nice_y_axis(
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

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
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
    let h = 18u16.min(area.height.saturating_sub(4));
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
        ("/", "Search transactions"),
        ("y / Y", "Year back/fwd (Monthly)"),
        ("Y", "Copy view to clipboard"),
        ("s", "Toggle chart stacked/unstacked"),
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
    let help = "  [1-3] tab  [↑↓/jk] navigate  [←→/hl] month/range  [r] refresh  [?] help  [q] quit";
    let text = Line::from(vec![
        Span::styled(&app.status_msg, Style::default().fg(ACCENT)),
        Span::styled(help, Style::default().fg(MUTED)),
    ]);
    f.render_widget(Paragraph::new(text), area);
}

// ── Portfolio tab ─────────────────────────────────────────────────────────────

fn coin_color(commodity: &str) -> Color {
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
fn series_interp(series: &[(f64, f64)], day: f64) -> f64 {
    match series.iter().rposition(|p| p.0 <= day) {
        Some(i) => series[i].1,
        None => 0.0,
    }
}

fn series_value_at_day(series: &crate::data::CoinChartSeries, day: f64) -> f64 {
    series_interp(&series.investment, day)
        + series_interp(&series.price_growth, day)
        + series_interp(&series.staking_growth, day)
}

/// Compute (pl_abs, basis_for_pct) for a single holding given the current range.
///
/// ALL range: pl = current_value − FIFO_basis.
/// Sub-ranges (3M/6M/YTD): change in unrealized P/L over the period.
///   pl = (value_now − basis_now) − (value_at_start − basis_at_start)
///   This correctly weights each buy by its actual purchase price.
///
/// If a sell within the range distorts the result (|pl| > current value),
/// we fall back to a price-only estimate on current holdings.
fn holding_pl(
    app: &App,
    h: &crate::data::CryptoHolding,
    series: &crate::data::CoinChartSeries,
    first_date: chrono::NaiveDate,
) -> (f64, f64) {
    let invested = series.total_invested();
    let min_x = app.portfolio_range_min_x(first_date);
    let first_x = series.investment.first().map(|p| p.0).unwrap_or(0.0);

    if min_x <= first_x {
        (h.value_eur - invested, invested)
    } else {
        let current_pl = h.value_eur - invested;
        let basis_at_start = series_interp(&series.investment, min_x);
        let value_at_start = series_value_at_day(series, min_x);
        let pl_at_start = value_at_start - basis_at_start;
        let pl_range = current_pl - pl_at_start;
        let denom = if value_at_start > 0.0 { value_at_start } else { invested };

        if pl_range.abs() > h.value_eur {
            let price_at_start = series_interp(&series.price, min_x);
            if price_at_start > 0.0 {
                let val_start = h.amount * price_at_start;
                return (h.value_eur - val_start, val_start);
            }
            return (h.value_eur - invested, invested);
        }

        (pl_range, denom)
    }
}


fn render_portfolio(f: &mut Frame, app: &App, area: Rect) {
    if area.width < 100 {
        let chunks = Layout::vertical([
            Constraint::Percentage(45),
            Constraint::Percentage(55),
        ])
        .split(area);
        render_holdings_table(f, app, chunks[0]);
        render_price_chart(f, app, chunks[1]);
    } else {
        let chunks =
            Layout::horizontal([Constraint::Percentage(38), Constraint::Percentage(62)])
                .split(area);

        let left = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(app.holdings.len().min(8) as u16 + 2),
        ])
        .split(chunks[0]);

        render_holdings_table(f, app, left[0]);
        render_allocation_chart(f, app, left[1]);
        render_price_chart(f, app, chunks[1]);
    }
}

fn render_holdings_table(f: &mut Frame, app: &App, area: Rect) {
    let total = app.total_portfolio_value();
    let narrow = area.width < 100;
    let very_narrow = area.width < 80;

    let mut coin_first_dates: std::collections::HashMap<&str, chrono::NaiveDate> =
        std::collections::HashMap::new();
    for e in &app.price_history {
        coin_first_dates.entry(&e.commodity).or_insert(e.date);
    }

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
                app.config.fmt_amount(h.price_eur, 0)
            } else if h.price_eur >= 100.0 {
                app.config.fmt_amount(h.price_eur, 2)
            } else {
                app.config.fmt_amount(h.price_eur, 3)
            };

            let amt_str = if h.amount >= 100.0 {
                app.config.fmt_number(h.amount, 2)
            } else {
                app.config.fmt_number(h.amount, 4)
            };

            let value_str = app.config.fmt_amount(h.value_eur, 2);

            let indicator = if selected { "▶ " } else { "  " };

            let pct = if total > 0.0 {
                format!("{:.1}%", h.value_eur / total * 100.0)
            } else {
                "—".into()
            };

            let (pl_pct_str, pl_pct_style, pl_eur_str, pl_eur_style) =
                match app.coin_chart_cache.get(&h.commodity) {
                    Some(series) if !series.investment.is_empty() => {
                        let first_date = coin_first_dates
                            .get(h.commodity.as_str())
                            .copied()
                            .unwrap_or(chrono::Local::now().date_naive());
                        let (pl_abs, basis) = holding_pl(app, h, series, first_date);

                        if basis > 0.0 {
                            let abs_color = if pl_abs >= 0.0 { GREEN } else { RED };
                            let abs_prefix = if pl_abs >= 0.0 { "+" } else { "" };
                            let pct = pl_abs / basis * 100.0;
                            let pct_clamped = pct.clamp(-9999.0, 9999.0);
                            let (prefix, color) =
                                if pct >= 0.0 { ("+", GREEN) } else { ("", RED) };
                            let pct_str = if pct.abs() > 9999.0 {
                                format!("{prefix}{:.0}%", pct_clamped)
                            } else {
                                format!("{prefix}{:.1}%", pct)
                            };
                            (
                                pct_str,
                                Style::default().fg(color).bold(),
                                format!("{abs_prefix}{}", app.config.fmt_amount_compact(pl_abs, 2)),
                                Style::default().fg(abs_color).bold(),
                            )
                        } else if basis < 0.0 {
                            (
                                "—".into(),
                                Style::default().fg(MUTED),
                                app.config.fmt_amount_compact(pl_abs, 2),
                                Style::default().fg(MUTED),
                            )
                        } else if h.value_eur > 0.0 {
                            (
                                "∞".into(),
                                Style::default().fg(GREEN).bold(),
                                format!("+{}", app.config.fmt_amount_compact(h.value_eur, 2)),
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

            let mut cells = vec![
                Cell::from(format!("{indicator}{}", h.commodity)).style(coin_style),
            ];
            if !very_narrow {
                cells.push(
                    Cell::from(amt_str)
                        .style(Style::default().fg(if selected { GOLD } else { MUTED })),
                );
            }
            cells.extend([
                Cell::from(price_str).style(Style::default().fg(ACCENT)),
                Cell::from(value_str).style(Style::default().fg(GREEN)),
                Cell::from(pct).style(Style::default().fg(FG)),
                Cell::from(pl_pct_str).style(pl_pct_style),
            ]);
            if !very_narrow {
                cells.push(Cell::from(pl_eur_str).style(pl_eur_style));
            }

            Row::new(cells)
        })
        .collect();

    let (pl_abs, pl_pct) = {
        let mut total_pl = 0.0_f64;
        let mut total_basis = 0.0_f64;
        for h in &app.holdings {
            if let Some(s) = app.coin_chart_cache.get(&h.commodity) {
                if !s.investment.is_empty() {
                    let first_date = coin_first_dates
                        .get(h.commodity.as_str())
                        .copied()
                        .unwrap_or(chrono::Local::now().date_naive());
                    let (pl, basis) = holding_pl(app, h, s, first_date);
                    total_pl += pl;
                    if basis > 0.0 {
                        total_basis += basis;
                    }
                }
            }
        }
        let pct = if total_basis > 0.0 {
            total_pl / total_basis * 100.0
        } else {
            0.0
        };
        (total_pl, pct)
    };
    let pl_color = if pl_abs >= 0.0 { GREEN } else { RED };
    let pl_prefix = if pl_abs >= 0.0 { "+" } else { "" };

    let mut total_cells = vec![
        Cell::from("──────").style(Style::default().fg(MUTED)),
    ];
    if !very_narrow {
        total_cells.push(Cell::from(""));
    }
    total_cells.extend([
        Cell::from("Total").style(Style::default().fg(FG).bold()),
        Cell::from(app.config.fmt_amount(total, 2)).style(Style::default().fg(GOLD).bold()),
        Cell::from("100%").style(Style::default().fg(FG).bold()),
        Cell::from(format!("{pl_prefix}{:.1}%", pl_pct))
            .style(Style::default().fg(pl_color).bold()),
    ]);
    if !very_narrow {
        total_cells.push(
            Cell::from(format!("{pl_prefix}{}", app.config.fmt_amount_compact(pl_abs, 2)))
                .style(Style::default().fg(pl_color).bold()),
        );
    }
    rows.push(Row::new(total_cells).height(1));

    let widths: Vec<Constraint> = if very_narrow {
        vec![
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Length(11),
            Constraint::Length(6),
            Constraint::Length(8),
        ]
    } else if narrow {
        vec![
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(9),
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Length(11),
        ]
    } else {
        vec![
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(11),
            Constraint::Length(12),
            Constraint::Length(7),
            Constraint::Length(8),
            Constraint::Length(14),
        ]
    };

    let pl_header = format!("P/L {}", app.config.currency_symbol);
    let mut header_cells: Vec<String> = vec!["Coin".into()];
    if !very_narrow {
        header_cells.push("Amount".into());
    }
    header_cells.extend([
        "Price".into(),
        "Value".into(),
        "Alloc".into(),
        "P/L %".into(),
    ]);
    if !very_narrow {
        header_cells.push(pl_header);
    }

    let table = Table::new(rows, widths)
        .header(
            Row::new(header_cells)
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

    let hint_area = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(2),
        width: area.width.saturating_sub(2),
        height: 1,
    };
    f.render_widget(
        Paragraph::new("  ↑↓ select coin for chart  │  P/L is unrealized only. Sells reflected in Accounts tab.").style(Style::default().fg(MUTED)),
        hint_area,
    );
}

fn render_allocation_chart(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(" Allocation ", Style::default().fg(ACCENT).bold()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let total = app.total_portfolio_value();
    let label_w = 6u16;
    let pct_w = 6u16;
    let bar_area_w = inner.width.saturating_sub(label_w + pct_w + 2);

    for (i, h) in app.holdings.iter().enumerate() {
        if i as u16 >= inner.height {
            break;
        }
        let y = inner.y + i as u16;
        let pct = if total > 0.0 { h.value_eur / total * 100.0 } else { 0.0 };
        let bar_len = ((pct / 100.0) * bar_area_w as f64) as usize;
        let color = coin_color(&h.commodity);

        let label = Span::styled(
            format!(" {:<w$}", h.commodity, w = (label_w - 1) as usize),
            Style::default().fg(color).bold(),
        );
        f.render_widget(Paragraph::new(Line::from(label)),
            Rect { x: inner.x, y, width: label_w, height: 1 });

        let pct_str = Span::styled(
            format!("{:>4.1}% ", pct),
            Style::default().fg(MUTED),
        );
        f.render_widget(Paragraph::new(Line::from(pct_str)),
            Rect { x: inner.x + label_w, y, width: pct_w, height: 1 });

        let bar = "█".repeat(bar_len.min(bar_area_w as usize));
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(bar, Style::default().fg(color)))),
            Rect { x: inner.x + label_w + pct_w, y, width: bar_area_w, height: 1 },
        );
    }
}

fn render_price_chart(f: &mut Frame, app: &App, area: Rect) {
    let selected_coin = app.selected_coin().unwrap_or("SOL");
    let range_label = app.portfolio_range.label();

    let mode_label = if app.chart_stacked { "stacked" } else { "unstacked" };
    let block = Block::default()
        .title(Span::styled(
            format!(" {} Portfolio Analysis [{range_label}] [{mode_label}]  ◀ ▶ ", selected_coin),
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

    let coin_entries: Vec<_> = app
        .price_history
        .iter()
        .filter(|e| e.commodity == selected_coin)
        .collect();
    let first_date = coin_entries.first().map(|e| e.date).unwrap_or(chrono::Local::now().date_naive());
    let today = chrono::Local::now().date_naive();
    let today_x = (today - first_date).num_days() as f64;

    let min_x = app.portfolio_range_min_x(first_date);

    let filtered_inv: Vec<(f64, f64)> = series.investment.iter().filter(|p| p.0 >= min_x).copied().collect();
    let filtered_price: Vec<(f64, f64)> = series.price_growth.iter().filter(|p| p.0 >= min_x).copied().collect();
    let filtered_staking: Vec<(f64, f64)> = series.staking_growth.iter().filter(|p| p.0 >= min_x).copied().collect();

    let (line2_data, line3_data, line2_name, line3_name): (Vec<(f64, f64)>, Vec<(f64, f64)>, &str, &str) =
        if app.chart_stacked {
            let purchased: Vec<(f64, f64)> = filtered_inv.iter()
                .zip(filtered_price.iter())
                .map(|(inv, pg)| (inv.0, inv.1 + pg.1))
                .collect();
            let total: Vec<(f64, f64)> = filtered_inv.iter()
                .zip(filtered_price.iter())
                .zip(filtered_staking.iter())
                .map(|((inv, pg), sg)| (inv.0, inv.1 + pg.1 + sg.1))
                .collect();
            (purchased, total, "Purchased value", "Total value")
        } else {
            (filtered_price.clone(), filtered_staking.clone(), "Price gain", "Staking gain")
        };

    let x_min = min_x;
    let x_max = today_x.max(filtered_inv.last().map(|p| p.0).unwrap_or(1.0));

    let all_y = filtered_inv.iter()
        .chain(line2_data.iter())
        .chain(line3_data.iter())
        .map(|p| p.1);
    let y_min_raw = all_y.clone().fold(f64::INFINITY, f64::min);
    let y_max_raw = all_y.fold(f64::NEG_INFINITY, f64::max);
    let (y_min, y_max, y_labels) = nice_y_axis(y_min_raw, y_max_raw, 4, &app.config);

    let filtered_entries: Vec<_> = coin_entries.iter()
        .filter(|e| {
            let day = (e.date - first_date).num_days() as f64;
            day >= min_x
        })
        .collect();
    let x_labels: Vec<Span> = {
        let n = filtered_entries.len();
        let count = if area.width < 60 { 3 } else { 5 };
        let indices: Vec<usize> = (0..count).map(|i| i * n.saturating_sub(1) / (count - 1).max(1)).collect();
        indices
            .iter()
            .filter_map(|&i| filtered_entries.get(i))
            .map(|e| Span::styled(e.date.format("%b %y").to_string(), Style::default().fg(MUTED)))
            .collect()
    };

    let ds_investment = Dataset::default()
        .name("Invested")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(GOLD))
        .data(&filtered_inv);

    let ds_line2 = Dataset::default()
        .name(line2_name)
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(ACCENT))
        .data(&line2_data);

    let ds_line3 = Dataset::default()
        .name(line3_name)
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(GREEN))
        .data(&line3_data);

    let chart = Chart::new(vec![ds_investment, ds_line2, ds_line3])
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
    let (y_min, y_max, y_labels) = nice_y_axis(y_min_raw, y_max_raw, 4, &app.config);

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
            app.config.fmt_amount(net_worth, 2),
            Style::default().fg(GOLD).bold(),
        ),
    ]);
    let nw_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED));
    f.render_widget(Paragraph::new(nw_text).block(nw_block), chunks[1]);

    let has_goals = !app.config.goals.is_empty();

    if app.account_detail.is_some() {
        let txns = app.account_detail.as_ref().unwrap();
        let name = app.detail_account_name.as_ref().unwrap();
        render_account_detail(f, txns, name, &app.config, chunks[2]);
    } else if has_goals {
        let goal_h = (app.config.goals.len() as u16 + 2).min(8);
        let bottom = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(goal_h),
        ]).split(chunks[2]);

        if !app.liabilities.is_empty() {
            let split = Layout::vertical([
                Constraint::Percentage(70),
                Constraint::Percentage(30),
            ]).split(bottom[0]);
            render_accounts_table(f, app, split[0]);
            render_liabilities_table(f, app, split[1]);
        } else {
            render_accounts_table(f, app, bottom[0]);
        }

        render_goals(f, app, bottom[1]);
    } else if !app.liabilities.is_empty() {
        let split = Layout::vertical([
            Constraint::Percentage(70),
            Constraint::Percentage(30),
        ])
        .split(chunks[2]);
        render_accounts_table(f, app, split[0]);
        render_liabilities_table(f, app, split[1]);
    } else {
        render_accounts_table(f, app, chunks[2]);
    }
}

fn render_detail_with_title(
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

fn render_account_detail(
    f: &mut Frame,
    txns: &[crate::data::Transaction],
    name: &str,
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
    let narrow = area.width < 100;
    let max_amount = app
        .account_balances
        .iter()
        .map(|b| b.amount.abs())
        .fold(0.0_f64, f64::max);

    let bar_width = if narrow { 0 } else { (area.width as f64 * 0.2) as usize };

    let rows: Vec<Row> = app
        .account_balances
        .iter()
        .map(|b| {
            let amount_style = if b.amount >= 0.0 {
                Style::default().fg(GREEN)
            } else {
                Style::default().fg(RED)
            };

            let amount_str = format!("{:>15}", app.config.fmt_amount(b.amount, 2));

            let name = if narrow && b.account.len() > 30 {
                format!("  {}…", &b.account[..29])
            } else {
                format!("  {}", b.account)
            };

            let mut cells = vec![
                Cell::from(name).style(Style::default().fg(FG)),
                Cell::from(amount_str).style(amount_style),
            ];

            if !narrow {
                let bar_len = if max_amount > 0.0 {
                    ((b.amount.abs() / max_amount) * bar_width as f64) as usize
                } else {
                    0
                };
                let bar = "█".repeat(bar_len);
                cells.push(Cell::from(bar).style(Style::default().fg(Color::Rgb(0, 130, 130))));
            }

            Row::new(cells)
        })
        .collect();

    let widths: Vec<Constraint> = if narrow {
        vec![Constraint::Min(30), Constraint::Length(16)]
    } else {
        vec![Constraint::Min(38), Constraint::Length(16), Constraint::Min(10)]
    };

    let bal_header = format!("Balance ({})", app.config.currency_symbol);
    let mut header: Vec<String> = vec!["Account".into(), bal_header];
    if !narrow {
        header.push(String::new());
    }

    let table = Table::new(rows, widths)
        .header(
            Row::new(header)
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

fn render_liabilities_table(f: &mut Frame, app: &App, area: Rect) {
    let rows: Vec<Row> = app
        .liabilities
        .iter()
        .map(|b| {
            let amount_str = format!("{:>15}", app.config.fmt_amount(b.amount, 2));
            Row::new(vec![
                Cell::from(format!("  {}", b.account)).style(Style::default().fg(FG)),
                Cell::from(amount_str).style(Style::default().fg(RED)),
            ])
        })
        .collect();

    let widths = [Constraint::Min(38), Constraint::Length(16)];

    let total_liab: f64 = app.liabilities.iter().map(|b| b.amount).sum();
    let title = format!(" Liabilities  ({}) ", app.config.fmt_amount(total_liab, 2));

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![
                "Account".to_string(),
                format!("Balance ({})", app.config.currency_symbol),
            ])
                .style(Style::default().fg(MUTED).bold())
                .bottom_margin(1),
        )
        .block(
            Block::default()
                .title(Span::styled(title, Style::default().fg(RED).bold()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED)),
        );

    f.render_widget(table, area);
}

fn render_goals(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(" Savings Goals ", Style::default().fg(GOLD).bold()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let goals = app.goal_progress();
    for (i, g) in goals.iter().enumerate() {
        if i as u16 >= inner.height {
            break;
        }
        let y = inner.y + i as u16;
        let color = if g.pct >= 100.0 { GREEN }
                   else if g.pct >= 60.0 { GOLD }
                   else { ACCENT };

        let label_w = 18u16.min(inner.width / 3);
        let pct_w = 22u16;
        let bar_w = inner.width.saturating_sub(label_w + pct_w);

        let label = Span::styled(
            format!(" {:<w$}", g.name, w = (label_w - 1) as usize),
            Style::default().fg(FG),
        );
        f.render_widget(Paragraph::new(Line::from(label)),
            Rect { x: inner.x, y, width: label_w, height: 1 });

        let filled = ((g.pct / 100.0).min(1.0) * bar_w as f64) as usize;
        let empty_b = bar_w as usize - filled;
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty_b));
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(bar, Style::default().fg(color)))),
            Rect { x: inner.x + label_w, y, width: bar_w, height: 1 },
        );

        let info = Span::styled(
            format!(
                " {}/{} {:>3.0}%",
                app.config.fmt_amount_compact(g.current, 0),
                app.config.fmt_amount_compact(g.target, 0),
                g.pct
            ),
            Style::default().fg(MUTED),
        );
        f.render_widget(Paragraph::new(Line::from(info)),
            Rect { x: inner.x + label_w + bar_w, y, width: pct_w, height: 1 });
    }
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

    let chart_title = if app.monthly_year_offset == 0 {
        " Income vs Expenses ".to_string()
    } else {
        format!(" Income vs Expenses ({})  [y/Y] ", app.displayed_year())
    };

    let mut chart = BarChart::default()
        .block(
            Block::default()
                .title(Span::styled(
                    chart_title,
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
    let narrow = area.width < 100;
    let very_narrow = area.width < 80;

    let chunks = Layout::vertical([
        Constraint::Length(12),
        Constraint::Length(if narrow { 20 } else { 14 }),
        Constraint::Min(0),
    ])
    .split(area);

    render_monthly_chart(f, app, chunks[0]);
    render_monthly_summary(f, app, chunks[1]);

    let detail_chunks = if very_narrow {
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2])
    } else {
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2])
    };

    render_monthly_income(f, app, detail_chunks[0]);

    if let (Some(txns), Some(name)) = (&app.expense_detail, &app.detail_expense_name) {
        let short = name.strip_prefix("expenses:").unwrap_or(name);
        let month = app
            .current_month()
            .map(|m| m.month_name.as_str())
            .unwrap_or("");
        let title = format!(" {} — {}  [Esc back] ", short, month);
        render_detail_with_title(f, txns, &title, &app.config, detail_chunks[1]);
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

    let narrow = area.width < 100;
    let chunks = if narrow {
        Layout::vertical([
            Constraint::Length(8),
            Constraint::Length(4),
            Constraint::Min(0),
        ])
        .split(area)
    } else {
        Layout::horizontal([
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(40),
        ])
        .split(area)
    };

    let nav_title = if app.monthly_year_offset != 0 {
        format!(" ◀ {} {} ▶  [y/Y] ", m.month_name, app.displayed_year())
    } else {
        format!(" ◀ {} ▶ ", m.month_name)
    };

    let mut text = vec![
        Line::from(vec![
            Span::styled("  Income    ", Style::default().fg(MUTED)),
            Span::styled(
                app.config.fmt_amount(m.total_income, 2),
                Style::default().fg(GREEN).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Expenses  ", Style::default().fg(MUTED)),
            Span::styled(
                app.config.fmt_amount(m.total_expenses, 2),
                Style::default().fg(RED).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Net       ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{net_prefix}{}", app.config.fmt_amount(net, 2)),
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
                    format!(
                        "{short} '{}: {}",
                        year_ago % 100,
                        app.config.fmt_amount_compact(ly_net, 0)
                    ),
                    Style::default().fg(MUTED),
                ),
                Span::styled(
                    format!("  {arrow}{:.0}%", pct.abs()),
                    Style::default().fg(color).bold(),
                ),
            ]));
        }
    }

    if !app.config.budgets.is_empty() {
        let budgets = app.budget_status();
        let over: Vec<_> = budgets.iter().filter(|b| b.pct > 100.0).collect();
        text.push(Line::from(""));
        if over.is_empty() {
            text.push(Line::from(vec![
                Span::styled("  ✓ ", Style::default().fg(GREEN)),
                Span::styled("All budgets on track", Style::default().fg(GREEN)),
            ]));
        } else {
            text.push(Line::from(vec![
                Span::styled("  ⚠ ", Style::default().fg(RED)),
                Span::styled(
                    format!("{} over budget:", over.len()),
                    Style::default().fg(RED).bold(),
                ),
            ]));
            for b in &over {
                let short = b.category.strip_prefix("expenses:").unwrap_or(&b.category);
                text.push(Line::from(vec![
                    Span::styled(format!("    {short}: "), Style::default().fg(FG)),
                    Span::styled(
                        format!(
                            "{}/{} ({:.0}%)",
                            app.config.fmt_amount_compact(b.spent, 0),
                            app.config.fmt_amount_compact(b.limit, 0),
                            b.pct
                        ),
                        Style::default().fg(RED),
                    ),
                ]));
            }
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

    let ytd_block = Block::default()
        .title(Span::styled(
            " Year to Date ",
            Style::default().fg(GOLD).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED));
    let ytd_area = ytd_block.inner(chunks[2]);
    f.render_widget(ytd_block, chunks[2]);

    let ytd_split = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
    ]).split(ytd_area);

    let ytd_text = vec![
        Line::from(vec![
            Span::styled("  Net YTD   ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{ytd_prefix}{}", app.config.fmt_amount(ytd_net, 0)),
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
                format!(
                    "{} (+{})",
                    ytd.best_month.get(..3).unwrap_or(&ytd.best_month),
                    app.config.fmt_amount(ytd.best_net, 0)
                ),
                Style::default().fg(GREEN),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Worst     ", Style::default().fg(MUTED)),
            Span::styled(
                format!(
                    "{} ({})",
                    ytd.worst_month.get(..3).unwrap_or(&ytd.worst_month),
                    app.config.fmt_amount(ytd.worst_net, 0)
                ),
                Style::default().fg(RED),
            ),
        ]),
    ];
    f.render_widget(Paragraph::new(ytd_text), ytd_split[0]);

    let spark_months = &app.monthly.months[app.monthly.months.len().saturating_sub(6)..];
    let inc_data: Vec<u64> = spark_months.iter().map(|m| m.total_income as u64).collect();
    let exp_data: Vec<u64> = spark_months.iter().map(|m| m.total_expenses as u64).collect();
    let net_data: Vec<u64> = spark_months.iter()
        .map(|m| (m.total_income - m.total_expenses).max(0.0) as u64).collect();

    let spark_rows = Layout::vertical([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ]).split(ytd_split[1]);

    let spark_inc = Sparkline::default()
        .block(Block::default()
            .title(Span::styled(" Income ▁▃▅ ", Style::default().fg(GREEN)))
            .borders(Borders::TOP)
            .border_style(Style::default().fg(MUTED)))
        .data(&inc_data)
        .style(Style::default().fg(GREEN));
    f.render_widget(spark_inc, spark_rows[0]);

    let spark_exp = Sparkline::default()
        .block(Block::default()
            .title(Span::styled(" Expenses ▁▃▅ ", Style::default().fg(RED)))
            .borders(Borders::TOP)
            .border_style(Style::default().fg(MUTED)))
        .data(&exp_data)
        .style(Style::default().fg(RED));
    f.render_widget(spark_exp, spark_rows[1]);

    let spark_net = Sparkline::default()
        .block(Block::default()
            .title(Span::styled(" Net ▁▃▅ ", Style::default().fg(GOLD)))
            .borders(Borders::TOP)
            .border_style(Style::default().fg(MUTED)))
        .data(&net_data)
        .style(Style::default().fg(GOLD));
    f.render_widget(spark_net, spark_rows[2]);
}

fn render_monthly_income(f: &mut Frame, app: &App, area: Rect) {
    let narrow = area.width < 100;
    let empty = crate::data::SingleMonth::default();
    let m = app.current_month().unwrap_or(&empty);
    let max_val = m.income.first().map(|i| i.1).unwrap_or(1.0);
    let bar_width = if narrow { 0 } else { area.width.saturating_sub(40) as usize };

    let rows: Vec<Row> = m
        .income
        .iter()
        .map(|(name, amount)| {
            let short = name.strip_prefix("income:").unwrap_or(name);
            let mut cells = vec![
                Cell::from(short.to_string()).style(Style::default().fg(FG)),
                Cell::from(app.config.fmt_amount(*amount, 2)).style(Style::default().fg(GREEN)),
            ];
            if !narrow {
                let bar_len = ((amount / max_val) * bar_width as f64) as usize;
                let bar = "█".repeat(bar_len.min(bar_width));
                cells.push(Cell::from(bar).style(Style::default().fg(Color::Rgb(0, 160, 80))));
            }
            Row::new(cells)
        })
        .collect();

    let widths: Vec<Constraint> = if narrow {
        vec![Constraint::Min(20), Constraint::Length(12)]
    } else {
        vec![Constraint::Min(20), Constraint::Length(12), Constraint::Min(4)]
    };

    let mut header = vec!["Source", "Amount"];
    if !narrow {
        header.push("");
    }

    let table = Table::new(rows, widths)
        .header(
            Row::new(header)
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
    let narrow = area.width < 100;
    let has_budgets = !app.config.budgets.is_empty();
    let empty = crate::data::SingleMonth::default();
    let m = app.current_month().unwrap_or(&empty);
    let max_val = m.expenses.first().map(|e| e.1).unwrap_or(1.0);
    let bar_width = if narrow { 0 } else { area.width.saturating_sub(if has_budgets { 56 } else { 40 }) as usize };

    let rows: Vec<Row> = m
        .expenses
        .iter()
        .map(|(name, amount)| {
            let short = name.strip_prefix("expenses:").unwrap_or(name);
            let color = if app.expense_colors {
                expense_color(short, app)
            } else {
                FG
            };
            let mut cells = vec![
                Cell::from(short.to_string()).style(Style::default().fg(color)),
                Cell::from(app.config.fmt_amount(*amount, 2)).style(Style::default().fg(RED)),
            ];
            if !narrow {
                let bar_len = ((amount / max_val) * bar_width as f64) as usize;
                let bar = "█".repeat(bar_len.min(bar_width));
                cells.push(Cell::from(bar).style(Style::default().fg(if app.expense_colors {
                    color
                } else {
                    Color::Rgb(180, 50, 50)
                })));
            }
            if has_budgets {
                let matched = app.config.budgets.iter()
                    .find(|(cat, _)| budget_matches(cat, name));
                if let Some((cat, &limit)) = matched {
                    let spent = budget_spent(cat, &m.expenses);
                    let pct = if limit > 0.0 { spent / limit * 100.0 } else { 0.0 };
                    let bar_color = if pct < 80.0 { GREEN }
                                    else if pct <= 100.0 { GOLD }
                                    else { RED };
                    let filled = ((pct / 100.0).min(1.0) * 8.0) as usize;
                    let empty_b = 8 - filled;
                    let bar = format!("[{}{}] {:>3.0}%",
                        "█".repeat(filled), "░".repeat(empty_b), pct);
                    cells.push(Cell::from(bar).style(Style::default().fg(bar_color)));
                } else {
                    cells.push(Cell::from(""));
                }
            }
            Row::new(cells)
        })
        .collect();

    let widths: Vec<Constraint> = if narrow {
        let mut w = vec![Constraint::Min(20), Constraint::Length(12)];
        if has_budgets { w.push(Constraint::Length(16)); }
        w
    } else {
        let mut w = vec![Constraint::Min(20), Constraint::Length(12), Constraint::Min(4)];
        if has_budgets { w.push(Constraint::Length(16)); }
        w
    };

    let mut header = vec!["Category", "Amount"];
    if !narrow {
        header.push("");
    }
    if has_budgets {
        header.push("Budget");
    }

    let table = Table::new(rows, widths)
        .header(
            Row::new(header)
                .style(Style::default().fg(MUTED).bold())
                .bottom_margin(1),
        )
        .row_highlight_style(Style::default().bg(Color::Rgb(40, 40, 60)).bold())
        .highlight_symbol("▶ ")
        .block(
            Block::default()
                .title(Span::styled(
                    if has_budgets {
                        format!(" Expenses ({} budgets) ", app.config.budgets.len())
                    } else {
                        " Expenses ".to_string()
                    },
                    Style::default().fg(RED).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED)),
        );

    f.render_stateful_widget(table, area, &mut app.expense_state);
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
