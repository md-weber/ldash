use ratatui::{prelude::*, widgets::*};

use crate::app::App;
use crate::config::Config;
use super::{Theme, nice_y_axis, render_detail_with_title};

pub(super) fn render_accounts(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let chunks = Layout::vertical([
        Constraint::Percentage(40), // net worth chart
        Constraint::Length(3),      // net worth number
        Constraint::Min(0),         // accounts table or detail
    ])
    .split(area);

    render_net_worth_chart(f, app, chunks[0], theme);

    let net_worth = app.total_net_worth();
    let nw_text = Line::from(vec![
        Span::styled("  Net Worth: ", Style::default().fg(theme.muted).bold()),
        Span::styled(
            app.config.fmt_amount(net_worth, 2),
            Style::default().fg(theme.gold).bold(),
        ),
    ]);
    let nw_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.muted));
    f.render_widget(Paragraph::new(nw_text).block(nw_block), chunks[1]);

    let has_goals = !app.config.goals.is_empty();

    if let Some(txns) = app.account_detail.as_ref() {
        let name = app.detail_account_name.as_ref().unwrap();
        render_account_detail(f, txns, name, &app.config, chunks[2], theme, &mut app.detail_state);
    } else if has_goals {
        let goal_h = (app.config.goals.len() as u16 + 2).min(8);
        let bottom = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(goal_h),
        ]).split(chunks[2]);

        if !app.liabilities.is_empty() {
            let split = Layout::vertical([
                Constraint::Percentage(70),
                Constraint::Percentage(30),
            ]).split(bottom[0]);
            render_accounts_table(f, app, split[0], theme);
            render_liabilities_table(f, app, split[1], theme);
        } else {
            render_accounts_table(f, app, bottom[0], theme);
        }

        render_goals(f, app, bottom[1], theme);
    } else if !app.liabilities.is_empty() {
        let split = Layout::vertical([
            Constraint::Percentage(70),
            Constraint::Percentage(30),
        ])
        .split(chunks[2]);
        render_accounts_table(f, app, split[0], theme);
        render_liabilities_table(f, app, split[1], theme);
    } else {
        render_accounts_table(f, app, chunks[2], theme);
    }
}

fn render_net_worth_chart(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let range_label = app.nw_range.label();
    let block = Block::default()
        .title(Span::styled(
            format!(" Net Worth History [{range_label}]  ◀ ▶ "),
            Style::default().fg(theme.gold).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.muted));

    let series = &app.net_worth_history;
    if series.points.len() < 2 {
        f.render_widget(
            Paragraph::new(Span::styled("Not enough data", Style::default().fg(theme.muted)))
                .block(block)
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let x_min = series.points.first().unwrap().0;
    let x_max = series.points.last().unwrap().0;
    let y_min_raw = series.points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let y_max_raw = series.points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let (y_min, y_max, y_labels) = nice_y_axis(y_min_raw, y_max_raw, 4, &app.config, theme.muted);

    let label_count = if area.width < 60 { 3 } else { 5 };
    let n = series.labels.len();
    let x_labels: Vec<Span> = (0..label_count)
        .map(|i| i * n.saturating_sub(1) / (label_count - 1).max(1))
        .filter_map(|i| series.labels.get(i))
        .map(|(d, _)| Span::styled(d.format("%b %y").to_string(), Style::default().fg(theme.muted)))
        .collect();

    let dataset = Dataset::default()
        .name("Net Worth")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(theme.gold))
        .data(&series.points);

    let chart = Chart::new(vec![dataset])
        .style(Style::default().bg(theme.background))
        .block(block)
        .x_axis(
            Axis::default()
                .style(Style::default().fg(theme.muted))
                .bounds([x_min, x_max])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(theme.muted))
                .bounds([y_min, y_max])
                .labels(y_labels),
        );

    f.render_widget(chart, area);
}

fn render_account_detail(
    f: &mut Frame,
    txns: &[crate::data::Transaction],
    name: &str,
    cfg: &Config,
    area: Rect,
    theme: &Theme,
    state: &mut ratatui::widgets::TableState,
) {
    let title = format!(" {} — Recent Transactions  [Esc back] ", name);
    render_detail_with_title(f, txns, &title, cfg, area, theme, state);
}

