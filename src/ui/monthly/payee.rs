use ratatui::{prelude::*, widgets::*};

use super::sparkline::sparkline_str;
use crate::app::App;
use crate::ui::Theme;

pub(super) fn render_payee_analytics(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    if app.payee_data.is_empty() {
        let msg = if app.loading {
            "Loading payee analytics…"
        } else {
            "No payee data for this year"
        };
        let block = Block::default()
            .title(Span::styled(
                " Payee Analytics  [p] close ",
                Style::default().fg(theme.accent).bold(),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.muted));
        let inner = block.inner(area);
        f.render_widget(block, area);
        f.render_widget(
            Paragraph::new(msg)
                .style(Style::default().fg(theme.muted))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let rows: Vec<Row> = app
        .payee_data
        .iter()
        .map(|p| {
            let spark = sparkline_str(&p.sparkline);
            Row::new(vec![
                Cell::from(p.name.clone()).style(Style::default().fg(theme.fg)),
                Cell::from(format!("{:>14}", app.config.fmt_amount(p.total, 0)))
                    .style(Style::default().fg(theme.negative)),
                Cell::from(format!("{:>12}", app.config.fmt_amount(p.monthly_avg, 0)))
                    .style(Style::default().fg(theme.muted)),
                Cell::from(spark).style(Style::default().fg(theme.accent)),
            ])
        })
        .collect();

    let title = format!(
        " Payee Analytics — {} payees  [p] close ",
        app.payee_data.len()
    );

    let table = Table::new(
        rows,
        [
            Constraint::Min(20),
            Constraint::Length(15),
            Constraint::Length(13),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["Payee", "YTD Total", "Avg/mo", "Last 6M"])
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

    app.geometry.payee_table_area = area;
    f.render_stateful_widget(table, area, &mut app.payee_state);
}
