use ratatui::style::Modifier;
use ratatui::text::Span;
use ratatui::widgets::{
    Axis, Bar, BarChart, BarGroup, Block, BorderType, Borders, Chart, Dataset, GraphType, Padding,
    Paragraph,
};
use ratatui::{prelude::*, symbols, Frame};

use crate::app::{App, CategoryShare, MonthBreakdown};
use crate::config::Config;

use super::{expense_color, nice_y_axis, Theme};

/// Full-width threshold (typical 1920×1080 fullscreen ≈ 240 cols).
const FULL_WIDTH_MIN: u16 = 160;
/// Per-panel threshold for roomy layout (half of ~240 cols minus chrome).
const PANEL_SPACIOUS_MIN: u16 = 55;

pub(super) fn render_dashboard(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    if app.tabs_loaded.get(crate::app::Tab::Dashboard)
        && !app.loading
        && app.combined_months.is_empty()
    {
        render_dashboard_no_data(f, area, theme, &app.config);
        return;
    }

    let month_title = app
        .current_month()
        .map(|m| m.month_name.as_str())
        .unwrap_or("—");
    let forecast = if app.current_month_is_forecast() {
        "  · forecast"
    } else {
        ""
    };

    let full_width = area.width >= FULL_WIDTH_MIN;
    let grid = Layout::vertical([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .margin(1)
        .split(area);

    let h_gap = if full_width { 3 } else { 1 };
    let top = Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .margin(h_gap)
        .split(grid[0]);
    let bottom = Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .margin(h_gap)
        .split(grid[1]);

    let breakdown = app.month_breakdown();
    render_summary(
        f,
        top[0],
        theme,
        &app.config,
        month_title,
        forecast,
        &breakdown,
    );
    render_comparison_chart(f, top[1], theme, &breakdown);

    let categories = app.top_expense_categories(5);
    render_category_breakdown(f, app, bottom[0], theme, &categories);

    let cash = app.cash_overview();
    render_cash_panel(f, bottom[1], app, theme, &app.config, month_title, &cash);
}

fn panel_spacious(area: Rect) -> bool {
    area.width >= PANEL_SPACIOUS_MIN && area.height >= 10
}

fn render_dashboard_no_data(f: &mut Frame, area: Rect, theme: &Theme, cfg: &Config) {
    let exp = &cfg.expenses_account;
    let inc = &cfg.income_account;
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "No income or expense data for the dashboard",
            Style::default().fg(theme.accent).bold(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Looking for accounts under  \"{inc}\"  and  \"{exp}\""),
            Style::default().fg(theme.fg),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Set income_account / expenses_account in ~/.config/ldash/config.toml",
            Style::default().fg(theme.muted),
        )),
    ];
    let block = panel_block(" Dashboard ".to_string(), theme.gold, theme);
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
}

fn panel_block(title: String, title_color: Color, theme: &Theme) -> Block<'_> {
    Block::default()
        .title(Span::styled(title, Style::default().fg(title_color).bold()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.muted))
        .padding(Padding::new(4, 4, 2, 1))
}

fn render_summary(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    cfg: &Config,
    month: &str,
    forecast: &str,
    b: &MonthBreakdown,
) {
    let spacious = panel_spacious(area);
    let label_w = if spacious {
        area.width.saturating_sub(22) as usize
    } else {
        26
    };
    let label_w = label_w.clamp(22, 34);
    let title = format!(" {month}{forecast}  [←/→ month]  [G current] ");
    let rows = [
        ("Total Income", b.total_income, theme.positive),
        ("Recurring Income", b.recurring_income, theme.positive),
        ("Recurring Expenses", b.recurring_expenses, theme.negative),
        ("Other Expenses", b.other_expenses, theme.negative),
        ("Total Expenses", b.total_expenses, theme.negative),
    ];

    let mut lines: Vec<Line> = Vec::new();
    if spacious {
        lines.push(Line::from(""));
    }
    for (label, amount, color) in rows {
        lines.push(Line::from(vec![
            Span::styled(format!("{label:<label_w$}"), Style::default().fg(theme.fg)),
            Span::styled(
                format!("{:>16}", cfg.fmt_amount(amount, 2)),
                Style::default().fg(color),
            ),
        ]));
        if spacious {
            lines.push(Line::from(""));
        }
    }
    let net = b.total_income - b.total_expenses;
    let net_color = if net >= 0.0 {
        theme.positive
    } else {
        theme.negative
    };
    if !spacious {
        lines.push(Line::from(""));
    }
    lines.push(Line::from(vec![
        Span::styled("Net Balance", Style::default().fg(theme.gold).bold()),
        Span::raw(" ".repeat(label_w.saturating_sub(11))),
        Span::styled(
            format!("{:>16}", cfg.fmt_amount(net, 2)),
            Style::default().fg(net_color).bold(),
        ),
    ]));

    f.render_widget(
        Paragraph::new(lines).block(panel_block(title, theme.gold, theme)),
        area,
    );
}

