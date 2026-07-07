use chrono::Datelike;
use ratatui::{prelude::*, widgets::*};

use crate::app::{App, RecurringExpense};
use crate::ui::Theme;

pub(super) fn render_monthly_summary(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
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

    let is_forecast = app.current_month_is_forecast();
    let forecast_tag = if is_forecast { "  ~ forecast" } else { "" };
    let nav_title = if app.monthly_year_offset != 0 {
        format!(
            " ◀ {} {} ▶  [y/Y]{} ",
            m.month_name,
            app.displayed_year(),
            forecast_tag
        )
    } else {
        format!(" ◀ {} ▶{} ", m.month_name, forecast_tag)
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

    if let Some(liquid) = app.liquid_cash_change_for_selected_month() {
        let liquid_color = if liquid >= 0.0 {
            theme.positive
        } else {
            theme.negative
        };
        let liquid_prefix = if liquid >= 0.0 { "+" } else { "" };
        text.push(Line::from(vec![
            Span::styled("  Liquid Chg", Style::default().fg(theme.muted)),
            Span::styled(
                format!(" {liquid_prefix}{}", app.config.fmt_amount(liquid, 2)),
                Style::default().fg(liquid_color).bold(),
            ),
        ]));
    }

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
                let short = app
                    .config
                    .strip_account_prefix(&b.category, &app.config.expenses_account.clone());
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

    let recurring = app.recurring_expenses();
    if !recurring.is_empty() {
        let total: f64 = recurring.iter().map(|r| r.monthly_avg).sum();
        let max_names = 3usize;
        let names = recurring_display_names(&recurring, max_names);
        let names_str = names.join(", ");
        let suffix = if recurring.len() > max_names {
            format!(" +{} more", recurring.len() - max_names)
        } else {
            String::new()
        };
        text.push(Line::from(""));
        text.push(Line::from(vec![
            Span::styled("  ↻ ", Style::default().fg(theme.gold)),
            Span::styled(
                format!("{} recurring: ", recurring.len()),
                Style::default().fg(theme.gold).bold(),
            ),
            Span::styled(
                format!("{names_str}{suffix}"),
                Style::default().fg(theme.fg),
            ),
            Span::styled(
                format!(" = {}/mo", app.config.fmt_amount_compact(total, 0)),
                Style::default().fg(theme.gold).bold(),
            ),
        ]));
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

/// Build short display labels for the recurring expenses summary line.
/// Always uses the leaf segment. Duplicate leaves are dropped so the same
/// word never appears twice; the caller's count reflects the true total.
fn recurring_display_names(recurring: &[RecurringExpense], max: usize) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(max);
    for r in recurring {
        if out.len() >= max {
            break;
        }
        let leaf = r.name.rsplit(':').next().unwrap_or(&r.name).to_string();
        if seen.insert(leaf.clone()) {
            out.push(leaf);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(name: &str, avg: f64) -> RecurringExpense {
        RecurringExpense {
            name: name.to_string(),
            monthly_avg: avg,
        }
    }

    #[test]
    fn display_names_uses_leaf() {
        let r = vec![
            rec("expenses:wohnen:miete", 800.0),
            rec("expenses:abos:netflix", 10.0),
            rec("expenses:abos:lilly", 5.0),
        ];
        assert_eq!(
            recurring_display_names(&r, 3),
            vec!["miete", "netflix", "lilly"]
        );
    }

    #[test]
    fn display_names_deduplicates_same_leaf() {
        // "miete" must not appear twice even if two accounts share that leaf
        let r = vec![
            rec("expenses:wohnen:miete", 800.0),
            rec("expenses:auto:miete", 300.0),
            rec("expenses:abos:lilly", 5.0),
        ];
        let names = recurring_display_names(&r, 3);
        assert_eq!(names, vec!["miete", "lilly"]);
    }

    #[test]
    fn display_names_respects_max() {
        let r = vec![
            rec("expenses:a", 3.0),
            rec("expenses:b", 2.0),
            rec("expenses:c", 1.0),
        ];
        assert_eq!(recurring_display_names(&r, 2), vec!["a", "b"]);
    }

    #[test]
    fn display_names_max_applies_after_dedup() {
        // max=2 with a collision: first unique leaf + skip dup + next unique
        let r = vec![
            rec("expenses:wohnen:miete", 800.0),
            rec("expenses:auto:miete", 300.0),
            rec("expenses:abos:lilly", 5.0),
        ];
        // after dedup: ["miete", "lilly"] → max=2 fits exactly
        assert_eq!(recurring_display_names(&r, 2), vec!["miete", "lilly"]);
    }
}
