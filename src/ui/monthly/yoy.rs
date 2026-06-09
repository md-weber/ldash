use chrono::Datelike;
use ratatui::{prelude::*, style::Modifier, widgets::*};

use crate::app::App;
use crate::data::SingleMonth;
use crate::ui::Theme;

pub(super) fn render_yoy_comparison(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    if area.height < 4 {
        return;
    }

    let this_year = chrono::Local::now().year();
    let last_year = this_year - 1;

    // 1-line legend at the top, remainder for the two charts.
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(area);

    render_legend(f, app, rows[0], theme, last_year, this_year);

    if rows[1].height < 3 {
        return;
    }

    let halves =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(rows[1]);

    render_half(f, app, halves[0], theme, Half::Income, last_year);
    render_half(f, app, halves[1], theme, Half::Expenses, last_year);
}

// ── Legend ────────────────────────────────────────────────────────────────────

fn render_legend(
    f: &mut Frame,
    app: &App,
    area: Rect,
    theme: &Theme,
    last_year: i32,
    this_year: i32,
) {
    let yr_short = last_year % 100;
    let mut spans = vec![
        Span::raw("  "),
        Span::styled("█", Style::default().fg(theme.muted)),
        Span::styled(
            format!(" '{yr_short} (dim)   "),
            Style::default().fg(theme.muted),
        ),
        Span::styled("█", Style::default().fg(theme.fg)),
        Span::styled(
            format!(" {this_year} (bright)"),
            Style::default().fg(theme.fg),
        ),
    ];

    if let (Some(cur), Some(ly)) = (app.current_month(), app.last_year_match()) {
        let short = &cur.month_name[..3];

        let inc_diff = cur.total_income - ly.total_income;
        let exp_diff = cur.total_expenses - ly.total_expenses;
        let inc_pct = pct_change(ly.total_income, inc_diff);
        let exp_pct = pct_change(ly.total_expenses, exp_diff);

        let (inc_arrow, inc_color) = signed_indicator(inc_diff, true, theme);
        let (exp_arrow, exp_color) = signed_indicator(exp_diff, false, theme);

        spans.push(Span::styled(
            format!("     {short} vs '{yr_short}:  "),
            Style::default().fg(theme.muted),
        ));
        spans.push(Span::styled("Inc ", Style::default().fg(theme.muted)));
        spans.push(Span::styled(
            format!("{inc_arrow}{:.0}%", inc_pct.abs()),
            Style::default().fg(inc_color).bold(),
        ));
        spans.push(Span::styled("   Exp ", Style::default().fg(theme.muted)));
        spans.push(Span::styled(
            format!("{exp_arrow}{:.0}%", exp_pct.abs()),
            Style::default().fg(exp_color).bold(),
        ));
        spans.push(Span::styled(
            "     [C hide]",
            Style::default().fg(theme.muted),
        ));
    } else {
        spans.push(Span::styled(
            "     [C hide]",
            Style::default().fg(theme.muted),
        ));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ── Half chart ────────────────────────────────────────────────────────────────

enum Half {
    Income,
    Expenses,
}

fn render_half(f: &mut Frame, app: &App, area: Rect, theme: &Theme, half: Half, last_year: i32) {
    use ratatui::widgets::{Bar, BarChart, BarGroup};

    let (base_color, get_val): (Color, fn(&SingleMonth) -> f64) = match half {
        Half::Income => (theme.positive, |m| m.total_income),
        Half::Expenses => (theme.negative, |m| m.total_expenses),
    };

    let title = build_half_title(app, theme, &half, last_year, base_color);

    // Last-year bars use `theme.muted` (DarkGray in the default theme) so
    // they are clearly distinct from the colored this-year bars on every
    // theme and terminal — no DIM modifier needed.
    let prev_style = Style::default().fg(theme.muted);
    let curr_style = Style::default().fg(base_color);
    // For the selected month, bold both bars so the active comparison pops.
    let prev_sel = Style::default()
        .fg(theme.muted)
        .add_modifier(Modifier::BOLD);
    let curr_sel = Style::default().fg(base_color).add_modifier(Modifier::BOLD);

    let groups: Vec<BarGroup> = app
        .combined_months
        .iter()
        .enumerate()
        .filter_map(|(i, (m, _))| {
            let ly = app
                .last_year
                .months
                .iter()
                .find(|ly| ly.month_name == m.month_name)?;

            let selected = i == app.combined_selected;
            let label_color = if selected { theme.accent } else { theme.muted };
            let (ly_style, ty_style) = if selected {
                (prev_sel, curr_sel)
            } else {
                (prev_style, curr_style)
            };

            Some(
                BarGroup::default()
                    .label(
                        Line::from(m.month_name[..3].to_string())
                            .style(Style::default().fg(label_color)),
                    )
                    .bars(&[
                        Bar::default().value(get_val(ly) as u64).style(ly_style),
                        Bar::default().value(get_val(m) as u64).style(ty_style),
                    ]),
            )
        })
        .collect();

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.muted))
        .padding(Padding::new(1, 1, 1, 0));

    if groups.is_empty() {
        f.render_widget(
            Paragraph::new("No last-year data")
                .block(block)
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let bar_width = if area.width >= 90 { 3u16 } else { 2 };

    let mut chart = BarChart::default()
        .style(Style::default().bg(theme.background))
        .block(block)
        .bar_width(bar_width)
        .bar_gap(0)
        .group_gap(2)
        .bar_style(Style::default().fg(base_color));

    for g in groups {
        chart = chart.data(g);
    }

    f.render_widget(chart, area);
}

fn build_half_title(
    app: &App,
    theme: &Theme,
    half: &Half,
    last_year: i32,
    base_color: Color,
) -> Line<'static> {
    let label = match half {
        Half::Income => "Income",
        Half::Expenses => "Expenses",
    };

    let base = Span::styled(format!(" {label} "), Style::default().fg(base_color).bold());

    match (app.current_month(), app.last_year_match()) {
        (Some(cur), Some(ly)) => {
            let yr_short = last_year % 100;
            let short = cur.month_name[..3].to_string();
            let (cur_val, ly_val) = match half {
                Half::Income => (cur.total_income, ly.total_income),
                Half::Expenses => (cur.total_expenses, ly.total_expenses),
            };
            let diff = cur_val - ly_val;
            let pct = pct_change(ly_val, diff);
            let higher_is_good = matches!(half, Half::Income);
            let (arrow, color) = signed_indicator(diff, higher_is_good, theme);

            Line::from(vec![
                base,
                Span::styled(
                    format!("{short} vs '{yr_short}  "),
                    Style::default().fg(theme.muted),
                ),
                Span::styled(
                    format!("{arrow}{:.0}% ", pct.abs()),
                    Style::default().fg(color).bold(),
                ),
            ])
        }
        _ => Line::from(vec![base]),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn pct_change(base: f64, diff: f64) -> f64 {
    if base > 0.0 {
        diff / base * 100.0
    } else {
        0.0
    }
}

/// Returns (arrow symbol, color). `higher_is_good = true` for income (up = green),
/// `false` for expenses (up = red).
fn signed_indicator(diff: f64, higher_is_good: bool, theme: &Theme) -> (&'static str, Color) {
    let positive_outcome = if higher_is_good {
        diff >= 0.0
    } else {
        diff <= 0.0
    };
    let arrow = if diff >= 0.0 { "↑" } else { "↓" };
    let color = if positive_outcome {
        theme.positive
    } else {
        theme.negative
    };
    (arrow, color)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::data::{MonthlyData, SingleMonth};

    fn make_month(name: &str, income: f64, expenses: f64) -> SingleMonth {
        SingleMonth {
            month_name: name.to_string(),
            income: vec![],
            expenses: vec![],
            total_income: income,
            total_expenses: expenses,
        }
    }

    #[test]
    fn last_year_match_finds_same_month_name() {
        let mut app = App::default();
        let jan = make_month("January", 3000.0, 2000.0);
        app.combined_months = vec![(jan.clone(), false)];
        app.last_year = MonthlyData {
            months: vec![make_month("January", 2800.0, 1900.0)],
        };
        app.combined_selected = 0;

        let ly = app
            .last_year_match()
            .expect("should find January in last_year");
        assert_eq!(ly.month_name, "January");
        assert!((ly.total_income - 2800.0).abs() < f64::EPSILON);
    }

    #[test]
    fn last_year_match_returns_none_when_missing() {
        let mut app = App::default();
        let feb = make_month("February", 3000.0, 2000.0);
        app.combined_months = vec![(feb, false)];
        app.last_year = MonthlyData {
            months: vec![make_month("January", 2800.0, 1900.0)],
        };
        app.combined_selected = 0;

        assert!(app.last_year_match().is_none());
    }

    #[test]
    fn yoy_view_defaults_to_false() {
        let app = App::default();
        assert!(!app.yoy_view);
    }

    #[test]
    fn pct_change_zero_base_returns_zero() {
        assert_eq!(pct_change(0.0, 100.0), 0.0);
    }

    #[test]
    fn pct_change_positive() {
        assert!((pct_change(1000.0, 100.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn signed_indicator_income_up_is_positive() {
        use crate::config::Config;
        use crate::ui::Theme;
        let theme = Theme::from_config(&Config::default());
        let (arrow, color) = signed_indicator(100.0, true, &theme);
        assert_eq!(arrow, "↑");
        assert_eq!(color, theme.positive);
    }

    #[test]
    fn signed_indicator_expenses_up_is_negative() {
        use crate::config::Config;
        use crate::ui::Theme;
        let theme = Theme::from_config(&Config::default());
        let (arrow, color) = signed_indicator(100.0, false, &theme);
        assert_eq!(arrow, "↑");
        assert_eq!(color, theme.negative);
    }
}
