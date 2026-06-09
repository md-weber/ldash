use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::app::{App, MonthlyFocus, Tab};
use crate::copy_to_clipboard;

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
    } else if app.search_active {
        handle_search_key(app, key)
    } else if app.show_alerts {
        app.show_alerts = false;
        app.alert_dismissed = true;
        Action::Continue
    } else if app.show_help {
        handle_help_key(app, key)
    } else if app.account_filter_active {
        handle_filter_key(app, key)
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

fn handle_search_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => app.close_search(),
        KeyCode::Enter => {
            // If results are showing and a row is selected → drill into account.
            // Otherwise execute the search.
            if !app.search_results.is_empty() && app.search_state.selected().is_some() {
                app.search_navigate_to_account();
            } else {
                app.execute_search();
            }
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            app.execute_search();
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            app.execute_search();
        }
        KeyCode::Up => app.search_scroll_up(),
        KeyCode::Down => app.search_scroll_down(),
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
        KeyCode::Enter => app.open_account_detail(),
        KeyCode::Char(c) => app.account_filter_push(c),
        _ => {}
    }
    Action::Continue
}

fn handle_normal_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('q') => return Action::Quit,
        KeyCode::Esc => {
            if app.account_detail.is_some() {
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
            } else {
                return Action::Quit;
            }
        }
        KeyCode::Enter => match app.tab {
            Tab::Accounts => app.open_account_detail(),
            Tab::Monthly => match app.monthly_focus {
                MonthlyFocus::Income => app.open_income_detail(),
                MonthlyFocus::Expenses => app.open_expense_detail(),
            },
            _ => {}
        },
        KeyCode::Char('i') if app.tab == Tab::Monthly => app.toggle_monthly_focus(),
        KeyCode::Char('p') if app.tab == Tab::Monthly => {
            app.payee_view = !app.payee_view;
        }
        KeyCode::Char('C') if app.tab == Tab::Monthly => {
            app.yoy_view = !app.yoy_view;
        }
        KeyCode::Char('G') if app.tab == Tab::Monthly => app.jump_to_last_entry(),
        KeyCode::Char('/') => app.open_search(),
        KeyCode::Char('?') => app.show_help = true,
        KeyCode::Tab => app.next_tab(),
        KeyCode::BackTab => app.prev_tab(),
        KeyCode::Char('1') => app.select_tab(0),
        KeyCode::Char('2') => app.select_tab(1),
        KeyCode::Char('3') => app.select_tab(2),
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
        },
        KeyCode::Right | KeyCode::Char('l') => match app.tab {
            Tab::Portfolio => app.portfolio_range_right(),
            Tab::Accounts => app.nw_range_right(),
            Tab::Monthly => app.month_right(),
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
        KeyCode::Char(c)
            if app.tab == Tab::Accounts
                && !matches!(
                    c,
                    'q' | '/' | '?' | 'r' | 's' | 'c' | 'y' | 'Y' | 'e' | '1' | '2' | '3'
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