fn render_comparison_chart(f: &mut Frame, area: Rect, theme: &Theme, b: &MonthBreakdown) {
    let spacious = panel_spacious(area);
    let bars = [
        ("Income", b.total_income, theme.positive),
        ("Recurring", b.recurring_income, Color::LightGreen),
        ("Expenses", b.total_expenses, theme.negative),
        ("Recurring", b.recurring_expenses, Color::LightRed),
    ];

    let bar_w = if spacious {
        if area.width >= 100 {
            (area.width.saturating_sub(40) / 4).clamp(14, 24)
        } else {
            (area.width.saturating_sub(24) / 4).clamp(6, 10)
        }
    } else {
        6
    };
    let group_gap = if area.width >= 100 {
        14
    } else if spacious {
        6
    } else {
        4
    };
    let bar_gap = if spacious { 3 } else { 2 };

    let groups: Vec<BarGroup> = bars
        .iter()
        .enumerate()
        .map(|(i, (_label, value, color))| {
            let tag = match i {
                0 => "Income",
                1 => "Rec. In",
                2 => "Expenses",
                _ => "Rec. Out",
            };
            BarGroup::default()
                .label(Line::from(Span::styled(
                    tag,
                    Style::default().fg(theme.muted),
                )))
                .bars(&[Bar::default()
                    .value(value.max(0.0) as u64)
                    .style(Style::default().fg(*color))])
        })
        .collect();

    let mut chart = BarChart::default()
        .style(Style::default().bg(theme.background))
        .block(panel_block(
            " Month Overview ".to_string(),
            theme.accent,
            theme,
        ))
        .bar_width(bar_w)
        .bar_gap(bar_gap)
        .group_gap(group_gap);

    for g in groups {
        chart = chart.data(g);
    }

    f.render_widget(chart, area);
}

