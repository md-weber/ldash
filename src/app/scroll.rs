use ratatui::layout::Rect;

use super::{App, MonthlyFocus, Tab};

/// Identifies which scrollable list is currently in focus, taking the
/// monthly tab's income/expenses sub-focus into account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedList {
    Portfolio,
    Accounts,
    AccountsLiabilities,
    MonthlyIncome,
    MonthlyExpenses,
    MonthlyPayee,
    Register,
}

impl App {
    fn focused_list(&self) -> FocusedList {
        match self.tab {
            Tab::Portfolio => FocusedList::Portfolio,
            Tab::Accounts => {
                if self.liability_focus {
                    FocusedList::AccountsLiabilities
                } else {
                    FocusedList::Accounts
                }
            }
            Tab::Monthly => {
                if self.payee_view {
                    FocusedList::MonthlyPayee
                } else {
                    match self.monthly_focus {
                        MonthlyFocus::Income => FocusedList::MonthlyIncome,
                        MonthlyFocus::Expenses => FocusedList::MonthlyExpenses,
                    }
                }
            }
            Tab::Register => FocusedList::Register,
        }
    }

    fn current_selected(&self) -> usize {
        match self.focused_list() {
            FocusedList::Portfolio => self.selected_holding,
            FocusedList::Accounts => self.account_state.selected().unwrap_or(0),
            FocusedList::AccountsLiabilities => self.liability_state.selected().unwrap_or(0),
            FocusedList::MonthlyIncome => self.income_state.selected().unwrap_or(0),
            FocusedList::MonthlyExpenses => self.expense_state.selected().unwrap_or(0),
            FocusedList::MonthlyPayee => self.payee_state.selected().unwrap_or(0),
            FocusedList::Register => self.register_table.selected().unwrap_or(0),
        }
    }

    fn current_len(&self) -> usize {
        match self.focused_list() {
            FocusedList::Portfolio => self.holdings.len(),
            FocusedList::Accounts => self.filtered_accounts().len(),
            FocusedList::AccountsLiabilities => self.liabilities.len(),
            FocusedList::MonthlyIncome => self.current_month().map(|m| m.income.len()).unwrap_or(0),
            FocusedList::MonthlyExpenses => {
                self.current_month().map(|m| m.expenses.len()).unwrap_or(0)
            }
            FocusedList::MonthlyPayee => self.payee_data.len(),
            FocusedList::Register => self.register_rows.len(),
        }
    }

    fn set_current_selected(&mut self, i: usize) {
        match self.focused_list() {
            FocusedList::Portfolio => self.selected_holding = i,
            FocusedList::Accounts => self.account_state.select(Some(i)),
            FocusedList::AccountsLiabilities => self.liability_state.select(Some(i)),
            FocusedList::MonthlyIncome => self.income_state.select(Some(i)),
            FocusedList::MonthlyExpenses => self.expense_state.select(Some(i)),
            FocusedList::MonthlyPayee => self.payee_state.select(Some(i)),
            FocusedList::Register => self.register_table.select(Some(i)),
        }
    }

    /// Visible data rows in the current focused list's table area: subtract
    /// borders (2) + header (1) + header margin (1) = 4.
    fn current_page_size(&self) -> usize {
        let area = match self.focused_list() {
            FocusedList::Portfolio | FocusedList::Accounts | FocusedList::Register => {
                self.geometry.table_area
            }
            FocusedList::AccountsLiabilities => self.geometry.liability_table_area,
            FocusedList::MonthlyIncome => self.geometry.income_table_area,
            FocusedList::MonthlyExpenses => self.geometry.expense_table_area,
            FocusedList::MonthlyPayee => self.geometry.payee_table_area,
        };
        page_size_for(area).max(1)
    }

    pub fn scroll_up(&mut self) {
        // When at the first liability, pressing up transfers focus back to assets.
        if self.tab == Tab::Accounts && self.liability_focus {
            let i = self.liability_state.selected().unwrap_or(0);
            if i == 0 {
                self.liability_focus = false;
                return;
            }
            self.liability_state.select(Some(i - 1));
            return;
        }
        if self.tab == Tab::Register {
            self.register_select_prev_txn();
            return;
        }
        let i = self.current_selected();
        self.set_current_selected(i.saturating_sub(1));
    }

    pub fn scroll_down(&mut self) {
        if self.tab == Tab::Register {
            self.register_select_next_txn();
            return;
        }
        let i = self.current_selected();
        let len = self.current_len();
        if i + 1 < len {
            self.set_current_selected(i + 1);
        } else if self.tab == Tab::Accounts && !self.liability_focus && !self.liabilities.is_empty()
        {
            // Transfer focus to the liabilities table when scrolling past the
            // last asset row.
            self.liability_focus = true;
            self.liability_state.select(Some(0));
        }
    }

    pub fn scroll_page_up(&mut self) {
        if self.tab == Tab::Register {
            let n = self.current_page_size();
            let i = self.register_table.selected().unwrap_or(0);
            self.register_select_near(i.saturating_sub(n), false);
            return;
        }
        let n = self.current_page_size();
        let i = self.current_selected();
        self.set_current_selected(i.saturating_sub(n));
    }

    pub fn scroll_page_down(&mut self) {
        if self.tab == Tab::Register {
            let n = self.current_page_size();
            let i = self.register_table.selected().unwrap_or(0);
            let len = self.register_rows.len();
            let target = (i + n).min(len.saturating_sub(1));
            self.register_select_near(target, true);
            return;
        }
        let n = self.current_page_size();
        let i = self.current_selected();
        let len = self.current_len();
        let new = (i + n).min(len.saturating_sub(1));
        self.set_current_selected(new);
    }

    pub fn scroll_home(&mut self) {
        self.set_current_selected(0);
    }

    pub fn scroll_end(&mut self) {
        if self.tab == Tab::Register {
            self.register_select_last_txn();
            return;
        }
        let len = self.current_len();
        self.set_current_selected(len.saturating_sub(1));
    }
}

fn page_size_for(area: Rect) -> usize {
    (area.height.saturating_sub(4)) as usize
}
