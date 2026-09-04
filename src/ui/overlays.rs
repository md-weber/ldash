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
    let h = 36u16.min(area.height.saturating_sub(4));
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
        ("1/2/3/4/5  Tab/⇧Tab", "Switch tab"),
        ("↑k / ↓j", "Scroll / select row"),
        ("PgUp/PgDn  Home/End", "Page up/down · first/last"),
        ("←h / →l", "Month · NW range · chart range"),
        ("Enter", "Open Register / transaction legs"),
        ("/", "Register query bar"),
        ("", ""),
        ("§", "Tab-specific"),
        ("←h / →l", "Month  (Dashboard, Monthly)"),
        ("y / Y", "Year back / forward  (Monthly)"),
        ("G", "Current month  (Dashboard) · last entry  (Monthly)"),
        ("i", "Income/expense focus  (Monthly) · assets/liabilities  (Accounts)"),
        ("p", "Payee analytics  (Monthly)"),
        ("C", "YoY comparison  (Monthly)"),
        ("F", "Current-month forecast  (Monthly)"),
        ("h / l", "Previous / next month  (Register)"),
        ("gg / G", "First / last transaction  (Register)"),
        ("a–z  Backspace", "Filter · clear char  (Accounts)"),
        ("", ""),
        ("§", "Data & view"),
        ("Ctrl-O", "Open a different journal file"),
        ("Ctrl-S", "Save journal to config  (in Ctrl-O prompt)"),
        ("e", "Export view to file"),
        ("Y", "Copy view to clipboard  (non-Monthly)"),
        ("s", "Toggle chart stacked / unstacked"),
        ("c", "Toggle expense colors"),
        ("r", "Refresh data"),
        ("P", "Fetch prices from CoinGecko"),
        ("", ""),
        ("§", "General"),
        ("? / q", "Toggle help / quit"),
        ("Esc", "Close detail / back (Register via Enter returns to prior tab)"),
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

pub(super) fn render_file_prompt(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let picker = app.picker_journals();
    let has_config_journals = !app.config.journals.is_empty();

    // Extra rows: header + one per journal + blank line (or tip line when empty)
    let extra_rows = if picker.is_empty() {
        4u16 // blank + 2 tip lines + blank
    } else {
        let tip_lines = if !has_config_journals { 4u16 } else { 1u16 }; // blank + 2 tip + blank, or just blank
        picker.len() as u16 + 2 + tip_lines // header + entries + spacing
    };
    let base_h = 4u16;
    let total_h = (base_h + extra_rows).min(area.height.saturating_sub(4));
    let w = 72u16.min(area.width.saturating_sub(4));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(total_h)) / 2,
        width: w,
        height: total_h,
    };
    f.render_widget(Clear, popup);

    let hint = if !picker.is_empty() {
        "  [Tab] complete  [↑↓] select  [Enter] open  [^S] save to config  [Esc] cancel"
    } else {
        "  [Tab] complete  [Enter] open  [^S] save to config  [Esc] cancel"
    };

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("  Open: ", Style::default().fg(theme.gold).bold()),
            Span::styled(&app.file_prompt_path, Style::default().fg(theme.fg)),
            Span::styled("█", Style::default().fg(theme.accent)),
        ]),
        Line::from(""),
    ];

    if picker.is_empty() {
        // No session history and no configured journals yet
        lines.push(Line::from(Span::styled(
            "  Tip: add journals = [\"~/path/a.journal\", …]",
            Style::default().fg(theme.muted),
        )));
        lines.push(Line::from(Span::styled(
            "  to your config for quick switching",
            Style::default().fg(theme.muted),
        )));
        lines.push(Line::from(""));
    } else {
        // Determine where the session / config boundary falls
        let recent_count = app.recent_journals.len();

        lines.push(Line::from(Span::styled(
            "  Recent:",
            Style::default().fg(theme.muted),
        )));
        for (i, j) in picker.iter().enumerate() {
            let selected = app.file_prompt_journal_idx == Some(i);
            let is_config_only = i >= recent_count;

            let label_style = if selected {
                Style::default().fg(theme.accent).bold()
            } else if is_config_only {
                Style::default().fg(theme.gold)
            } else {
                Style::default().fg(theme.fg)
            };
            let prefix = if selected { "  ▶ " } else { "    " };

            // Append a small "(config)" badge at the start of config-only entries
            // and change the section label when we cross the boundary
            if is_config_only && i == recent_count {
                lines.push(Line::from(Span::styled(
                    "  Configured:",
                    Style::default().fg(theme.muted),
                )));
            }

            lines.push(Line::from(vec![
                Span::styled(prefix, label_style),
                Span::styled(j.as_str(), label_style),
            ]));
        }

        if !has_config_journals {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Tip: add journals = [\"…\"]",
                Style::default().fg(theme.muted),
            )));
            lines.push(Line::from(Span::styled(
                "  to your config to persist this list",
                Style::default().fg(theme.muted),
            )));
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(""));
        }
    }

    lines.push(Line::from(Span::styled(
        hint,
        Style::default().fg(theme.muted),
    )));

    let block = Block::default()
        .title(Span::styled(
            " Open Journal  (absolute path or ~/…) ",
            Style::default().fg(theme.accent).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .style(Style::default().bg(theme.background));

    f.render_widget(Paragraph::new(lines).block(block), popup);
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

    let help = "  [1-4] tab  [↑↓/jk] navigate  [PgUp/PgDn/Home/End] scroll  [←→/hl] month/range  [^O] open file  [r] refresh  [?] help  [q] quit";
    let text = Line::from(vec![
        Span::styled(&app.status_msg, Style::default().fg(theme.accent)),
        Span::styled(help, Style::default().fg(theme.muted)),
    ]);
    f.render_widget(Paragraph::new(text), area);
}