fn render_category_breakdown(
    f: &mut Frame,
    app: &App,
    area: Rect,
    theme: &Theme,
    categories: &[CategoryShare],
) {
    let spacious = panel_spacious(area);
    let block = panel_block(" Top 5 Categories ".to_string(), theme.accent, theme);

    if categories.is_empty() {
        f.render_widget(Paragraph::new("No expenses this month").block(block), area);
        return;
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    let gap = if spacious { 3 } else { 2 };

    let display = categories_for_donut(categories);

    let split = if spacious && inner.width >= 28 {
        let donut_cols = inner.height.min(inner.width * 2 / 5).clamp(14, 40);
        Layout::horizontal([Constraint::Length(donut_cols), Constraint::Min(0)])
            .spacing(gap)
            .split(inner)
    } else {
        let donut_rows = inner.height.saturating_sub(7).clamp(6, 10);
        Layout::vertical([Constraint::Length(donut_rows), Constraint::Min(4)])
            .spacing(gap)
            .split(inner)
    };

    let donut_area = centered_donut_area(split[0]);

    render_braille_donut(f, donut_area, app, theme, &display);
    render_category_legend(f, split[1], app, theme, &display, &app.config, spacious);
}

/// Pick a centered canvas where braille dot width equals dot height (cols × 2 = rows × 4).
fn centered_donut_area(container: Rect) -> Rect {
    let (cols, rows) = donut_canvas_size(container.width, container.height);
    Rect {
        x: container.x + container.width.saturating_sub(cols) / 2,
        y: container.y + container.height.saturating_sub(rows) / 2,
        width: cols,
        height: rows,
    }
}

fn donut_canvas_size(max_cols: u16, max_rows: u16) -> (u16, u16) {
    if max_cols < 4 || max_rows < 3 {
        return (max_cols.max(1), max_rows.max(1));
    }
    let rows = max_rows.min(max_cols / 2).clamp(3, 20);
    let cols = (rows * 2).min(max_cols);
    let rows = cols / 2;
    (cols.max(4), rows.max(3))
}

/// Top-N shares are fractions of all expenses; fill the ring with an "Other" slice when needed.
fn categories_for_donut(categories: &[CategoryShare]) -> Vec<CategoryShare> {
    if categories.is_empty() {
        return Vec::new();
    }
    let shown: f64 = categories.iter().map(|c| c.fraction).sum();
    let mut out = categories.to_vec();
    let remainder = 1.0 - shown;
    if remainder > 0.001 {
        let total = categories
            .iter()
            .find(|c| c.fraction > 0.0)
            .map(|c| c.amount / c.fraction)
            .unwrap_or(0.0);
        out.push(CategoryShare {
            name: "Other".to_string(),
            amount: total * remainder,
            fraction: remainder,
        });
    }
    out
}

fn render_braille_donut(
    f: &mut Frame,
    area: Rect,
    app: &App,
    theme: &Theme,
    categories: &[CategoryShare],
) {
    let cols = area.width as usize;
    let rows = area.height as usize;
    if cols == 0 || rows == 0 {
        return;
    }

    let dot_w = cols * 2;
    let dot_h = rows * 4;
    let cx = dot_w as f64 / 2.0;
    let cy = dot_h as f64 / 2.0;
    let outer = dot_w.min(dot_h) as f64 / 2.0 - 1.0;
    let inner_r = outer * 0.52;

    let mut cum = 0.0;
    let slices: Vec<(f64, f64, Color)> = categories
        .iter()
        .map(|c| {
            let start = cum;
            cum += c.fraction;
            (start, cum, category_share_color(&c.name, app, theme))
        })
        .collect();

    let geo = DonutGeometry {
        cx,
        cy,
        outer,
        inner_r,
        slices: &slices,
        bg: theme.background,
    };

    for char_y in 0..rows {
        let mut spans: Vec<Span> = Vec::with_capacity(cols);
        for char_x in 0..cols {
            let (bits, color) = braille_cell_bits(&geo, char_x, char_y);
            if bits == 0 {
                spans.push(Span::raw(" "));
            } else {
                spans.push(Span::styled(
                    char::from_u32(0x2800 + bits as u32)
                        .unwrap_or(' ')
                        .to_string(),
                    Style::default().fg(color),
                ));
            }
        }
        f.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect {
                x: area.x,
                y: area.y + char_y as u16,
                width: area.width,
                height: 1,
            },
        );
    }
}

struct DonutGeometry<'a> {
    cx: f64,
    cy: f64,
    outer: f64,
    inner_r: f64,
    slices: &'a [(f64, f64, Color)],
    bg: Color,
}

fn braille_cell_bits(geo: &DonutGeometry<'_>, char_x: usize, char_y: usize) -> (u8, Color) {
    let mut bits = 0u8;
    let mut color = geo.bg;
    let mut color_hits = 0u8;

    for dy in 0..4u8 {
        for dx in 0..2u8 {
            let px = char_x * 2 + dx as usize;
            let py = char_y * 4 + dy as usize;
            let dot_x = px as f64 + 0.5;
            let dot_y = py as f64 + 0.5;
            let ddx = dot_x - geo.cx;
            let ddy = dot_y - geo.cy;
            let dist = (ddx * ddx + ddy * ddy).sqrt();
            if dist > geo.outer || dist < geo.inner_r {
                continue;
            }
            let mut angle = (dot_y - geo.cy).atan2(dot_x - geo.cx) / (2.0 * std::f64::consts::PI);
            if angle < 0.0 {
                angle += 1.0;
            }
            for (i, (start, end, col)) in geo.slices.iter().enumerate() {
                let last = i + 1 == geo.slices.len();
                if angle >= *start && (angle < *end || (last && angle <= *end)) {
                    bits |= braille_dot_bit(dx, dy);
                    if color == geo.bg {
                        color = *col;
                        color_hits = 1;
                    } else if color != *col {
                        color_hits += 1;
                        if color_hits > 4 {
                            color = *col;
                        }
                    }
                    break;
                }
            }
        }
    }

    (bits, color)
}

fn braille_dot_bit(dx: u8, dy: u8) -> u8 {
    match (dx, dy) {
        (0, 0) => 0x01,
        (1, 0) => 0x08,
        (0, 1) => 0x02,
        (1, 1) => 0x10,
        (0, 2) => 0x04,
        (1, 2) => 0x20,
        (0, 3) => 0x40,
        (1, 3) => 0x80,
        _ => 0,
    }
}

