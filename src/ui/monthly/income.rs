use ratatui::{prelude::*, widgets::*};

use crate::app::{App, MonthlyFocus};
use crate::ui::{income_color, Theme};

pub(super) fn render_monthly_income(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
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
            let short = app
                .config
                .strip_account_prefix(name, &app.config.income_account.clone());
            let override_color = income_color(short, app, theme);
            let name_color = override_color.unwrap_or(theme.fg);
            let amt_color = override_color.unwrap_or(theme.positive);
            let mut cells = vec![
                Cell::from(short.to_string()).style(Style::default().fg(name_color)),
                Cell::from(app.config.fmt_amount(*amount, 2)).style(Style::default().fg(amt_color)),
            ];
            if !narrow {
                let bar_len = ((amount / max_val) * bar_width as f64) as usize;
                let bar = "█".repeat(bar_len.min(bar_width));
                cells.push(Cell::from(bar).style(Style::default().fg(amt_color)));
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
        if app.current_month_is_forecast() {
            " Income  ~ forecast ".to_string()
        } else {
            " Income  [i] focus  [Enter] detail ".to_string()
        }
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

    app.geometry.income_table_area = area;
    f.render_stateful_widget(table, area, &mut app.income_state);
}
