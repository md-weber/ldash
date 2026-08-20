use std::sync::mpsc;

use crate::data::{load_recent_transactions, AccountBalance};

use super::{App, DetailLoad};

/// Spawn a background hledger register call. Sends a `DetailLoad` over `tx`
/// when done so the UI thread keeps rendering while hledger runs.
///
/// Accounts/Monthly Enter now opens Register instead. The overlay loaders
/// stay so the existing detail UI can come back without a rewrite.
#[allow(dead_code)]
pub(super) fn spawn_detail_load(
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
            || self.register_detail.is_some()
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
            .or_else(|| self.register_detail.as_ref().map(|t| t.postings.len()))
            .unwrap_or(0);
        if i + 1 < len {
            self.detail_state.select(Some(i + 1));
        }
    }

    pub fn close_account_detail(&mut self) {
        self.account_detail = None;
        self.detail_account_name = None;
    }

    pub fn close_expense_detail(&mut self) {
        self.expense_detail = None;
        self.detail_expense_name = None;
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
}
