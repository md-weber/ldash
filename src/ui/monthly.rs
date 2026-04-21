use chrono::Datelike;
use ratatui::{prelude::*, widgets::*};

use super::{expense_color, render_detail_with_title, Theme};
use crate::app::{budget_matches, budget_spent, App, MonthlyFocus};

pub(super) fn render_monthly(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let narrow = area.width < 100;
    let very_narrow = area.width < 80;

    let chunks = Layout::vertical([
        Constraint::Length(12),
        Constraint::Length(if narrow { 20 } else { 14 }),
        Constraint::Min(0),
    ])
    .split(area);

    app.monthly_chart_area = chunks[0];
    render_monthly_chart(f, app, chunks[0], theme);
    render_monthly_summary(f, app, chunks[1], theme);

    let detail_chunks = if very_narrow {
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).split(chunks[2])
    } else {
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2])
    };

    if let (Some(txns), Some(name)) = (&app.income_detail.clone(), &app.detail_income_name.clone())
    {
        let short = name.strip_prefix("income:").unwrap_or(name);
        let month = app
            .current_month()
            .map(|m| m.month_name.as_str())
            .unwrap_or("");
        let title = format!(" {} — {}  [Esc back] ", short, month);
        render_detail_with_title(
            f,
            txns,
            &title,
            &app.config,
            detail_chunks[0],
            theme,
            &mut app.detail_state,
        );
    } else {
        render_monthly_income(f, app, detail_chunks[0], theme);
    }

    if let (Some(txns), Some(name)) = (
        &app.expense_detail.clone(),
        &app.detail_expense_name.clone(),
    ) {
        let short = name.strip_prefix("expenses:").unwrap_or(name);
        let month = app
            .current_month()
            .map(|m| m.month_name.as_str())
            .unwrap_or("");
        let title = format!(" {} — {}  [Esc back] ", short, month);
        render_detail_with_title(
            f,
            txns,
            &title,
            &app.config,
            detail_chunks[1],
            theme,
            &mut app.detail_state,
        );
    } else {
        render_monthly_expenses(f, app, detail_chunks[1], theme);
    }
}

fn render_monthly_chart(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
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
                (theme.positive, theme.negative)
            };

            let label = &m.month_name[..3];
            BarGroup::default()
                .label(
                    Line::from(label.to_string()).style(Style::default().fg(if selected {
                        theme.accent
                    } else {
                        theme.muted
                    })),
                )
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
        .style(Style::default().bg(theme.background))
        .block(
            Block::default()
                .title(Span::styled(
                    chart_title,
                    Style::default().fg(theme.accent).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.muted))
                .padding(Padding::new(1, 1, 1, 0)),
        )
        .bar_width(3)
        .bar_gap(0)
        .group_gap(2)
        .bar_style(Style::default().fg(theme.positive));

    for g in groups {
        chart = chart.data(g);
    }

    f.render_widget(chart, area);
}

