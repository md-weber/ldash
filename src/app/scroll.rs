use ratatui::layout::Rect;

use super::{App, MonthlyFocus, Tab};

impl App {
    pub fn scroll_up(&mut self) {
        match self.tab {
            Tab::Portfolio => {
                self.selected_holding = self.selected_holding.saturating_sub(1);
            }
            Tab::Accounts => {
                let i = self.account_state.selected().unwrap_or(0);
                self.account_state.select(Some(i.saturating_sub(1)));
            }
            Tab::Monthly => match self.monthly_focus {
                MonthlyFocus::Income => {
                    let i = self.income_state.selected().unwrap_or(0);
                    self.income_state.select(Some(i.saturating_sub(1)));
                }
                MonthlyFocus::Expenses => {
                    let i = self.expense_state.selected().unwrap_or(0);
                    self.expense_state.select(Some(i.saturating_sub(1)));
                }
            },
        }
    }

    pub fn scroll_down(&mut self) {
        match self.tab {
            Tab::Portfolio => {
                if self.selected_holding + 1 < self.holdings.len() {
                    self.selected_holding += 1;
                }
            }
            Tab::Accounts => {
                let i = self.account_state.selected().unwrap_or(0);
                let len = self.filtered_accounts().len();
                if i + 1 < len {
                    self.account_state.select(Some(i + 1));
                }
            }
            Tab::Monthly => match self.monthly_focus {
                MonthlyFocus::Income => {
                    let i = self.income_state.selected().unwrap_or(0);
                    let len = self.current_month().map(|m| m.income.len()).unwrap_or(0);
                    if i + 1 < len {
                        self.income_state.select(Some(i + 1));
                    }
                }
                MonthlyFocus::Expenses => {
                    let i = self.expense_state.selected().unwrap_or(0);
                    let len = self.current_month().map(|m| m.expenses.len()).unwrap_or(0);
                    if i + 1 < len {
                        self.expense_state.select(Some(i + 1));
                    }
                }
            },
        }
    }

    /// Visible data rows in a table area: subtract borders (2) + header (1) + header margin (1).
    fn page_size_for(&self, area: Rect) -> usize {
        (area.height.saturating_sub(4)) as usize
    }

    pub fn scroll_page_up(&mut self) {
        match self.tab {
            Tab::Portfolio => {
                let n = self.page_size_for(self.table_area).max(1);
                self.selected_holding = self.selected_holding.saturating_sub(n);
            }
            Tab::Accounts => {
                let n = self.page_size_for(self.table_area).max(1);
                let i = self.account_state.selected().unwrap_or(0);
                self.account_state.select(Some(i.saturating_sub(n)));
            }
            Tab::Monthly => match self.monthly_focus {
                MonthlyFocus::Income => {
                    let n = self.page_size_for(self.income_table_area).max(1);
                    let i = self.income_state.selected().unwrap_or(0);
                    self.income_state.select(Some(i.saturating_sub(n)));
                }
                MonthlyFocus::Expenses => {
                    let n = self.page_size_for(self.expense_table_area).max(1);
                    let i = self.expense_state.selected().unwrap_or(0);
                    self.expense_state.select(Some(i.saturating_sub(n)));
                }
            },
        }
    }

    pub fn scroll_page_down(&mut self) {
        match self.tab {
            Tab::Portfolio => {
                let n = self.page_size_for(self.table_area).max(1);
                let new = (self.selected_holding + n).min(self.holdings.len().saturating_sub(1));
                self.selected_holding = new;
            }
            Tab::Accounts => {
                let n = self.page_size_for(self.table_area).max(1);
                let i = self.account_state.selected().unwrap_or(0);
                let new = (i + n).min(self.filtered_accounts().len().saturating_sub(1));
                self.account_state.select(Some(new));
            }
            Tab::Monthly => match self.monthly_focus {
                MonthlyFocus::Income => {
                    let n = self.page_size_for(self.income_table_area).max(1);
                    let i = self.income_state.selected().unwrap_or(0);
                    let len = self.current_month().map(|m| m.income.len()).unwrap_or(0);
                    self.income_state
                        .select(Some((i + n).min(len.saturating_sub(1))));
                }
                MonthlyFocus::Expenses => {
                    let n = self.page_size_for(self.expense_table_area).max(1);
                    let i = self.expense_state.selected().unwrap_or(0);
                    let len = self.current_month().map(|m| m.expenses.len()).unwrap_or(0);
                    self.expense_state
                        .select(Some((i + n).min(len.saturating_sub(1))));
                }
            },
        }
    }

    pub fn scroll_home(&mut self) {
        match self.tab {
            Tab::Portfolio => self.selected_holding = 0,
            Tab::Accounts => self.account_state.select(Some(0)),
            Tab::Monthly => match self.monthly_focus {
                MonthlyFocus::Income => self.income_state.select(Some(0)),
                MonthlyFocus::Expenses => self.expense_state.select(Some(0)),
            },
        }
    }

    pub fn scroll_end(&mut self) {
        match self.tab {
            Tab::Portfolio => {
                self.selected_holding = self.holdings.len().saturating_sub(1);
            }
            Tab::Accounts => {
                self.account_state
                    .select(Some(self.filtered_accounts().len().saturating_sub(1)));
            }
            Tab::Monthly => match self.monthly_focus {
                MonthlyFocus::Income => {
                    let len = self.current_month().map(|m| m.income.len()).unwrap_or(0);
                    self.income_state.select(Some(len.saturating_sub(1)));
                }
                MonthlyFocus::Expenses => {
                    let len = self.current_month().map(|m| m.expenses.len()).unwrap_or(0);
                    self.expense_state.select(Some(len.saturating_sub(1)));
                }
            },
        }
    }
}
