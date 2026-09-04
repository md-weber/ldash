use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::app::{App, Tab};
use crate::copy_to_clipboard;
use crate::data::RegisterQuery;

pub enum Action {
    Quit,
    Continue,
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> Action {
    if key.kind != KeyEventKind::Press {
        return Action::Continue;
    }

    if app.file_prompt_active {
        handle_file_prompt_key(app, key)
    } else if app.export_prompt_active {
        handle_export_prompt_key(app, key)
    } else if app.show_alerts {
        app.show_alerts = false;
        app.alert_dismissed = true;
        Action::Continue
    } else if app.show_help {
        handle_help_key(app, key)
    } else if app.account_filter_active {
        handle_filter_key(app, key)
    } else if app.tab == Tab::Register && app.register_focus_query {
        handle_register_query_key(app, key)
    } else {
        handle_normal_key(app, key)
    }
}

fn handle_file_prompt_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => app.cancel_file_prompt(),
        KeyCode::Enter => app.confirm_file_prompt(),
        KeyCode::Tab => app.file_prompt_tab_complete(),
        KeyCode::Down => app.file_prompt_next_journal(),
        KeyCode::BackTab | KeyCode::Up => app.file_prompt_prev_journal(),
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.save_journal_to_config();
        }
        KeyCode::Backspace => {
            app.file_prompt_path.pop();
            app.file_prompt_journal_idx = None;
        }
        KeyCode::Char(c) => {
            app.file_prompt_path.push(c);
            app.file_prompt_journal_idx = None;
        }
        _ => {}
    }
    Action::Continue
}

fn handle_export_prompt_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => app.cancel_export_prompt(),
        KeyCode::Enter => {
            let path = app.export_prompt_path.clone();
            app.export_prompt_active = false;
            match app.export_to_file(&path) {
                Ok(written) => app.status_msg = format!("Exported → {written}"),
                Err(e) => app.status_msg = format!("Export error: {e}"),
            }
        }
        KeyCode::Backspace => {
            app.export_prompt_path.pop();
        }
        KeyCode::Char(c) => app.export_prompt_path.push(c),
        _ => {}
    }
    Action::Continue
}

fn handle_register_query_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            app.register_focus_query = false;
        }
        KeyCode::Enter => app.submit_register_query(),
        KeyCode::Backspace => {
            app.register_draft.pop();
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.register_draft.push(c);
        }
        _ => {}
    }
    Action::Continue
}

fn handle_help_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc => {
            app.show_help = false;
        }
        _ => {}
    }
    Action::Continue
}

fn handle_filter_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            if app.account_detail.is_some() {
                app.close_account_detail();
            } else {
                app.close_account_filter();
            }
        }
        KeyCode::Backspace => app.account_filter_backspace(),
        KeyCode::Up | KeyCode::Char('k') => {
            if app.has_open_detail() {
                app.detail_scroll_up();
            } else {
                app.scroll_up();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.has_open_detail() {
                app.detail_scroll_down();
            } else {
                app.scroll_down();
            }
        }
        KeyCode::Enter => app.open_selected_account_in_register(),
        KeyCode::Char(c) => app.account_filter_push(c),
        _ => {}
    }
    Action::Continue
}

