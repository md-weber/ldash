use ratatui::{prelude::*, widgets::*};

use super::{coin_color, nice_y_axis, Theme};
use crate::app::App;

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
        let denom = if value_at_start > 0.0 {
            value_at_start
        } else {
            invested
        };

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

pub(super) fn render_portfolio(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    if area.width < 100 {
        let chunks =
            Layout::vertical([Constraint::Percentage(45), Constraint::Percentage(55)]).split(area);
        render_holdings_table(f, app, chunks[0], theme);
        render_price_chart(f, app, chunks[1], theme);
    } else {
        let chunks = Layout::horizontal([Constraint::Percentage(38), Constraint::Percentage(62)])
            .split(area);

        let left = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(app.holdings.len().min(8) as u16 + 2),
        ])
        .split(chunks[0]);

        render_holdings_table(f, app, left[0], theme);
        render_allocation_chart(f, app, left[1], theme);
        render_price_chart(f, app, chunks[1], theme);
    }
}

fn render_holdings_table(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
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
                Style::default().fg(theme.gold).bold()
            } else {
                Style::default().fg(theme.fg)
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
                            let abs_color = if pl_abs >= 0.0 {
                                theme.positive
                            } else {
                                theme.negative
                            };
                            let abs_prefix = if pl_abs >= 0.0 { "+" } else { "" };
                            let pct = pl_abs / basis * 100.0;
                            let (prefix, color) = if pct >= 0.0 {
                                ("+", theme.positive)
                            } else {
                                ("", theme.negative)
                            };
                            // Above ±9999% the signed prefix + 4-digit integer
                            // saturates a typical 7-char column; drop the decimal.
                            let pct_str = if pct.abs() > 9999.0 {
                                let pct_clamped = pct.clamp(-9999.0, 9999.0);
                                format!("{prefix}{pct_clamped:.0}%")
                            } else {
                                format!("{prefix}{pct:.1}%")
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
                                Style::default().fg(theme.muted),
                                app.config.fmt_amount_compact(pl_abs, 2),
                                Style::default().fg(theme.muted),
                            )
                        } else if h.value_eur > 0.0 {
                            (
                                "∞".into(),
                                Style::default().fg(theme.positive).bold(),
                                format!("+{}", app.config.fmt_amount_compact(h.value_eur, 2)),
                                Style::default().fg(theme.positive).bold(),
                            )
                        } else {
                            (
                                "—".into(),
                                Style::default().fg(theme.muted),
                                "—".into(),
                                Style::default().fg(theme.muted),
                            )
                        }
                    }
                    _ => (
                        "—".into(),
                        Style::default().fg(theme.muted),
                        "—".into(),
                        Style::default().fg(theme.muted),
                    ),
                };

            let mut cells =
                vec![Cell::from(format!("{indicator}{}", h.commodity)).style(coin_style)];
            if !very_narrow {
                cells.push(Cell::from(amt_str).style(Style::default().fg(if selected {
                    theme.gold
                } else {
                    theme.muted
                })));
            }
            cells.extend([
                Cell::from(price_str).style(Style::default().fg(theme.accent)),
                Cell::from(value_str).style(Style::default().fg(theme.positive)),
                Cell::from(pct).style(Style::default().fg(theme.fg)),
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
    let pl_color = if pl_abs >= 0.0 {
        theme.positive
    } else {
        theme.negative
    };
    let pl_prefix = if pl_abs >= 0.0 { "+" } else { "" };

    let mut total_cells = vec![Cell::from("──────").style(Style::default().fg(theme.muted))];
    if !very_narrow {
        total_cells.push(Cell::from(""));
    }
    total_cells.extend([
        Cell::from("Total").style(Style::default().fg(theme.fg).bold()),
        Cell::from(app.config.fmt_amount(total, 2)).style(Style::default().fg(theme.gold).bold()),
        Cell::from("100%").style(Style::default().fg(theme.fg).bold()),
        Cell::from(format!("{pl_prefix}{:.1}%", pl_pct))
            .style(Style::default().fg(pl_color).bold()),
    ]);
    if !very_narrow {
        total_cells.push(
            Cell::from(format!(
                "{pl_prefix}{}",
                app.config.fmt_amount_compact(pl_abs, 2)
            ))
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
                .style(Style::default().fg(theme.muted).bold())
                .bottom_margin(1),
        )
        .block(
            Block::default()
                .title(Span::styled(
                    " Crypto Portfolio ",
                    Style::default().fg(theme.accent).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.muted)),
        );

    app.geometry.table_area = area;
    f.render_widget(table, area);

    let hint_area = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(2),
        width: area.width.saturating_sub(2),
        height: 1,
    };
    f.render_widget(
        Paragraph::new("  ↑↓ select coin for chart  │  P/L is unrealized only. Sells reflected in Accounts tab.").style(Style::default().fg(theme.muted)),
        hint_area,
    );
}

fn render_allocation_chart(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .title(Span::styled(
            " Allocation ",
            Style::default().fg(theme.accent).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.muted));
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
        let pct = if total > 0.0 {
            h.value_eur / total * 100.0
        } else {
            0.0
        };
        let bar_len = ((pct / 100.0) * bar_area_w as f64) as usize;
        let color = coin_color(&h.commodity, theme);

        let label = Span::styled(
            format!(" {:<w$}", h.commodity, w = (label_w - 1) as usize),
            Style::default().fg(color).bold(),
        );
        f.render_widget(
            Paragraph::new(Line::from(label)),
            Rect {
                x: inner.x,
                y,
                width: label_w,
                height: 1,
            },
        );

        let pct_str = Span::styled(format!("{:>4.1}% ", pct), Style::default().fg(theme.muted));
        f.render_widget(
            Paragraph::new(Line::from(pct_str)),
            Rect {
                x: inner.x + label_w,
                y,
                width: pct_w,
                height: 1,
            },
        );

        let bar = "█".repeat(bar_len.min(bar_area_w as usize));
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(bar, Style::default().fg(color)))),
            Rect {
                x: inner.x + label_w + pct_w,
                y,
                width: bar_area_w,
                height: 1,
            },
        );
    }
}

