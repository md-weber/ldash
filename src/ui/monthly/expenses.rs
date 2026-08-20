use ratatui::{prelude::*, widgets::*};

use super::sparkline::{category_expense_history, sparkline_str};
use crate::app::{App, MonthlyFocus};
use crate::ui::{expense_color, Theme};

pub(super) fn render_monthly_expenses(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let narrow = area.width < 100;
    let focused = app.monthly_focus == MonthlyFocus::Expenses;
    let has_budgets = !app.config.budgets.is_empty();
    let empty = crate::data::SingleMonth::default();
    let m = app.current_month().unwrap_or(&empty);
    let max_val = m.expenses.first().map(|e| e.1).unwrap_or(1.0);
    let bar_width = if narrow {
        0
    } else {
        area.width.saturating_sub(if has_budgets { 64 } else { 48 }) as usize
    };

    let rows: Vec<Row> = m
        .expenses
        .iter()
        .map(|(name, amount)| {
            let short = app
                .config
                .strip_account_prefix(name, &app.config.expenses_account.clone());
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
                let history = category_expense_history(&app.monthly.months, &m.month_name, name);
                cells.push(
                    Cell::from(sparkline_str(&history)).style(Style::default().fg(theme.accent)),
                );
            }
            if has_budgets {
                let matched = app
                    .config
                    .budgets
                    .iter()
                    .find(|(cat, _)| app.config.budget_matches(cat, name));
                if let Some((cat, &limit)) = matched {
                    let spent = app.config.budget_spent(cat, &m.expenses);
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
            Constraint::Length(8),
        ];
        if has_budgets {
            w.push(Constraint::Length(16));
        }
        w
    };

    let mut header = vec!["Category", "Amount"];
    if !narrow {
        header.push("");
        header.push("Trend");
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
                    if app.current_month_is_forecast() {
                        " Expenses  ~ forecast ".to_string()
                    } else if has_budgets {
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

    app.geometry.expense_table_area = area;
    f.render_stateful_widget(table, area, &mut app.expense_state);
}