fn handle_normal_key(app: &mut App, key: KeyEvent) -> Action {
    if app.tab == Tab::Register && app.register_detail.is_none() {
        match key.code {
            KeyCode::Char('g') => {
                app.register_handle_g();
                return Action::Continue;
            }
            _ => app.register_clear_g_pending(),
        }
    }

    match key.code {
        KeyCode::Char('q') => return Action::Quit,
        KeyCode::Esc => {
            if app.register_detail.is_some() {
                app.close_register_detail();
            } else if app.account_detail.is_some() {
                app.close_account_detail();
            } else if app.income_detail.is_some() {
                app.close_income_detail();
            } else if app.expense_detail.is_some() {
                app.close_expense_detail();
            } else if app.liability_focus {
                app.liability_focus = false;
                app.liability_state.select(Some(0));
            } else if !app.account_filter.is_empty() {
                app.close_account_filter();
            } else if app.tab == Tab::Register && app.return_from_register() {
            } else {
                return Action::Quit;
            }
        }
        KeyCode::Enter => match app.tab {
            Tab::Accounts => app.open_selected_account_in_register(),
            Tab::Monthly => app.open_selected_category_in_register(),
            Tab::Register => app.open_register_row_detail(),
            Tab::Portfolio => {}
        },
        KeyCode::Char('i') if app.tab == Tab::Monthly => app.toggle_monthly_focus(),
        KeyCode::Char('i') if app.tab == Tab::Accounts => app.toggle_accounts_focus(),
        KeyCode::Char('p') if app.tab == Tab::Monthly => {
            app.payee_view = !app.payee_view;
        }
        KeyCode::Char('C') if app.tab == Tab::Monthly => {
            app.yoy_view = !app.yoy_view;
        }
        KeyCode::Char('F') if app.tab == Tab::Monthly => {
            app.show_current_month_forecast = !app.show_current_month_forecast;
            app.rebuild_combined_months();
        }
        KeyCode::Char('G') if app.tab == Tab::Monthly => app.jump_to_last_entry(),
        KeyCode::Char('G') if app.tab == Tab::Register => app.scroll_end(),
        KeyCode::Char('/') => {
            if app.tab == Tab::Register {
                app.register_focus_query = true;
            } else {
                let return_tab = app.tab;
                app.open_register(RegisterQuery::current_month(), true, Some(return_tab));
            }
        }
        KeyCode::Char('?') => app.show_help = true,
        KeyCode::Tab => app.next_tab(),
        KeyCode::BackTab => app.prev_tab(),
        KeyCode::Char('1') => app.select_tab(0),
        KeyCode::Char('2') => app.select_tab(1),
        KeyCode::Char('3') => app.select_tab(2),
        KeyCode::Char('4') => app.select_tab(3),
        KeyCode::Up | KeyCode::Char('k') => {
            if app.has_open_detail() {
                app.detail_scroll_up();
            } else {
                app.scroll_up();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.has_open_detail() {
                app.detail_scroll_down();
            } else {
                app.scroll_down();
            }
        }
        KeyCode::PageUp => app.scroll_page_up(),
        KeyCode::PageDown => app.scroll_page_down(),
        KeyCode::Home => app.scroll_home(),
        KeyCode::End => app.scroll_end(),
        KeyCode::Left | KeyCode::Char('h') => match app.tab {
            Tab::Portfolio => app.portfolio_range_left(),
            Tab::Accounts => app.nw_range_left(),
            Tab::Monthly => app.month_left(),
            Tab::Register => app.register_month_prev(),
        },
        KeyCode::Right | KeyCode::Char('l') => match app.tab {
            Tab::Portfolio => app.portfolio_range_right(),
            Tab::Accounts => app.nw_range_right(),
            Tab::Monthly => app.month_right(),
            Tab::Register => app.register_month_next(),
        },
        KeyCode::Char('y') if app.tab == Tab::Monthly => {
            app.cycle_year_back();
        }
        KeyCode::Char('Y') => {
            if app.tab == Tab::Monthly {
                app.cycle_year_forward();
            } else {
                let data = app.export_current_view();
                match copy_to_clipboard(&data) {
                    Ok(()) => app.status_msg = "Copied to clipboard".to_string(),
                    Err(e) => app.status_msg = format!("Clipboard error: {e}"),
                }
            }
        }
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.open_file_prompt();
        }
        KeyCode::Char('e') => app.open_export_prompt(),
        KeyCode::Char('s') => app.chart_mode = app.chart_mode.toggle(),
        KeyCode::Char('c') => app.expense_colors = !app.expense_colors,
        KeyCode::Char('r') => app.start_refresh(),
        KeyCode::Char('P') => app.start_price_fetch(),
        KeyCode::Char(c)
            if app.tab == Tab::Accounts
                && !matches!(
                    c,
                    'q' | '/'
                        | '?'
                        | 'r'
                        | 's'
                        | 'c'
                        | 'y'
                        | 'Y'
                        | 'e'
                        | 'P'
                        | 'i'
                        | '1'
                        | '2'
                        | '3'
                        | '4'
                ) =>
        {
            app.account_filter_active = true;
            app.account_filter.push(c);
            app.account_state.select(Some(0));
        }
        _ => {}
    }
    Action::Continue
}
