use std::sync::mpsc;

use ratatui::widgets::TableState;

use crate::data::{self, load_recent_transactions, AccountBalance};

use super::{month_name_to_period, App, DetailLoad, Tab};

/// Spawn a background hledger register call. Sends a `DetailLoad` over `tx`
/// when done. Used by `open_account_detail`, `open_expense_detail` and
/// `open_income_detail` so the UI thread keeps rendering (loading spinner
/// stays visible) while hledger churns.
fn spawn_detail_load(
    tx: mpsc::Sender<DetailLoad>,
    journal_path: std::path::PathBuf,
    name: String,
    n: usize,
    period: Option<String>,
    currency_symbol: String,
) {
    std::thread::spawn(move || {
        let result =
            load_recent_transactions(&journal_path, &name, n, period.as_deref(), &currency_symbol)
                .map_err(|e| e.to_string());
        let _ = tx.send(DetailLoad { name, result });
    });
}

impl App {
    pub fn has_open_detail(&self) -> bool {
        self.account_detail.is_some()
            || self.expense_detail.is_some()
            || self.income_detail.is_some()
    }

    pub fn detail_scroll_up(&mut self) {
        let i = self.detail_state.selected().unwrap_or(0);
        self.detail_state.select(Some(i.saturating_sub(1)));
    }

    pub fn detail_scroll_down(&mut self) {
        let i = self.detail_state.selected().unwrap_or(0);
        let len = self
            .account_detail
            .as_ref()
            .or(self.expense_detail.as_ref())
            .or(self.income_detail.as_ref())
            .map(|d| d.len())
            .unwrap_or(0);
        if i + 1 < len {
            self.detail_state.select(Some(i + 1));
        }
    }

    pub fn open_account_detail(&mut self) {
        if self.tab != Tab::Accounts
            || self.account_detail.is_some()
            || self.account_detail_rx.is_some()
        {
            return;
        }
        let sel = self.account_state.selected().unwrap_or(0);
        let account = match self.filtered_accounts().get(sel).map(|b| b.account.clone()) {
            Some(a) => a,
            None => return,
        };
        let (tx, rx) = mpsc::channel();
        self.account_detail_rx = Some(rx);
        self.status_msg = format!("Loading {account}…");
        spawn_detail_load(
            tx,
            self.journal_path.clone(),
            account,
            30,
            None,
            self.config.currency_symbol.clone(),
        );
    }

    pub fn close_account_detail(&mut self) {
        self.account_detail = None;
        self.detail_account_name = None;
    }

    pub fn open_expense_detail(&mut self) {
        if self.tab != Tab::Monthly
            || self.expense_detail.is_some()
            || self.expense_detail_rx.is_some()
            || self.current_month_is_forecast()
        {
            return;
        }
        let sel = self.expense_state.selected().unwrap_or(0);
        let (category, period) = match self.current_month() {
            Some(m) => match m.expenses.get(sel) {
                Some((cat, _)) => (
                    cat.clone(),
                    month_name_to_period(&m.month_name, self.displayed_year()),
                ),
                None => return,
            },
            None => return,
        };
        let (tx, rx) = mpsc::channel();
        self.expense_detail_rx = Some(rx);
        self.status_msg = format!("Loading {category}…");
        spawn_detail_load(
            tx,
            self.journal_path.clone(),
            category,
            50,
            Some(period),
            self.config.currency_symbol.clone(),
        );
    }

    pub fn close_expense_detail(&mut self) {
        self.expense_detail = None;
        self.detail_expense_name = None;
    }

    pub fn open_income_detail(&mut self) {
        if self.tab != Tab::Monthly
            || self.income_detail.is_some()
            || self.income_detail_rx.is_some()
            || self.current_month_is_forecast()
        {
            return;
        }
        let sel = self.income_state.selected().unwrap_or(0);
        let (category, period) = match self.current_month() {
            Some(m) => match m.income.get(sel) {
                Some((cat, _)) => (
                    cat.clone(),
                    month_name_to_period(&m.month_name, self.displayed_year()),
                ),
                None => return,
            },
            None => return,
        };
        let (tx, rx) = mpsc::channel();
        self.income_detail_rx = Some(rx);
        self.status_msg = format!("Loading {category}…");
        spawn_detail_load(
            tx,
            self.journal_path.clone(),
            category,
            50,
            Some(period),
            self.config.currency_symbol.clone(),
        );
    }

    pub fn close_income_detail(&mut self) {
        self.income_detail = None;
        self.detail_income_name = None;
    }

    pub fn filtered_accounts(&self) -> Vec<&AccountBalance> {
        if self.account_filter.is_empty() {
            return self.account_balances.iter().collect();
        }
        let q = self.account_filter.to_lowercase();
        self.account_balances
            .iter()
            .filter(|b| b.account.to_lowercase().contains(&q))
            .collect()
    }

    pub fn account_filter_push(&mut self, c: char) {
        self.account_filter.push(c);
        self.account_state.select(Some(0));
    }

    pub fn account_filter_backspace(&mut self) {
        self.account_filter.pop();
        self.account_state.select(Some(0));
    }

    pub fn close_account_filter(&mut self) {
        self.account_filter.clear();
        self.account_filter_active = false;
        self.account_state.select(Some(0));
    }

    pub fn open_search(&mut self) {
        self.search_active = true;
        self.search_query.clear();
        self.search_results.clear();
        self.search_state = TableState::default();
    }

    pub fn close_search(&mut self) {
        self.search_active = false;
        self.search_query.clear();
        self.search_results.clear();
    }

    pub fn execute_search(&mut self) {
        if self.search_query.is_empty() {
            self.search_results.clear();
            return;
        }
        match data::search_transactions(&self.journal_path, &self.search_query) {
            Ok(txns) => {
                self.search_results = txns;
                if !self.search_results.is_empty() {
                    self.search_state = TableState::default().with_selected(0);
                }
            }
            Err(e) => {
                self.status_msg = format!("Search error: {e}");
            }
        }
    }

    pub fn search_scroll_up(&mut self) {
        let i = self.search_state.selected().unwrap_or(0);
        self.search_state.select(Some(i.saturating_sub(1)));
    }

    pub fn search_scroll_down(&mut self) {
        let i = self.search_state.selected().unwrap_or(0);
        if i + 1 < self.search_results.len() {
            self.search_state.select(Some(i + 1));
        }
    }
}