fn render_price_chart(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let selected_coin = app.selected_coin().unwrap_or("SOL");
    let range_label = app.portfolio_range.label();

    let mode_label = app.chart_mode.label();
    let block = Block::default()
        .title(Span::styled(
            format!(
                " {} Portfolio Analysis [{range_label}] [{mode_label}]  ◀ ▶ ",
                selected_coin
            ),
            Style::default().fg(theme.accent).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.muted));

    let series = match app.coin_chart_cache.get(selected_coin) {
        Some(s) if !s.investment.is_empty() => s,
        _ => {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "No portfolio data available",
                    Style::default().fg(theme.muted),
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
    let first_date = coin_entries
        .first()
        .map(|e| e.date)
        .unwrap_or(chrono::Local::now().date_naive());
    let today = chrono::Local::now().date_naive();
    let today_x = (today - first_date).num_days() as f64;

    let min_x = app.portfolio_range_min_x(first_date);

    let filtered_inv: Vec<(f64, f64)> = series
        .investment
        .iter()
        .filter(|p| p.0 >= min_x)
        .copied()
        .collect();
    let filtered_price: Vec<(f64, f64)> = series
        .price_growth
        .iter()
        .filter(|p| p.0 >= min_x)
        .copied()
        .collect();
    let filtered_staking: Vec<(f64, f64)> = series
        .staking_growth
        .iter()
        .filter(|p| p.0 >= min_x)
        .copied()
        .collect();

    type ChartLines<'a> = (Vec<(f64, f64)>, Vec<(f64, f64)>, &'a str, &'a str);
    let (line2_data, line3_data, line2_name, line3_name): ChartLines<'_> = if app.chart_mode.is_stacked() {
        let purchased: Vec<(f64, f64)> = filtered_inv
            .iter()
            .zip(filtered_price.iter())
            .map(|(inv, pg)| (inv.0, inv.1 + pg.1))
            .collect();
        let total: Vec<(f64, f64)> = filtered_inv
            .iter()
            .zip(filtered_price.iter())
            .zip(filtered_staking.iter())
            .map(|((inv, pg), sg)| (inv.0, inv.1 + pg.1 + sg.1))
            .collect();
        (purchased, total, "Purchased value", "Total value")
    } else {
        (
            filtered_price.clone(),
            filtered_staking.clone(),
            "Price gain",
            "Staking gain",
        )
    };

    let x_min = min_x;
    let x_max = today_x.max(filtered_inv.last().map(|p| p.0).unwrap_or(1.0));

    let all_y = filtered_inv
        .iter()
        .chain(line2_data.iter())
        .chain(line3_data.iter())
        .map(|p| p.1);
    let y_min_raw = all_y.clone().fold(f64::INFINITY, f64::min);
    let y_max_raw = all_y.fold(f64::NEG_INFINITY, f64::max);
    let (y_min, y_max, y_labels) = nice_y_axis(y_min_raw, y_max_raw, 4, &app.config, theme.muted);

    let filtered_entries: Vec<_> = coin_entries
        .iter()
        .filter(|e| {
            let day = (e.date - first_date).num_days() as f64;
            day >= min_x
        })
        .collect();
    let x_labels: Vec<Span> = {
        let n = filtered_entries.len();
        let count = if area.width < 60 { 3 } else { 5 };
        let indices: Vec<usize> = (0..count)
            .map(|i| i * n.saturating_sub(1) / (count - 1).max(1))
            .collect();
        indices
            .iter()
            .filter_map(|&i| filtered_entries.get(i))
            .map(|e| {
                Span::styled(
                    e.date.format("%b %y").to_string(),
                    Style::default().fg(theme.muted),
                )
            })
            .collect()
    };

    let ds_investment = Dataset::default()
        .name("Invested")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(theme.gold))
        .data(&filtered_inv);

    let ds_line2 = Dataset::default()
        .name(line2_name)
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(theme.accent))
        .data(&line2_data);

    let ds_line3 = Dataset::default()
        .name(line3_name)
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(theme.positive))
        .data(&line3_data);

    let chart = Chart::new(vec![ds_investment, ds_line2, ds_line3])
        .style(Style::default().bg(theme.background))
        .block(block)
        .x_axis(
            Axis::default()
                .style(Style::default().fg(theme.muted))
                .bounds([x_min, x_max])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(theme.muted))
                .bounds([y_min, y_max])
                .labels(y_labels),
        );

    f.render_widget(chart, area);
}