fn category_share_color(name: &str, app: &App, theme: &Theme) -> Color {
    if name == "Other" {
        theme.muted
    } else {
        expense_color(name, app, theme)
    }
}

fn render_category_legend(
    f: &mut Frame,
    area: Rect,
    app: &App,
    theme: &Theme,
    categories: &[CategoryShare],
    cfg: &Config,
    spacious: bool,
) {
    let mut lines: Vec<Line> = Vec::new();
    let name_w = if spacious {
        area.width.saturating_sub(18).clamp(8, 20) as usize
    } else {
        14
    };
    for c in categories {
        let color = category_share_color(&c.name, app, theme);
        let pct = c.fraction * 100.0;
        if spacious {
            lines.push(Line::from(vec![
                Span::styled("■ ", Style::default().fg(color)),
                Span::styled(
                    format!("{name:<name_w$}", name = truncate(&c.name, name_w)),
                    Style::default().fg(theme.fg),
                ),
                Span::styled(format!("{:>5.1}%", pct), Style::default().fg(theme.muted)),
                Span::raw("  "),
                Span::styled(
                    cfg.fmt_amount(c.amount, 0),
                    Style::default()
                        .fg(theme.negative)
                        .add_modifier(Modifier::DIM),
                ),
            ]));
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(vec![
                Span::styled("■ ", Style::default().fg(color)),
                Span::styled(
                    format!("{:<14}", truncate(&c.name, 14)),
                    Style::default().fg(theme.fg),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(format!("{:>5.1}%", pct), Style::default().fg(theme.muted)),
                Span::raw("  "),
                Span::styled(
                    cfg.fmt_amount(c.amount, 0),
                    Style::default()
                        .fg(theme.negative)
                        .add_modifier(Modifier::DIM),
                ),
            ]));
        }
    }

    let content_h = lines.len() as u16;
    let y = area.y + area.height.saturating_sub(content_h) / 2;
    f.render_widget(
        Paragraph::new(lines),
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: content_h.min(area.height),
        },
    );
}

fn render_cash_panel(
    f: &mut Frame,
    area: Rect,
    app: &App,
    theme: &Theme,
    cfg: &Config,
    month: &str,
    cash: &crate::app::CashOverview,
) {
    let block = panel_block(" Cash Accounts ".to_string(), theme.accent, theme);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if cash.accounts.is_empty() {
        let lines = vec![
            Line::from(Span::styled(
                "No liquid accounts configured.",
                Style::default().fg(theme.muted),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Set liquid_accounts in config, or use assets:bank / assets:cash.",
                Style::default().fg(theme.muted),
            )),
        ];
        f.render_widget(Paragraph::new(lines), inner);
        return;
    }

    let footer_h = if panel_spacious(area) { 3 } else { 2 };
    let chart = app.liquid_account_chart(5);
    let legend_h = account_legend_height(chart.series.len());
    let split = Layout::vertical([
        Constraint::Min(6),
        Constraint::Length(legend_h),
        Constraint::Length(footer_h),
    ])
    .split(inner);

    render_liquid_accounts_chart(f, split[0], theme, cfg, &chart, app.selected_month_index());
    render_account_chart_legend(f, split[1], theme, &chart);
    render_cash_footer(f, split[2], theme, cfg, month, cash);
}

fn account_legend_height(series_count: usize) -> u16 {
    series_count as u16
}

fn account_line_colors(theme: &Theme) -> [Color; 5] {
    [
        theme.accent,
        theme.gold,
        theme.positive,
        Color::Cyan,
        Color::Magenta,
    ]
}

