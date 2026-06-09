use ratatui::{prelude::*, widgets::*};

use crate::app::App;
use crate::ui::Theme;

/// Render a single Unicode block-character sparkline from `values`.
/// Scales relative to the maximum value in the slice.
fn sparkline_str(values: &[f64]) -> String {
    if values.is_empty() {
        return String::new();
    }
    let max = values.iter().cloned().fold(0.0f64, f64::max);
    if max <= 0.0 {
        return "▁".repeat(values.len());
    }
    const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    values
        .iter()
        .map(|&v| BLOCKS[((v / max * 7.0) as usize).min(7)])
        .collect()
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_str_empty() {
        assert_eq!(sparkline_str(&[]), "");
    }

    #[test]
    fn sparkline_str_all_zero() {
        let s = sparkline_str(&[0.0, 0.0, 0.0]);
        assert_eq!(s, "▁▁▁");
    }

    #[test]
    fn sparkline_str_ascending() {
        let s = sparkline_str(&[0.0, 50.0, 100.0]);
        let chars: Vec<char> = s.chars().collect();
        // Ascending: first ≤ second ≤ third
        assert!(chars[0] <= chars[1], "should be non-decreasing");
        assert!(chars[1] <= chars[2], "should be non-decreasing");
        // Max value should be full block
        assert_eq!(chars[2], '█');
    }

    #[test]
    fn sparkline_str_single_max() {
        let s = sparkline_str(&[100.0]);
        assert_eq!(s, "█");
    }
}
