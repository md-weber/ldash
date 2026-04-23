use std::sync::mpsc;

use chrono::{Datelike, Local, NaiveDate};

use crate::data::{self, load_net_worth_breakdown, load_net_worth_history, SingleMonth};

use super::{App, MonthlyFocus, MonthlyYearLoad, NetWorthLoad, PortfolioRange};

impl App {
    pub fn current_month(&self) -> Option<&SingleMonth> {
        self.combined_months
            .get(self.combined_selected)
            .map(|(m, _)| m)
    }

    pub fn last_year_match(&self) -> Option<&SingleMonth> {
        let cur = self.current_month()?;
        self.last_year
            .months
            .iter()
            .find(|m| m.month_name == cur.month_name)
    }

    pub fn cycle_year_back(&mut self) {
        self.monthly_year_offset -= 1;
        self.reload_monthly_year();
    }

    pub fn cycle_year_forward(&mut self) {
        if self.monthly_year_offset < 0 {
            self.monthly_year_offset += 1;
            self.reload_monthly_year();
        }
    }

    /// Kick off an async reload of the year's monthly data. The result is
    /// applied by `App::check_background()` from the main loop, keeping the
    /// loading spinner live while hledger runs.
    fn reload_monthly_year(&mut self) {
        if self.monthly_year_rx.is_some() {
            return;
        }
        let offset = self.monthly_year_offset;
        let year = Local::now().date_naive().year() + offset;
        let period = format!("monthly in {year}");
        let jp = self.journal_path.clone();
        let currency = self.config.currency_symbol.clone();

        let (tx, rx) = mpsc::channel();
        self.monthly_year_rx = Some(rx);
        self.loading = true;
        self.status_msg = format!("Loading {year}…");

        std::thread::spawn(move || {
            let result =
                data::load_monthly_for_period(&jp, &period, &currency).map_err(|e| e.to_string());
            let _ = tx.send(MonthlyYearLoad {
                year_offset: offset,
                result,
            });
        });
    }

    pub fn displayed_year(&self) -> i32 {
        Local::now().date_naive().year() + self.monthly_year_offset
    }

    pub fn month_left(&mut self) {
        self.combined_selected = self.combined_selected.saturating_sub(1);
    }

    pub fn month_right(&mut self) {
        if !self.combined_months.is_empty()
            && self.combined_selected + 1 < self.combined_months.len()
        {
            self.combined_selected += 1;
        }
    }

    pub fn nw_range_left(&mut self) {
        let prev = self.nw_range.prev();
        if prev != self.nw_range {
            self.nw_range = prev;
            self.reload_net_worth();
        }
    }

    pub fn nw_range_right(&mut self) {
        let next = self.nw_range.next();
        if next != self.nw_range {
            self.nw_range = next;
            self.reload_net_worth();
        }
    }

    /// Min x (days offset) for current portfolio range, given a coin's first_date.
    pub fn portfolio_range_min_x(&self, first_date: NaiveDate) -> f64 {
        let today = Local::now().date_naive();
        let today_x = (today - first_date).num_days() as f64;
        match self.portfolio_range {
            PortfolioRange::Month3 => (today_x - 90.0).max(0.0),
            PortfolioRange::Month6 => (today_x - 180.0).max(0.0),
            PortfolioRange::Ytd => {
                let jan1 = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
                (jan1 - first_date).num_days().max(0) as f64
            }
            PortfolioRange::All => 0.0,
        }
    }

    pub fn portfolio_range_left(&mut self) {
        self.portfolio_range = self.portfolio_range.prev();
    }

    pub fn portfolio_range_right(&mut self) {
        self.portfolio_range = self.portfolio_range.next();
    }

    /// Kick off an async reload of the net-worth history + breakdown for the
    /// current `nw_range`. Both queries run on a single worker thread (one
    /// reload = one channel send) so the UI applies them atomically.
    pub(super) fn reload_net_worth(&mut self) {
        if self.net_worth_rx.is_some() {
            return;
        }
        let period = self.nw_range.period_arg();
        let jp = self.journal_path.clone();
        let currency = self.config.currency_symbol.clone();

        let (tx, rx) = mpsc::channel();
        self.net_worth_rx = Some(rx);
        self.loading = true;
        self.status_msg = "Loading net worth…".to_string();

        std::thread::spawn(move || {
            let history =
                load_net_worth_history(&jp, &period, &currency).map_err(|e| e.to_string());
            // Breakdown is non-fatal: chart simply omits the layer if it fails.
            let breakdown = load_net_worth_breakdown(&jp, &period, &currency).ok();
            let _ = tx.send(NetWorthLoad { history, breakdown });
        });
    }

    /// Rebuild `combined_months` from the current `monthly` (actuals) and
    /// `monthly_forecast` (periodic projections for future months).
    ///
    /// Months are placed in calendar order (Jan → Dec). Forecast months are
    /// only added when viewing the current year (`monthly_year_offset == 0`).
    /// The previous selection is preserved by month name when possible.
    pub(super) fn rebuild_combined_months(&mut self) {
        let actual_names: std::collections::HashSet<&str> = self
            .monthly
            .months
            .iter()
            .map(|m| m.month_name.as_str())
            .collect();

        let mut combined: Vec<(SingleMonth, bool)> = Vec::new();
        for name in &crate::data::MONTH_NAMES {
            if let Some(m) = self
                .monthly
                .months
                .iter()
                .find(|m| m.month_name.as_str() == *name)
            {
                combined.push((m.clone(), false));
            } else if self.monthly_year_offset == 0 {
                if let Some(m) = self.monthly_forecast.months.iter().find(|m| {
                    m.month_name.as_str() == *name && !actual_names.contains(m.month_name.as_str())
                }) {
                    combined.push((m.clone(), true));
                }
            }
        }

        // Preserve selection by month name; fall back to last actual month.
        let target_name: Option<&str> = self
            .combined_months
            .get(self.combined_selected)
            .map(|(m, _)| m.month_name.as_str())
            .or_else(|| self.monthly.months.last().map(|m| m.month_name.as_str()));

        self.combined_selected = target_name
            .and_then(|name| {
                combined
                    .iter()
                    .position(|(m, _)| m.month_name.as_str() == name)
            })
            .unwrap_or_else(|| combined.len().saturating_sub(1));

        self.combined_months = combined;
    }

    /// Returns `true` when the currently selected month is a forecast month.
    pub fn current_month_is_forecast(&self) -> bool {
        self.combined_months
            .get(self.combined_selected)
            .map(|(_, f)| *f)
            .unwrap_or(false)
    }

    pub fn toggle_monthly_focus(&mut self) {
        self.monthly_focus = match self.monthly_focus {
            MonthlyFocus::Income => MonthlyFocus::Expenses,
            MonthlyFocus::Expenses => MonthlyFocus::Income,
        };
    }
}