fn render_liquid_accounts_chart(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    cfg: &Config,
    chart: &crate::app::LiquidAccountChart,
    selected_month: Option<usize>,
) {
    if chart.series.is_empty() || area.width < 12 || area.height < 4 {
        f.render_widget(
            Paragraph::new(Span::styled(
                "No monthly account activity yet",
                Style::default().fg(theme.muted),
            )),
            area,
        );
        return;
    }

    let colors = account_line_colors(theme);
    let mut datasets: Vec<Dataset> = chart
        .series
        .iter()
        .enumerate()
        .map(|(i, (name, points))| {
            Dataset::default()
                .name(truncate(name, 12))
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(colors[i % colors.len()]))
                .data(points)
        })
        .collect();

    let (y_min, y_max, y_labels) = nice_y_axis(chart.y_min, chart.y_max, 3, cfg, theme.muted);

    let marker_data: Vec<(f64, f64)> = selected_month
        .map(|idx| vec![(idx as f64, y_min), (idx as f64, y_max)])
        .unwrap_or_default();
    if !marker_data.is_empty() {
        datasets.push(
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(theme.gold).add_modifier(Modifier::DIM))
                .data(&marker_data),
        );
    }

    let mut label_indices: Vec<usize> = if area.width < 50 {
        vec![0, 5, 11]
    } else {
        vec![0, 2, 5, 8, 11]
    };
    if let Some(sel) = selected_month {
        if !label_indices.contains(&sel) {
            label_indices.push(sel);
            label_indices.sort();
        }
    }
    let x_labels: Vec<Span> = label_indices
        .into_iter()
        .map(|idx| {
            let is_selected = selected_month == Some(idx);
            let label = if is_selected {
                format!("▲{}", &crate::data::MONTH_NAMES[idx][..3])
            } else {
                crate::data::MONTH_NAMES[idx][..3].to_string()
            };
            Span::styled(
                label,
                Style::default().fg(if is_selected { theme.gold } else { theme.muted }),
            )
        })
        .collect();

    let widget = Chart::new(datasets)
        .style(Style::default().bg(theme.background))
        .x_axis(
            Axis::default()
                .bounds([0.0, chart.x_max])
                .labels(x_labels)
                .style(Style::default().fg(theme.muted)),
        )
        .y_axis(
            Axis::default()
                .bounds([y_min, y_max])
                .labels(y_labels)
                .style(Style::default().fg(theme.muted)),
        );

    f.render_widget(widget, area);
}

fn render_account_chart_legend(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    chart: &crate::app::LiquidAccountChart,
) {
    if chart.series.is_empty() || area.height == 0 {
        return;
    }

    let colors = account_line_colors(theme);
    let name_w = area.width.saturating_sub(2) as usize;
    let lines: Vec<Line> = chart
        .series
        .iter()
        .enumerate()
        .map(|(i, (name, _))| {
            Line::from(vec![
                Span::styled("■ ", Style::default().fg(colors[i % colors.len()])),
                Span::styled(truncate(name, name_w), Style::default().fg(theme.fg)),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines), area);
}

fn render_cash_footer(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    cfg: &Config,
    month: &str,
    cash: &crate::app::CashOverview,
) {
    let ytd_color = if cash.ytd_net_change >= 0.0 {
        theme.positive
    } else {
        theme.negative
    };
    let line = Line::from(vec![
        Span::styled("Total ", Style::default().fg(theme.gold).bold()),
        Span::styled(
            cfg.fmt_amount(cash.total_balance, 0),
            Style::default().fg(theme.gold),
        ),
        Span::raw("   "),
        Span::styled(
            format!("YTD through {month} "),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            cfg.fmt_amount(cash.ytd_net_change, 0),
            Style::default().fg(ytd_color),
        ),
    ]);
    f.render_widget(
        Paragraph::new(line),
        Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(1),
            width: area.width,
            height: 1,
        },
    );
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!(
            "{}…",
            s.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_for_donut_adds_other_slice_for_remainder() {
        let categories = vec![
            CategoryShare {
                name: "a".to_string(),
                amount: 500.0,
                fraction: 0.5,
            },
            CategoryShare {
                name: "b".to_string(),
                amount: 300.0,
                fraction: 0.3,
            },
        ];
        let display = categories_for_donut(&categories);
        assert_eq!(display.len(), 3);
        assert_eq!(display[2].name, "Other");
        assert!((display[2].fraction - 0.2).abs() < 0.001);
        let sum: f64 = display.iter().map(|c| c.fraction).sum();
        assert!((sum - 1.0).abs() < 0.001);
    }

    #[test]
    fn donut_canvas_is_square_in_dot_space() {
        let (cols, rows) = donut_canvas_size(14, 8);
        assert_eq!(cols * 2, rows * 4);
        assert_eq!((cols, rows), (14, 7));

        let (cols, rows) = donut_canvas_size(14, 5);
        assert_eq!(cols * 2, rows * 4);
        assert_eq!((cols, rows), (10, 5));
    }
}
