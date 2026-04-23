use ratatui::{prelude::*, widgets::*};

use crate::app::App;

use super::Theme;

pub(super) fn render_loading_overlay(f: &mut Frame, area: Rect, theme: &Theme) {
    let w = 26u16.min(area.width.saturating_sub(4));
    let h = 3u16.min(area.height.saturating_sub(4));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent));
    let text = Paragraph::new(Line::from(vec![
        Span::styled("⏳ ", Style::default().fg(theme.gold)),
        Span::styled("Loading data…", Style::default().fg(theme.fg).bold()),
    ]))
    .alignment(Alignment::Center)
    .block(block);
    f.render_widget(text, popup);
}

pub(super) fn render_help_popup(f: &mut Frame, area: Rect, theme: &Theme) {
    let w = 68u16.min(area.width.saturating_sub(4));
    let h = 30u16.min(area.height.saturating_sub(4));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };

    f.render_widget(Clear, popup);

    // ("§", "Title") → section header   ("", "") → blank spacer
    let bindings: &[(&str, &str)] = &[
        ("", ""),
        ("§", "Navigation"),
        ("1/2/3  Tab/⇧Tab", "Switch tab"),
        ("↑k / ↓j", "Scroll / select row"),
        ("PgUp/PgDn  Home/End", "Page up/down · first/last"),
        ("←h / →l", "Month · NW range · chart range"),
        ("Enter  /", "Open detail · search"),
        ("", ""),
        ("§", "Tab-specific"),
        ("y / Y", "Year back / forward  (Monthly)"),
        ("i", "Income/expense focus  (Monthly)"),
        ("a–z  Backspace", "Filter · clear char  (Accounts)"),
        ("", ""),
        ("§", "Data & view"),
        ("e", "Export view to file"),
        ("Y", "Copy view to clipboard  (non-Monthly)"),
        ("s", "Toggle chart stacked / unstacked"),
        ("c", "Toggle expense colors"),
        ("r", "Refresh data"),
        ("", ""),
        ("§", "General"),
        ("? / q", "Toggle help / quit"),
        ("Esc", "Close detail / back"),
        ("Mouse click", "Select tab / row"),
        ("Scroll wheel", "Scroll table"),
    ];

    let mut lines: Vec<Line> = Vec::new();
    for (key, desc) in bindings {
        if *key == "§" {
            lines.push(Line::from(Span::styled(
                format!("  {}", desc),
                Style::default().fg(theme.gold).bold(),
            )));
        } else if key.is_empty() {
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("    {:<22}", key),
                    Style::default().fg(theme.accent).bold(),
                ),
                Span::styled(*desc, Style::default().fg(theme.fg)),
            ]));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  P/L shows unrealized gains only.",
        Style::default().fg(theme.muted),
    )));

    let block = Block::default()
        .title(Span::styled(
            " Keybindings ",
            Style::default().fg(theme.gold).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .style(Style::default().bg(theme.background));

    f.render_widget(Paragraph::new(lines).block(block), popup);
}

pub(super) fn render_search_overlay(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    f.render_widget(Clear, area);

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    let input = Paragraph::new(Line::from(vec![
        Span::styled("  / ", Style::default().fg(theme.gold).bold()),
        Span::styled(&app.search_query, Style::default().fg(theme.fg)),
        Span::styled("█", Style::default().fg(theme.accent)),
    ]))
    .block(
        Block::default()
            .title(Span::styled(
                " Search Transactions ",
                Style::default().fg(theme.accent).bold(),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.accent)),
    );
    f.render_widget(input, chunks[0]);

    let rows: Vec<Row> = app
        .search_results
        .iter()
        .map(|t| {
            let amt_color = if t.amount >= 0.0 {
                theme.positive
            } else {
                theme.negative
            };
            Row::new(vec![
                Cell::from(t.date.format("%Y-%m-%d").to_string())
                    .style(Style::default().fg(theme.muted)),
                Cell::from(t.description.clone()).style(Style::default().fg(theme.fg)),
                Cell::from(format!("{:>13}", app.config.fmt_amount(t.amount, 2)))
                    .style(Style::default().fg(amt_color)),
                Cell::from(format!("{:>13}", app.config.fmt_amount(t.running_total, 2)))
                    .style(Style::default().fg(theme.accent)),
            ])
        })
        .collect();

    let result_count = app.search_results.len();
    let title = if result_count > 0 {
        format!(" {} results ", result_count)
    } else if app.search_query.is_empty() {
        " Type query, Enter to search ".to_string()
    } else {
        " No results ".to_string()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Min(24),
            Constraint::Length(14),
            Constraint::Length(14),
        ],
    )
    .header(
        Row::new(vec!["Date", "Description", "Amount", "Balance"])
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

    f.render_stateful_widget(table, chunks[1], &mut app.search_state);

    let hint = Paragraph::new(Line::from(vec![
        Span::styled(
            "  [Enter] search  [↑↓] navigate  [Esc] close",
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            "  Supports regex (e.g. \"grocery|supermarket\")",
            Style::default().fg(theme.muted),
        ),
    ]));
    f.render_widget(hint, chunks[2]);
}

pub(super) fn render_price_alerts(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    if !app.show_alerts || app.price_alerts.is_empty() {
        return;
    }

    let h = 3u16.min(area.height);
    let banner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: h,
    };
    f.render_widget(Clear, banner);

    let threshold = app.config.price_alert_threshold_pct.max(0.0);
    let heading = if threshold > 0.0 {
        format!("  Price moves >= {:.1}%: ", threshold)
    } else {
        "  Price moves: ".to_string()
    };
    let mut spans = vec![Span::styled(
        heading,
        Style::default().fg(theme.gold).bold(),
    )];
    for (i, alert) in app.price_alerts.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(", ", Style::default().fg(theme.muted)));
        }
        let (prefix, color) = if alert.change_pct >= 0.0 {
            ("+", theme.positive)
        } else {
            ("", theme.negative)
        };
        spans.push(Span::styled(
            format!("{} {prefix}{:.1}%", alert.coin, alert.change_pct),
            Style::default().fg(color).bold(),
        ));
    }
    spans.push(Span::styled(
        "  [any key dismiss]",
        Style::default().fg(theme.muted),
    ));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.gold))
        .style(Style::default().bg(theme.background));

    f.render_widget(Paragraph::new(Line::from(spans)).block(block), banner);
}

pub(super) fn render_status(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    if app.export_prompt_active {
        let text = Line::from(vec![
            Span::styled("  Export to: ", Style::default().fg(theme.gold).bold()),
            Span::styled(&app.export_prompt_path, Style::default().fg(theme.fg)),
            Span::styled("█", Style::default().fg(theme.accent)),
            Span::styled(
                "  [Enter] confirm  [Esc] cancel",
                Style::default().fg(theme.muted),
            ),
        ]);
        f.render_widget(Paragraph::new(text), area);
        return;
    }

    let help = "  [1-3] tab  [↑↓/jk] navigate  [PgUp/PgDn/Home/End] scroll  [←→/hl] month/range  [r] refresh  [?] help  [q] quit";
    let text = Line::from(vec![
        Span::styled(&app.status_msg, Style::default().fg(theme.accent)),
        Span::styled(help, Style::default().fg(theme.muted)),
    ]);
    f.render_widget(Paragraph::new(text), area);
}
