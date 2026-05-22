use ratatui::{prelude::*, widgets::*};

use crate::app::App;
use crate::ui::Theme;

pub(super) fn render_forecast_chart(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    use ratatui::symbols;
    use ratatui::widgets::{Axis, Chart, Dataset, GraphType};

    if app.monthly.months.is_empty() {
        return;
    }

    let forecast = app.cash_flow_forecast();
    if forecast.actuals.is_empty() {
        return;
    }

    let has_projection = forecast.projected.len() > 1;

    let mut datasets = vec![Dataset::default()
        .name("Actual")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(theme.positive))
        .data(&forecast.actuals)];

    if has_projection {
        datasets.push(
            Dataset::default()
                .name("Forecast")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(theme.gold))
                .data(&forecast.projected),
        );
    }

    let fmt_k = |v: f64| -> Span {
        let s = if v.abs() >= 1000.0 {
            format!("{:.0}k", v / 1000.0)
        } else {
            format!("{:.0}", v)
        };
        Span::styled(s, Style::default().fg(theme.muted))
    };

    let title = if has_projection {
        format!(
            " Forecast ~{}/mo ",
            app.config
                .fmt_amount_compact(forecast.projected_monthly_net, 0)
        )
    } else {
        " Cash Flow ".to_string()
    };

    let mid_y = (forecast.min_y + forecast.max_y) / 2.0;

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title(Span::styled(title, Style::default().fg(theme.gold).bold()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.muted)),
        )
        .x_axis(
            Axis::default()
                .bounds([0.0, 11.0])
                .labels(vec![
                    Span::styled("Jan", Style::default().fg(theme.muted)),
                    Span::styled("Apr", Style::default().fg(theme.muted)),
                    Span::styled("Jul", Style::default().fg(theme.muted)),
                    Span::styled("Oct", Style::default().fg(theme.muted)),
                    Span::styled("Dec", Style::default().fg(theme.muted)),
                ])
                .style(Style::default().fg(theme.muted)),
        )
        .y_axis(
            Axis::default()
                .bounds([forecast.min_y, forecast.max_y])
                .labels(vec![
                    fmt_k(forecast.min_y),
                    fmt_k(mid_y),
                    fmt_k(forecast.max_y),
                ])
                .style(Style::default().fg(theme.muted)),
        );

    f.render_widget(chart, area);
}