fn render_monthly_summary(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let empty = crate::data::SingleMonth::default();
    let m = app.current_month().unwrap_or(&empty);
    let net = m.total_income - m.total_expenses;
    let net_color = if net >= 0.0 {
        theme.positive
    } else {
        theme.negative
    };
    let net_prefix = if net >= 0.0 { "+" } else { "" };

    let savings_rate = if m.total_income > 0.0 {
        (net / m.total_income * 100.0).clamp(0.0, 100.0) as u16
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
            Span::styled("  Income    ", Style::default().fg(theme.muted)),
            Span::styled(
                app.config.fmt_amount(m.total_income, 2),
                Style::default().fg(theme.positive).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Expenses  ", Style::default().fg(theme.muted)),
            Span::styled(
                app.config.fmt_amount(m.total_expenses, 2),
                Style::default().fg(theme.negative).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Net       ", Style::default().fg(theme.muted)),
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
                Span::styled("  vs ", Style::default().fg(theme.muted)),
                Span::styled(
                    format!("{short} '{}", year_ago % 100),
                    Style::default().fg(theme.muted),
                ),
                Span::styled("  (partial data)", Style::default().fg(theme.muted)),
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
                ("↓", theme.positive)
            } else {
                ("↑", theme.negative)
            };
            text.push(Line::from(vec![
                Span::styled("  vs ", Style::default().fg(theme.muted)),
                Span::styled(
                    format!(
                        "{short} '{}: {}",
                        year_ago % 100,
                        app.config.fmt_amount_compact(ly_net, 0)
                    ),
                    Style::default().fg(theme.muted),
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
                Span::styled("  ✓ ", Style::default().fg(theme.positive)),
                Span::styled("All budgets on track", Style::default().fg(theme.positive)),
            ]));
        } else {
            text.push(Line::from(vec![
                Span::styled("  ⚠ ", Style::default().fg(theme.negative)),
                Span::styled(
                    format!("{} over budget:", over.len()),
                    Style::default().fg(theme.negative).bold(),
                ),
            ]));
            for b in &over {
                let short = b.category.strip_prefix("expenses:").unwrap_or(&b.category);
                text.push(Line::from(vec![
                    Span::styled(format!("    {short}: "), Style::default().fg(theme.fg)),
                    Span::styled(
                        format!(
                            "{}/{} ({:.0}%)",
                            app.config.fmt_amount_compact(b.spent, 0),
                            app.config.fmt_amount_compact(b.limit, 0),
                            b.pct
                        ),
                        Style::default().fg(theme.negative),
                    ),
                ]));
            }
        }
    }

    let left_block = Block::default()
        .title(Span::styled(
            nav_title,
            Style::default().fg(theme.accent).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.muted));

    f.render_widget(Paragraph::new(text).block(left_block), chunks[0]);

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(Span::styled(
                    " Savings Rate ",
                    Style::default().fg(theme.accent).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.muted)),
        )
        .gauge_style(Style::default().fg(net_color).bg(theme.background))
        .percent(savings_rate)
        .label(Span::styled(
            format!("{savings_rate}% saved"),
            Style::default().fg(theme.fg).bold(),
        ));

    f.render_widget(gauge, chunks[1]);

    let ytd = app.ytd_stats();
    let ytd_net = ytd.total_income - ytd.total_expenses;
    let ytd_net_color = if ytd_net >= 0.0 {
        theme.positive
    } else {
        theme.negative
    };
    let ytd_prefix = if ytd_net >= 0.0 { "+" } else { "" };

    let ytd_block = Block::default()
        .title(Span::styled(
            " Year to Date ",
            Style::default().fg(theme.gold).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.muted));
    let ytd_area = ytd_block.inner(chunks[2]);
    f.render_widget(ytd_block, chunks[2]);

    let ytd_split = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(ytd_area);

    let ytd_text = vec![
        Line::from(vec![
            Span::styled("  Net YTD   ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{ytd_prefix}{}", app.config.fmt_amount(ytd_net, 0)),
                Style::default().fg(ytd_net_color).bold(),
            ),
            Span::styled(
                format!("  ({:.0}% saved)", ytd.avg_savings_rate),
                Style::default().fg(theme.muted),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Best      ", Style::default().fg(theme.muted)),
            Span::styled(
                format!(
                    "{} (+{})",
                    ytd.best_month.get(..3).unwrap_or(&ytd.best_month),
                    app.config.fmt_amount(ytd.best_net, 0)
                ),
                Style::default().fg(theme.positive),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Worst     ", Style::default().fg(theme.muted)),
            Span::styled(
                format!(
                    "{} ({})",
                    ytd.worst_month.get(..3).unwrap_or(&ytd.worst_month),
                    app.config.fmt_amount(ytd.worst_net, 0)
                ),
                Style::default().fg(theme.negative),
            ),
        ]),
    ];
    f.render_widget(Paragraph::new(ytd_text), ytd_split[0]);

    let spark_months = &app.monthly.months[app.monthly.months.len().saturating_sub(6)..];
    let inc_data: Vec<u64> = spark_months.iter().map(|m| m.total_income as u64).collect();
    let exp_data: Vec<u64> = spark_months
        .iter()
        .map(|m| m.total_expenses as u64)
        .collect();
    let net_data: Vec<u64> = spark_months
        .iter()
        .map(|m| (m.total_income - m.total_expenses).max(0.0) as u64)
        .collect();

    let spark_rows = Layout::vertical([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(ytd_split[1]);

    let spark_inc = Sparkline::default()
        .block(
            Block::default()
                .title(Span::styled(
                    " Income ▁▃▅ ",
                    Style::default().fg(theme.positive),
                ))
                .borders(Borders::TOP)
                .border_style(Style::default().fg(theme.muted)),
        )
        .data(&inc_data)
        .style(Style::default().fg(theme.positive).bg(theme.background));
    f.render_widget(spark_inc, spark_rows[0]);

    let spark_exp = Sparkline::default()
        .block(
            Block::default()
                .title(Span::styled(
                    " Expenses ▁▃▅ ",
                    Style::default().fg(theme.negative),
                ))
                .borders(Borders::TOP)
                .border_style(Style::default().fg(theme.muted)),
        )
        .data(&exp_data)
        .style(Style::default().fg(theme.negative).bg(theme.background));
    f.render_widget(spark_exp, spark_rows[1]);

    let spark_net = Sparkline::default()
        .block(
            Block::default()
                .title(Span::styled(" Net ▁▃▅ ", Style::default().fg(theme.gold)))
                .borders(Borders::TOP)
                .border_style(Style::default().fg(theme.muted)),
        )
        .data(&net_data)
        .style(Style::default().fg(theme.gold).bg(theme.background));
    f.render_widget(spark_net, spark_rows[2]);
}

fn render_monthly_income(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let narrow = area.width < 100;
    let focused = app.monthly_focus == MonthlyFocus::Income;
    let empty = crate::data::SingleMonth::default();
    let m = app.current_month().unwrap_or(&empty);
    let max_val = m.income.first().map(|i| i.1).unwrap_or(1.0);
    let bar_width = if narrow {
        0
    } else {
        area.width.saturating_sub(40) as usize
    };

    let rows: Vec<Row> = m
        .income
        .iter()
        .map(|(name, amount)| {
            let short = name.strip_prefix("income:").unwrap_or(name);
            let mut cells = vec![
                Cell::from(short.to_string()).style(Style::default().fg(theme.fg)),
                Cell::from(app.config.fmt_amount(*amount, 2))
                    .style(Style::default().fg(theme.positive)),
            ];
            if !narrow {
                let bar_len = ((amount / max_val) * bar_width as f64) as usize;
                let bar = "█".repeat(bar_len.min(bar_width));
                cells.push(Cell::from(bar).style(Style::default().fg(theme.positive)));
            }
            Row::new(cells)
        })
        .collect();

    let widths: Vec<Constraint> = if narrow {
        vec![Constraint::Min(20), Constraint::Length(12)]
    } else {
        vec![
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Min(4),
        ]
    };

    let mut header = vec!["Source", "Amount"];
    if !narrow {
        header.push("");
    }

    let title = if app.income_detail.is_none() {
        " Income  [i] focus  [Enter] detail ".to_string()
    } else {
        " Income ".to_string()
    };

    let border_color = if focused { theme.accent } else { theme.muted };

    let table = Table::new(rows, widths)
        .header(
            Row::new(header)
                .style(Style::default().fg(theme.muted).bold())
                .bottom_margin(1),
        )
        .row_highlight_style(Style::default().bg(theme.highlight_bg).bold())
        .highlight_symbol("▶ ")
        .block(
            Block::default()
                .title(Span::styled(
                    title,
                    Style::default().fg(theme.positive).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color)),
        );

    app.income_table_area = area;
    f.render_stateful_widget(table, area, &mut app.income_state);
}

fn render_monthly_expenses(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let narrow = area.width < 100;
    let focused = app.monthly_focus == MonthlyFocus::Expenses;
    let has_budgets = !app.config.budgets.is_empty();
    let empty = crate::data::SingleMonth::default();
    let m = app.current_month().unwrap_or(&empty);
    let max_val = m.expenses.first().map(|e| e.1).unwrap_or(1.0);
    let bar_width = if narrow {
        0
    } else {
        area.width.saturating_sub(if has_budgets { 56 } else { 40 }) as usize
    };

    let rows: Vec<Row> = m
        .expenses
        .iter()
        .map(|(name, amount)| {
            let short = name.strip_prefix("expenses:").unwrap_or(name);
            let color = if app.expense_colors {
                expense_color(short, app, theme)
            } else {
                theme.fg
            };
            let mut cells = vec![
                Cell::from(short.to_string()).style(Style::default().fg(color)),
                Cell::from(app.config.fmt_amount(*amount, 2))
                    .style(Style::default().fg(theme.negative)),
            ];
            if !narrow {
                let bar_len = ((amount / max_val) * bar_width as f64) as usize;
                let bar = "█".repeat(bar_len.min(bar_width));
                cells.push(
                    Cell::from(bar).style(Style::default().fg(if app.expense_colors {
                        color
                    } else {
                        theme.negative
                    })),
                );
            }
            if has_budgets {
                let matched = app
                    .config
                    .budgets
                    .iter()
                    .find(|(cat, _)| budget_matches(cat, name));
                if let Some((cat, &limit)) = matched {
                    let spent = budget_spent(cat, &m.expenses);
                    let pct = if limit > 0.0 {
                        spent / limit * 100.0
                    } else {
                        0.0
                    };
                    let bar_color = if pct < 80.0 {
                        theme.positive
                    } else if pct <= 100.0 {
                        theme.gold
                    } else {
                        theme.negative
                    };
                    let filled = ((pct / 100.0).min(1.0) * 8.0) as usize;
                    let empty_b = 8 - filled;
                    let bar = format!(
                        "[{}{}] {:>3.0}%",
                        "█".repeat(filled),
                        "░".repeat(empty_b),
                        pct
                    );
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
        if has_budgets {
            w.push(Constraint::Length(16));
        }
        w
    } else {
        let mut w = vec![
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Min(4),
        ];
        if has_budgets {
            w.push(Constraint::Length(16));
        }
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
                .style(Style::default().fg(theme.muted).bold())
                .bottom_margin(1),
        )
        .row_highlight_style(Style::default().bg(theme.highlight_bg).bold())
        .highlight_symbol("▶ ")
        .block(
            Block::default()
                .title(Span::styled(
                    if has_budgets {
                        format!(" Expenses ({} budgets) ", app.config.budgets.len())
                    } else {
                        " Expenses ".to_string()
                    },
                    Style::default().fg(theme.negative).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if focused {
                    theme.accent
                } else {
                    theme.muted
                })),
        );

    app.expense_table_area = area;
    f.render_stateful_widget(table, area, &mut app.expense_state);
}
