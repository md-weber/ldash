use ratatui::{prelude::*, widgets::*};

use super::{render_detail_with_title, Theme};
use crate::app::App;

mod chart;
mod expenses;
mod forecast;
mod income;
mod payee;
mod sparkline;
mod summary;
mod yoy;

/// BarChart bar width (chars per bar). Shared between render and mouse hit-test.
pub(crate) const MONTHLY_BAR_WIDTH: u16 = 3;
/// BarChart bar gap (chars between two bars within a group). Shared.
pub(crate) const MONTHLY_BAR_GAP: u16 = 0;
/// BarChart group gap (chars between adjacent month groups). Shared.
pub(crate) const MONTHLY_GROUP_GAP: u16 = 2;
/// Number of bars per month group (income + expense).
pub(crate) const MONTHLY_BARS_PER_GROUP: u16 = 2;
/// Total horizontal cells per month group; `on_monthly_chart_click` divides
/// the click x by this to map a column back to a month index. MUST stay in
/// sync with the `BarChart` builder below.
pub(crate) const MONTHLY_GROUP_WIDTH: u16 =
    MONTHLY_BARS_PER_GROUP * MONTHLY_BAR_WIDTH + MONTHLY_BAR_GAP + MONTHLY_GROUP_GAP;

fn render_monthly_no_data(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let exp = &app.config.expenses_account;
    let inc = &app.config.income_account;

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "No income or expense data found",
            Style::default().fg(theme.accent).bold(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Looking for accounts under  \"{inc}\"  and  \"{exp}\""),
            Style::default().fg(theme.fg),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "To fix this, set the matching top-level account names in your config:",
            Style::default().fg(theme.muted),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "    ~/.config/ldash/config.toml",
            Style::default().fg(theme.accent),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "    income_account   = \"revenues\"   # or whatever your journal uses",
            Style::default().fg(theme.muted),
        )),
        Line::from(Span::styled(
            "    expenses_account = \"expenses\"",
            Style::default().fg(theme.muted),
        )),
    ];

    let block = Block::default()
        .title(Span::styled(
            " Monthly ",
            Style::default().fg(theme.accent).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.muted));

    let inner = block.inner(area);
    f.render_widget(block, area);
    let v = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(lines.len() as u16),
        Constraint::Min(0),
    ])
    .split(inner);
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), v[1]);
}

pub(super) fn render_monthly(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    // Data finished loading but nothing came back — the journal likely has no
    // income/expense accounts under the configured prefixes.  Show a
    // configuration hint instead of a blank screen.
    if app.tabs_loaded.monthly && !app.loading && app.combined_months.is_empty() {
        render_monthly_no_data(f, app, area, theme);
        return;
    }

    let narrow = area.width < 100;
    let very_narrow = area.width < 80;

    // Common 3-chunk vertical layout shared by both modes.
    let chunks = Layout::vertical([
        Constraint::Length(12),
        Constraint::Length(if narrow { 20 } else { 14 }),
        Constraint::Min(0),
    ])
    .split(area);

    // Top row: main bar chart. Forecast appears side-by-side on wide screens
    // for the current year, regardless of YoY mode.
    let show_forecast = area.width >= 115 && app.monthly_year_offset == 0;
    if show_forecast {
        let top = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(chunks[0]);
        app.geometry.monthly_chart_area = top[0];
        chart::render_monthly_chart(f, app, top[0], theme);
        forecast::render_forecast_chart(f, app, top[1], theme);
    } else {
        app.geometry.monthly_chart_area = chunks[0];
        chart::render_monthly_chart(f, app, chunks[0], theme);
    }

    summary::render_monthly_summary(f, app, chunks[1], theme);

    // Bottom row: YoY comparison replaces income/expense panels when active.
    if app.yoy_view {
        yoy::render_yoy_comparison(f, app, chunks[2], theme);
        return;
    }

    // Payee analytics replaces the full income+expense area when active.
    if app.payee_view {
        payee::render_payee_analytics(f, app, chunks[2], theme);
        return;
    }

    let detail_chunks = if very_narrow {
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).split(chunks[2])
    } else {
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2])
    };

    if let (Some(txns), Some(name)) = (&app.income_detail.clone(), &app.detail_income_name.clone())
    {
        let short = app
            .config
            .strip_account_prefix(name, &app.config.income_account.clone());
        let month = app
            .current_month()
            .map(|m| m.month_name.as_str())
            .unwrap_or("");
        let title = format!(" {} — {}  [Esc back] ", short, month);
        render_detail_with_title(
            f,
            txns,
            &title,
            &app.config,
            detail_chunks[0],
            theme,
            &mut app.detail_state,
        );
    } else {
        income::render_monthly_income(f, app, detail_chunks[0], theme);
    }

    if let (Some(txns), Some(name)) = (
        &app.expense_detail.clone(),
        &app.detail_expense_name.clone(),
    ) {
        let short = app
            .config
            .strip_account_prefix(name, &app.config.expenses_account.clone());
        let month = app
            .current_month()
            .map(|m| m.month_name.as_str())
            .unwrap_or("");
        let title = format!(" {} — {}  [Esc back] ", short, month);
        render_detail_with_title(
            f,
            txns,
            &title,
            &app.config,
            detail_chunks[1],
            theme,
            &mut app.detail_state,
        );
    } else {
        expenses::render_monthly_expenses(f, app, detail_chunks[1], theme);
    }
}
