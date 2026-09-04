use ratatui::{prelude::*, widgets::*};

use super::Theme;
use crate::app::App;

pub(super) fn render_register(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    if let Some(txn) = app.register_detail.clone() {
        render_txn_detail(f, app, &txn, area, theme);
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(area);

    render_query_bar(f, app, chunks[0], theme);
    render_month_label(f, app, chunks[1], theme);
    render_register_table(f, app, chunks[2], theme);
}

fn render_query_bar(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let border = if app.register_focus_query {
        theme.accent
    } else {
        theme.muted
    };
    let cursor = if app.register_focus_query { "█" } else { "" };
    let input = Paragraph::new(Line::from(vec![
        Span::styled("  / ", Style::default().fg(theme.gold).bold()),
        Span::styled(&app.register_draft, Style::default().fg(theme.fg)),
        Span::styled(cursor, Style::default().fg(theme.accent)),
    ]))
    .block(
        Block::default()
            .title(Span::styled(
                " Query  [Enter apply]  [Esc rows] ",
                Style::default().fg(theme.accent).bold(),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border)),
    );
    f.render_widget(input, area);
}

fn render_month_label(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let period = app.register_query.period_label();
    let account = app
        .register_query
        .account
        .as_deref()
        .map(|a| format!("  ·  {a}"))
        .unwrap_or_default();
    let desc = app
        .register_query
        .description
        .as_deref()
        .map(|d| format!("  ·  desc:{d}"))
        .unwrap_or_default();
    let text = Line::from(vec![
        Span::styled(
            format!("  {period}"),
            Style::default().fg(theme.gold).bold(),
        ),
        Span::styled(account, Style::default().fg(theme.fg)),
        Span::styled(desc, Style::default().fg(theme.muted)),
        Span::styled(
            "  [h/l month]  [gg/G top/bottom]  [Enter txn]",
            Style::default().fg(theme.muted),
        ),
    ]);
    f.render_widget(Paragraph::new(text), area);
}

fn render_register_table(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    app.geometry.table_area = area;

    if let Some(err) = app.register_error.as_ref() {
        render_message(f, area, theme, " Register ", &format!("Error: {err}"));
        return;
    }

    if app.register_rx.is_some() && app.register_rows.is_empty() {
        render_message(f, area, theme, " Register ", "Loading register…");
        return;
    }

    if app.register_rows.is_empty() {
        render_message(f, area, theme, " Register ", "No transactions");
        return;
    }

    let txn_count = app.register_rows.iter().filter(|r| !r.continuation).count();
    let selected_txn = app
        .register_table
        .selected()
        .and_then(|i| app.register_rows.get(i).map(|r| r.txnidx));

    let rows: Vec<Row> = app
        .register_rows
        .iter()
        .map(|p| {
            let amt_color = if p.amount >= 0.0 {
                theme.positive
            } else {
                theme.negative
            };
            let date = if p.continuation {
                String::new()
            } else {
                p.date.format("%Y-%m-%d").to_string()
            };
            let status = if p.continuation {
                String::new()
            } else {
                p.status.as_cell().to_string()
            };
            let mut row = Row::new(vec![
                Cell::from(date).style(Style::default().fg(theme.muted)),
                Cell::from(status).style(Style::default().fg(theme.gold)),
                Cell::from(p.description.clone()).style(Style::default().fg(theme.fg)),
                Cell::from(p.account.clone()).style(Style::default().fg(theme.muted)),
                Cell::from(format!("{:>13}", app.config.fmt_amount(p.amount, 2)))
                    .style(Style::default().fg(amt_color)),
                Cell::from(format!("{:>13}", app.config.fmt_amount(p.running_total, 2)))
                    .style(Style::default().fg(theme.accent)),
            ]);
            if p.continuation && selected_txn == Some(p.txnidx) {
                row = row.style(Style::default().bg(theme.highlight_bg));
            }
            row
        })
        .collect();

    let title = format!(" Register  {txn_count} txns ");
    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(3),
            Constraint::Min(16),
            Constraint::Min(16),
            Constraint::Length(14),
            Constraint::Length(14),
        ],
    )
    .header(
        Row::new(vec![
            "Date",
            "St",
            "Description",
            "Account",
            "Amount",
            "Total",
        ])
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
            .border_style(Style::default().fg(theme.accent)),
    );

    f.render_stateful_widget(table, area, &mut app.register_table);
}

fn render_txn_detail(
    f: &mut Frame,
    app: &mut App,
    txn: &crate::data::RegisterTxn,
    area: Rect,
    theme: &Theme,
) {
    let status = match txn.status {
        crate::data::TxnStatus::Cleared => "*",
        crate::data::TxnStatus::Pending => "!",
        crate::data::TxnStatus::Unmarked => " ",
    };
    let title = format!(
        " {}  {}  {}  [Esc close] ",
        txn.date.format("%Y-%m-%d"),
        status,
        txn.description
    );

    let rows: Vec<Row> = txn
        .postings
        .iter()
        .map(|p| {
            let amt_color = if p.amount >= 0.0 {
                theme.positive
            } else {
                theme.negative
            };
            Row::new(vec![
                Cell::from(p.account.clone()).style(Style::default().fg(theme.fg)),
                Cell::from(format!("{:>13}", app.config.fmt_amount(p.amount, 2)))
                    .style(Style::default().fg(amt_color)),
            ])
        })
        .collect();

    let table = Table::new(rows, [Constraint::Min(24), Constraint::Length(14)])
        .header(
            Row::new(vec!["Account", "Amount"])
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
                .border_style(Style::default().fg(theme.accent)),
        );

    f.render_stateful_widget(table, area, &mut app.detail_state);
}

fn render_message(f: &mut Frame, area: Rect, theme: &Theme, title: &str, msg: &str) {
    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(theme.gold).bold()))
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
}