fn render_accounts_table(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let show_filter = app.account_filter_active || !app.account_filter.is_empty();

    let (filter_area, table_area) = if show_filter {
        let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(area);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, area)
    };

    if let Some(fa) = filter_area {
        let text = format!("{}_", app.account_filter);
        let para = Paragraph::new(text).block(
            Block::default()
                .title(Span::styled(" Filter ", Style::default().fg(theme.accent).bold()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.accent)),
        );
        f.render_widget(para, fa);
    }

    let narrow = table_area.width < 100;
    let bar_width = if narrow { 0 } else { (table_area.width as f64 * 0.2) as usize };

    // Collect filtered accounts into owned data to avoid borrow conflicts.
    let accounts: Vec<(String, f64)> = app
        .filtered_accounts()
        .into_iter()
        .map(|b| (b.account.clone(), b.amount))
        .collect();

    let max_amount = accounts.iter().map(|(_, a)| a.abs()).fold(0.0_f64, f64::max);

    let rows: Vec<Row> = if accounts.is_empty() && show_filter {
        vec![Row::new(vec![Cell::from(Span::styled(
            "  No accounts match",
            Style::default().fg(theme.muted),
        ))])]
    } else {
        accounts
            .iter()
            .map(|(account, amount)| {
                let amount_style = if *amount >= 0.0 {
                    Style::default().fg(theme.positive)
                } else {
                    Style::default().fg(theme.negative)
                };

                let amount_str = format!("{:>15}", app.config.fmt_amount(*amount, 2));

                let short = account
                    .strip_prefix("assets:")
                    .unwrap_or(account);
                let name = if narrow && short.len() > 30 {
                    format!("  {}…", &short[..29])
                } else {
                    format!("  {}", short)
                };

                let mut cells = vec![
                    Cell::from(name).style(Style::default().fg(theme.fg)),
                    Cell::from(amount_str).style(amount_style),
                ];

                if !narrow {
                    let bar_len = if max_amount > 0.0 {
                        ((amount.abs() / max_amount) * bar_width as f64) as usize
                    } else {
                        0
                    };
                    let bar = "█".repeat(bar_len);
                    cells.push(Cell::from(bar).style(Style::default().fg(theme.accent)));
                }

                Row::new(cells)
            })
            .collect()
    };

    let widths: Vec<Constraint> = if narrow {
        vec![Constraint::Min(30), Constraint::Length(16)]
    } else {
        vec![Constraint::Min(38), Constraint::Length(16), Constraint::Min(10)]
    };

    let bal_header = format!("Balance ({})", app.config.currency_symbol);
    let mut header: Vec<String> = vec!["Account".into(), bal_header];
    if !narrow {
        header.push(String::new());
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
                    " Asset Balances  [Enter drill-down]  [type to filter] ",
                    Style::default().fg(theme.accent).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.muted)),
        );

    app.table_area = table_area;
    f.render_stateful_widget(table, table_area, &mut app.account_state);
}

fn render_liabilities_table(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let rows: Vec<Row> = app
        .liabilities
        .iter()
        .map(|b| {
            let amount_str = format!("{:>15}", app.config.fmt_amount(b.amount, 2));
            let short = b.account.strip_prefix("liabilities:").unwrap_or(&b.account);
            Row::new(vec![
                Cell::from(format!("  {}", short)).style(Style::default().fg(theme.fg)),
                Cell::from(amount_str).style(Style::default().fg(theme.negative)),
            ])
        })
        .collect();

    let widths = [Constraint::Min(38), Constraint::Length(16)];

    let total_liab: f64 = app.liabilities.iter().map(|b| b.amount).sum();
    let title = format!(" Liabilities  ({}) ", app.config.fmt_amount(total_liab, 2));

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![
                "Account".to_string(),
                format!("Balance ({})", app.config.currency_symbol),
            ])
                .style(Style::default().fg(theme.muted).bold())
                .bottom_margin(1),
        )
        .block(
            Block::default()
                .title(Span::styled(title, Style::default().fg(theme.negative).bold()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.muted)),
        );

    f.render_widget(table, area);
}

fn render_goals(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .title(Span::styled(" Savings Goals ", Style::default().fg(theme.gold).bold()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.muted));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let goals = app.goal_progress();
    for (i, g) in goals.iter().enumerate() {
        if i as u16 >= inner.height {
            break;
        }
        let y = inner.y + i as u16;
        let color = if g.pct >= 100.0 { theme.positive }
                   else if g.pct >= 60.0 { theme.gold }
                   else { theme.accent };

        let label_w = 18u16.min(inner.width / 3);
        let pct_w = 22u16;
        let bar_w = inner.width.saturating_sub(label_w + pct_w);

        let label = Span::styled(
            format!(" {:<w$}", g.name, w = (label_w - 1) as usize),
            Style::default().fg(theme.fg),
        );
        f.render_widget(Paragraph::new(Line::from(label)),
            Rect { x: inner.x, y, width: label_w, height: 1 });

        let filled = ((g.pct / 100.0).min(1.0) * bar_w as f64) as usize;
        let empty_b = bar_w as usize - filled;
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty_b));
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(bar, Style::default().fg(color)))),
            Rect { x: inner.x + label_w, y, width: bar_w, height: 1 },
        );

        let info = Span::styled(
            format!(
                " {}/{} {:>3.0}%",
                app.config.fmt_amount_compact(g.current, 0),
                app.config.fmt_amount_compact(g.target, 0),
                g.pct
            ),
            Style::default().fg(theme.muted),
        );
        f.render_widget(Paragraph::new(Line::from(info)),
            Rect { x: inner.x + label_w + bar_w, y, width: pct_w, height: 1 });
    }
}
