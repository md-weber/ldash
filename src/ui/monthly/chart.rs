use ratatui::style::Modifier;
use ratatui::{prelude::*, widgets::*};

use super::{
    MONTHLY_BAR_GAP, MONTHLY_BAR_WIDTH, MONTHLY_GROUP_GAP,
};
use crate::app::App;
use crate::ui::Theme;

pub(super) fn render_monthly_chart(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    use ratatui::widgets::{Bar, BarChart, BarGroup};

    let groups: Vec<BarGroup> = app
        .combined_months
        .iter()
        .enumerate()
        .map(|(i, (m, is_forecast))| {
            let selected = i == app.combined_selected;
            let (inc_color, exp_color) = if selected {
                (Color::LightGreen, Color::LightRed)
            } else {
                (theme.positive, theme.negative)
            };

            let label_color = if selected {
                theme.accent
            } else if *is_forecast {
                theme.gold
            } else {
                theme.muted
            };

            // Prefix forecast month labels with "~" to signal they're projected.
            let label = if *is_forecast {
                format!("~{}", &m.month_name[..3])
            } else {
                m.month_name[..3].to_string()
            };

            // Dim the bars of unselected forecast months so actuals stand out.
            let inc_style = if *is_forecast && !selected {
                Style::default().fg(inc_color).add_modifier(Modifier::DIM)
            } else {
                Style::default().fg(inc_color)
            };
            let exp_style = if *is_forecast && !selected {
                Style::default().fg(exp_color).add_modifier(Modifier::DIM)
            } else {
                Style::default().fg(exp_color)
            };

            BarGroup::default()
                .label(Line::from(label).style(Style::default().fg(label_color)))
                .bars(&[
                    Bar::default().value(m.total_income as u64).style(inc_style),
                    Bar::default()
                        .value(m.total_expenses as u64)
                        .style(exp_style),
                ])
        })
        .collect();

    let yoy_hint = if app.yoy_view { "[C hide yoy]" } else { "[C yoy]" };
    let chart_title = if app.monthly_year_offset == 0 {
        format!(" Income vs Expenses  {yoy_hint}  [G last entry] ")
    } else {
        format!(
            " Income vs Expenses ({})  [y/Y]  {yoy_hint}  [G last entry] ",
            app.displayed_year()
        )
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
        .bar_width(MONTHLY_BAR_WIDTH)
        .bar_gap(MONTHLY_BAR_GAP)
        .group_gap(MONTHLY_GROUP_GAP)
        .bar_style(Style::default().fg(theme.positive));

    for g in groups {
        chart = chart.data(g);
    }

    f.render_widget(chart, area);
}
