use std::sync::mpsc;

use chrono::{Datelike, Local};
use ratatui::widgets::TableState;

use crate::data::{build_register_view, load_register_page, RegisterQuery};

use super::{month_name_to_period, App, MonthlyFocus, RegisterLoad, Tab};

fn spawn_register_load(
    tx: mpsc::Sender<RegisterLoad>,
    journal_path: std::path::PathBuf,
    query: RegisterQuery,
    currency_symbol: String,
) {
    std::thread::spawn(move || {
        let result =
            load_register_page(&journal_path, &query, &currency_symbol).map_err(|e| e.to_string());
        let _ = tx.send(RegisterLoad { query, result });
    });
}

impl App {
    pub fn open_register(&mut self, query: RegisterQuery, focus_query: bool) {
        self.register_draft = query.description.clone().unwrap_or_default();
        let same_period = self.register_loaded_period == Some((query.year, query.month));
        self.register_query = query;
        self.register_focus_query = focus_query;
        self.register_detail = None;
        self.tab = Tab::Register;
        if same_period && self.register_error.is_none() {
            self.rebuild_register_view();
        } else {
            self.register_error = None;
            self.spawn_register_load();
        }
    }

    pub fn reload_register(&mut self) {
        self.register_detail = None;
        self.spawn_register_load();
    }

    pub(super) fn ensure_register_loaded(&mut self) {
        if self.register_rx.is_some() {
            return;
        }
        if self.register_loaded_period
            == Some((self.register_query.year, self.register_query.month))
        {
            return;
        }
        self.spawn_register_load();
    }

    pub(super) fn register_already_loaded(&self) -> bool {
        self.register_loaded_period.is_some()
    }

    fn spawn_register_load(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.register_rx = Some(rx);
        spawn_register_load(
            tx,
            self.journal_path.clone(),
            self.register_query.clone(),
            self.config.currency_symbol.clone(),
        );
    }

    pub(super) fn check_register_load(&mut self) {
        let load = match self.register_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            Some(load) => load,
            None => return,
        };
        self.register_rx = None;
        if load.query.year != self.register_query.year
            || load.query.month != self.register_query.month
        {
            return;
        }
        match load.result {
            Ok(txns) => {
                self.register_txns = txns;
                self.register_error = None;
                self.register_loaded_period =
                    Some((self.register_query.year, self.register_query.month));
                self.rebuild_register_view();
            }
            Err(e) => {
                self.register_error = Some(e);
                self.register_loaded_period = None;
            }
        }
    }

    pub fn rebuild_register_view(&mut self) {
        self.register_rows = build_register_view(
            &self.register_txns,
            self.register_query.account.as_deref(),
            self.register_query.description.as_deref(),
        );
        self.register_table =
            TableState::default().with_selected(if self.register_rows.is_empty() {
                None
            } else {
                Some(0)
            });
    }

    pub fn submit_register_query(&mut self) {
        let desc = self.register_draft.trim();
        self.register_query.description = if desc.is_empty() {
            None
        } else {
            Some(desc.to_string())
        };
        self.register_focus_query = false;
        self.rebuild_register_view();
    }

    pub fn register_month_prev(&mut self) {
        self.register_query.prev_month();
        self.reload_register();
    }

    pub fn register_month_next(&mut self) {
        self.register_query.next_month();
        self.reload_register();
    }

    pub fn open_selected_account_in_register(&mut self) {
        let name = if self.liability_focus {
            let sel = self.liability_state.selected().unwrap_or(0);
            self.liabilities.get(sel).map(|b| b.account.clone())
        } else {
            let sel = self.account_state.selected().unwrap_or(0);
            self.filtered_accounts().get(sel).map(|b| b.account.clone())
        };
        let Some(name) = name else {
            return;
        };
        let today = Local::now().date_naive();
        self.open_register(
            RegisterQuery {
                year: today.year(),
                month: today.month() as u8,
                account: Some(name),
                description: None,
            },
            false,
        );
    }

    pub fn open_selected_category_in_register(&mut self) {
        if self.current_month_is_forecast() {
            return;
        }
        let Some(m) = self.current_month() else {
            return;
        };
        let sel = match self.monthly_focus {
            MonthlyFocus::Income => self.income_state.selected().unwrap_or(0),
            MonthlyFocus::Expenses => self.expense_state.selected().unwrap_or(0),
        };
        let month_name = m.month_name.clone();
        let cat = match self.monthly_focus {
            MonthlyFocus::Income => m.income.get(sel).map(|(c, _)| c.clone()),
            MonthlyFocus::Expenses => m.expenses.get(sel).map(|(c, _)| c.clone()),
        };
        let Some(cat) = cat else {
            return;
        };
        let year = self.displayed_year();
        let period = month_name_to_period(&month_name, year);
        let month = period
            .split('-')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        self.open_register(
            RegisterQuery {
                year,
                month,
                account: Some(cat),
                description: None,
            },
            false,
        );
    }

    pub fn open_register_row_detail(&mut self) {
        if self.register_detail.is_some() {
            return;
        }
        let Some(sel) = self.register_table.selected() else {
            return;
        };
        let Some(row) = self.register_rows.get(sel) else {
            return;
        };
        let Some(txn) = self
            .register_txns
            .iter()
            .find(|t| t.txnidx == row.txnidx)
            .cloned()
        else {
            return;
        };
        self.detail_state = TableState::default().with_selected(Some(0));
        self.register_detail = Some(txn);
    }

    pub fn close_register_detail(&mut self) {
        self.register_detail = None;
    }

    fn register_header_at(&self, idx: usize) -> Option<usize> {
        if self.register_rows.is_empty() {
            return None;
        }
        let mut i = idx.min(self.register_rows.len() - 1);
        while i > 0 && self.register_rows[i].continuation {
            i -= 1;
        }
        Some(i)
    }

    pub(super) fn register_select_txn_at(&mut self, idx: usize) {
        if let Some(header) = self.register_header_at(idx) {
            self.register_table.select(Some(header));
        }
    }

    pub(super) fn register_select_prev_txn(&mut self) {
        let i = self.register_table.selected().unwrap_or(0);
        if let Some(j) = (0..i).rev().find(|&j| !self.register_rows[j].continuation) {
            self.register_table.select(Some(j));
        }
    }

    pub(super) fn register_select_next_txn(&mut self) {
        let i = self.register_table.selected().unwrap_or(0);
        if let Some((j, _)) = self
            .register_rows
            .iter()
            .enumerate()
            .skip(i + 1)
            .find(|(_, r)| !r.continuation)
        {
            self.register_table.select(Some(j));
        }
    }

    pub(super) fn register_select_near(&mut self, idx: usize, forward: bool) {
        let Some(header) = self.register_header_at(idx) else {
            return;
        };
        if self.register_table.selected() == Some(header) {
            if forward {
                self.register_select_next_txn();
            } else {
                self.register_select_prev_txn();
            }
            return;
        }
        self.register_table.select(Some(header));
    }

    pub(super) fn register_select_last_txn(&mut self) {
        if let Some(j) = (0..self.register_rows.len())
            .rev()
            .find(|&j| !self.register_rows[j].continuation)
        {
            self.register_table.select(Some(j));
        }
    }
}
