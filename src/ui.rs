use ratatui::{prelude::*, widgets::*};

use crate::app::{App, Tab};

// ── Color palette ────────────────────────────────────────────────────────────
const ACCENT: Color = Color::Cyan;
const GREEN: Color = Color::Green;
const RED: Color = Color::Red;
const GOLD: Color = Color::Yellow;
const MUTED: Color = Color::DarkGray;
const FG: Color = Color::White;

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, app: &App) {
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

fn render_content(f: &mut Frame, app: &App, area: Rect) {
    match app.tab {
        Tab::Portfolio => render_portfolio(f, app, area),
        Tab::Accounts => render_accounts(f, app, area),
        Tab::Monthly => render_monthly(f, app, area),
    }
}

// ── Status bar ────────────────────────────────────────────────────────────────

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let help = "  [1-3] tab  [↑↓/jk] navigate  [←→/hl] month  [r] refresh  [q] quit";
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
                        let pl_abs = h.value_eur - invested;
                        let abs_color = if pl_abs >= 0.0 { GREEN } else { RED };
                        let abs_prefix = if pl_abs >= 0.0 { "+" } else { "" };
                        let eur = (
                            format!("{abs_prefix}{:.2}€", pl_abs),
                            Style::default().fg(abs_color).bold(),
                        );

                        if invested > 0.0 {
                            let pct = (h.value_eur - invested) / invested * 100.0;
                            let (prefix, color) = if pct >= 0.0 {
                                ("+", GREEN)
                            } else {
                                ("", RED)
                            };
                            (
                                format!("{prefix}{:.1}%", pct),
                                Style::default().fg(color).bold(),
                                eur.0,
                                eur.1,
                            )
                        } else {
                            ("—".into(), Style::default().fg(MUTED), eur.0, eur.1)
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

    // Divider + total row
    rows.push(
        Row::new(vec![
            Cell::from("──────").style(Style::default().fg(MUTED)),
            Cell::from(""),
            Cell::from("Total").style(Style::default().fg(FG).bold()),
            Cell::from(format!("{:.2} €", total)).style(Style::default().fg(GOLD).bold()),
            Cell::from("100%").style(Style::default().fg(FG).bold()),
            Cell::from(""),
            Cell::from(""),
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
    let padding = ((y_max_raw - y_min_raw) * 0.08).max(50.0);
    let y_min = y_min_raw - padding;
    let y_max = y_max_raw + padding;

    // X-axis date labels
    let coin_entries: Vec<_> = app
        .price_history
        .iter()
        .filter(|e| e.commodity == selected_coin)
        .collect();
    let x_labels: Vec<Span> = {
        let n = coin_entries.len();
        let indices = [0, n / 4, n / 2, 3 * n / 4, n.saturating_sub(1)];
        indices
            .iter()
            .filter_map(|&i| coin_entries.get(i))
            .map(|e| Span::styled(e.date.format("%d.%m").to_string(), Style::default().fg(MUTED)))
            .collect()
    };

    // Y-axis EUR labels
    let y_labels: Vec<Span> = {
        let step = (y_max - y_min) / 4.0;
        (0..=4)
            .map(|i| {
                let v = y_min + step * i as f64;
                Span::styled(format!("{:.0}€", v), Style::default().fg(MUTED))
            })
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

fn render_accounts(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(area);

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
    f.render_widget(Paragraph::new(nw_text).block(nw_block), chunks[0]);

    let table_area = chunks[1];

    let max_amount = app
        .account_balances
        .iter()
        .map(|b| b.amount.abs())
        .fold(0.0_f64, f64::max);

    let bar_width = (table_area.width as f64 * 0.2) as usize;

    let rows: Vec<Row> = app
        .account_balances
        .iter()
        .skip(app.account_scroll)
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
                Cell::from(b.account.clone()).style(Style::default().fg(FG)),
                Cell::from(amount_str).style(amount_style),
                Cell::from(bar).style(Style::default().fg(Color::Rgb(0, 130, 130))),
            ])
        })
        .collect();

    let widths = [Constraint::Min(36), Constraint::Length(16), Constraint::Min(10)];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["Account", "Balance (EUR)", ""])
                .style(Style::default().fg(MUTED).bold())
                .bottom_margin(1),
        )
        .block(
            Block::default()
                .title(Span::styled(
                    " Asset Balances ",
                    Style::default().fg(ACCENT).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(MUTED)),
        );

    f.render_widget(table, table_area);
}

// ── Monthly tab ───────────────────────────────────────────────────────────────

fn render_monthly(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::vertical([Constraint::Length(8), Constraint::Min(0)]).split(area);

    render_monthly_summary(f, app, chunks[0]);

    let detail_chunks =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

    render_monthly_income(f, app, detail_chunks[0]);
    render_monthly_expenses(f, app, detail_chunks[1]);
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

    let chunks = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let nav_title = format!(" ◀ {} ▶ ", m.month_name);

    let text = vec![
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

fn render_monthly_expenses(f: &mut Frame, app: &App, area: Rect) {
    let empty = crate::data::SingleMonth::default();
    let m = app.current_month().unwrap_or(&empty);
    let max_val = m.expenses.first().map(|e| e.1).unwrap_or(1.0);
    let bar_width = area.width.saturating_sub(40) as usize;

    let rows: Vec<Row> = m
        .expenses
        .iter()
        .skip(app.expense_scroll)
        .map(|(name, amount)| {
            let short = name.strip_prefix("expenses:").unwrap_or(name);
            let bar_len = ((amount / max_val) * bar_width as f64) as usize;
            let bar = "█".repeat(bar_len.min(bar_width));

            Row::new(vec![
                Cell::from(short.to_string()).style(Style::default().fg(FG)),
                Cell::from(format!("{:.2} €", amount)).style(Style::default().fg(RED)),
                Cell::from(bar).style(Style::default().fg(Color::Rgb(180, 50, 50))),
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

    f.render_widget(table, area);
}
