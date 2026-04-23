use chrono::Local;
use std::time::{Duration, Instant};

use super::{
    budget_spent, App, BudgetItem, CashFlowForecast, GoalProgress, PriceAlert, RecurringExpense,
    YtdStats,
};

impl App {
    pub fn ytd_stats(&self) -> YtdStats {
        let months = &self.monthly.months;
        if months.is_empty() {
            return YtdStats::default();
        }
        let total_income: f64 = months.iter().map(|m| m.total_income).sum();
        let total_expenses: f64 = months.iter().map(|m| m.total_expenses).sum();
        let avg_savings_rate = if total_income > 0.0 {
            ((total_income - total_expenses) / total_income * 100.0).max(0.0)
        } else {
            0.0
        };
        let best = months
            .iter()
            .max_by(|a, b| {
                let na = a.total_income - a.total_expenses;
                let nb = b.total_income - b.total_expenses;
                na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();
        let worst = months
            .iter()
            .min_by(|a, b| {
                let na = a.total_income - a.total_expenses;
                let nb = b.total_income - b.total_expenses;
                na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();
        YtdStats {
            total_income,
            total_expenses,
            avg_savings_rate,
            best_month: best.month_name.clone(),
            best_net: best.total_income - best.total_expenses,
            worst_month: worst.month_name.clone(),
            worst_net: worst.total_income - worst.total_expenses,
        }
    }

    pub fn budget_status(&self) -> Vec<BudgetItem> {
        let m = match self.current_month() {
            Some(m) => m,
            None => return Vec::new(),
        };
        self.config
            .budgets
            .iter()
            .map(|(category, &limit)| {
                let spent = budget_spent(category, &m.expenses);
                BudgetItem {
                    category: category.clone(),
                    limit,
                    spent,
                    pct: if limit > 0.0 {
                        spent / limit * 100.0
                    } else {
                        0.0
                    },
                }
            })
            .collect()
    }

    pub fn goal_progress(&self) -> Vec<GoalProgress> {
        self.config
            .goals
            .iter()
            .map(|g| {
                let current = self
                    .account_balances
                    .iter()
                    .filter(|b| b.account.starts_with(&g.account))
                    .map(|b| b.amount)
                    .sum::<f64>();
                let pct = if g.target > 0.0 {
                    (current / g.target * 100.0).min(100.0)
                } else {
                    0.0
                };
                GoalProgress {
                    name: g.name.clone(),
                    target: g.target,
                    current,
                    pct,
                }
            })
            .collect()
    }

    /// Detect recurring expenses across loaded months (current year + last year).
    ///
    /// Only leaf accounts are considered (skips parent accounts when a child is
    /// present in the same month) to avoid the duplicate "miete, miete" problem
    /// caused by hledger emitting both a parent and its sub-accounts.
    ///
    /// An expense is recurring if it appears in ≥3 months with a near-fixed
    /// amount (max/min < 1.15 — subscriptions and rent don't vary much).
    pub fn recurring_expenses(&self) -> Vec<RecurringExpense> {
        let mut appearances: std::collections::HashMap<&str, Vec<f64>> =
            std::collections::HashMap::new();

        let all_months = self
            .monthly
            .months
            .iter()
            .chain(self.last_year.months.iter());

        for m in all_months {
            for (name, amount) in &m.expenses {
                if *amount <= 0.0 {
                    continue;
                }
                // Skip if any child account is present in the same month.
                let name_lower = name.to_lowercase();
                let has_child = m.expenses.iter().any(|(other, _)| {
                    other
                        .to_lowercase()
                        .starts_with(&format!("{}:", name_lower))
                });
                if !has_child {
                    appearances.entry(name.as_str()).or_default().push(*amount);
                }
            }
        }

        let mut recurring: Vec<RecurringExpense> = appearances
            .into_iter()
            .filter(|(_, amounts)| amounts.len() >= 3)
            .filter(|(_, amounts)| {
                let max = amounts.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let min = amounts.iter().cloned().fold(f64::INFINITY, f64::min);
                // max/min < 1.15 — only near-fixed charges qualify (subscriptions,
                // rent, insurance). Variable expenses like groceries are excluded.
                min > 0.0 && max / min < 1.15
            })
            .map(|(name, amounts)| {
                let avg = amounts.iter().sum::<f64>() / amounts.len() as f64;
                RecurringExpense {
                    name: name.to_string(),
                    monthly_avg: avg,
                    occurrences: amounts.len(),
                }
            })
            .collect();

        recurring.sort_by(|a, b| {
            b.monthly_avg
                .partial_cmp(&a.monthly_avg)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        recurring
    }

    /// Build a 12-month cash flow forecast for the current year.
    ///
    /// **Source priority:**
    /// 1. `monthly_forecast` (loaded via `hledger --forecast`) — uses periodic
    ///    transaction rules defined in the journal, so projections reflect the
    ///    user's actual income/expense schedule.
    /// 2. Computed average — when hledger returned no periodic-rule data for
    ///    future months (journal has no `~ monthly …` rules), we fall back to
    ///    the average net of the months loaded so far.
    ///
    /// Only meaningful when `monthly_year_offset == 0` (current year).
    pub fn cash_flow_forecast(&self) -> CashFlowForecast {
        use crate::data::MONTH_NAMES;

        // Actual data: months loaded from the journal without --forecast.
        let actual_map: std::collections::HashMap<&str, f64> = self
            .monthly
            .months
            .iter()
            .map(|m| (m.month_name.as_str(), m.total_income - m.total_expenses))
            .collect();

        // hledger forecast data: months that are NOT already in actuals.
        // These come from periodic transaction rules via `--forecast`.
        let hledger_forecast_map: std::collections::HashMap<&str, f64> = self
            .monthly_forecast
            .months
            .iter()
            .filter(|m| !actual_map.contains_key(m.month_name.as_str()))
            .map(|m| (m.month_name.as_str(), m.total_income - m.total_expenses))
            .collect();

        // Computed fallback: average net of actual months (for journals with
        // no periodic transaction rules).
        let months_with_income: Vec<_> = self
            .monthly
            .months
            .iter()
            .filter(|m| m.total_income > 0.0)
            .collect();
        let computed_avg_net = if months_with_income.is_empty() {
            0.0
        } else {
            let sum: f64 = months_with_income
                .iter()
                .map(|m| m.total_income - m.total_expenses)
                .sum();
            sum / months_with_income.len() as f64
        };

        let last_actual_idx: Option<usize> = MONTH_NAMES
            .iter()
            .enumerate()
            .filter(|(_, name)| actual_map.contains_key(**name))
            .map(|(i, _)| i)
            .max();

        let use_fallback = hledger_forecast_map.is_empty();

        let mut actuals: Vec<(f64, f64)> = Vec::new();
        let mut projected: Vec<(f64, f64)> = Vec::new();

        for (i, name) in MONTH_NAMES.iter().enumerate() {
            let x = i as f64;
            if let Some(&net) = actual_map.get(*name) {
                actuals.push((x, net));
            } else if let Some(&net) = hledger_forecast_map.get(*name) {
                projected.push((x, net));
            } else if use_fallback && last_actual_idx.map_or(false, |last| i > last) {
                projected.push((x, computed_avg_net));
            }
        }

        // Prepend the last actual point so the forecast line is visually
        // continuous with the actuals line.
        if let Some(&last_a) = actuals.last() {
            if !projected.is_empty() {
                projected.insert(0, last_a);
            }
        }

        // Projected monthly net shown in the chart title: average of the
        // strictly-future projected points (skip the bridge point).
        let forecast_points = if projected.len() > 1 { &projected[1..] } else { &projected[..] };
        let projected_monthly_net = if forecast_points.is_empty() {
            computed_avg_net
        } else {
            forecast_points.iter().map(|(_, y)| *y).sum::<f64>() / forecast_points.len() as f64
        };

        let all_y: Vec<f64> = actuals
            .iter()
            .chain(projected.iter())
            .map(|(_, y)| *y)
            .collect();

        let raw_min = all_y
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min)
            .min(0.0);
        let raw_max = all_y
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max)
            .max(1.0);
        let padding = (raw_max - raw_min) * 0.1;

        CashFlowForecast {
            actuals,
            projected,
            projected_monthly_net,
            min_y: raw_min - padding,
            max_y: raw_max + padding,
        }
    }

    pub(super) fn compute_price_alerts(&mut self) {
        if self.alert_dismissed || !self.crypto_enabled() || self.price_history.len() < 2 {
            return;
        }

        let today = Local::now().date_naive();
        let yesterday = today - chrono::Duration::days(1);

        // One pass over `price_history` to bucket entries per commodity, so
        // the per-coin lookups below are O(log M) instead of O(M). Original
        // (sorted) order within each bucket is preserved. Scoped block keeps
        // the borrow of `self.price_history` from leaking into the mutation
        // of `self.price_alerts` further down.
        let mut alerts: Vec<PriceAlert> = {
            let mut by_coin: std::collections::HashMap<&str, Vec<&crate::data::PriceEntry>> =
                std::collections::HashMap::new();
            for entry in &self.price_history {
                by_coin
                    .entry(entry.commodity.as_str())
                    .or_default()
                    .push(entry);
            }

            let mut out = Vec::new();
            for h in &self.holdings {
                let coin = h.commodity.as_str();
                let prices = match by_coin.get(coin) {
                    Some(p) if !p.is_empty() => p,
                    _ => continue,
                };

                let current = prices.last().map(|e| e.price_eur);
                // `price_history` is already sorted by date — binary search
                // for the last entry with `date <= yesterday`.
                let prev_idx = prices.partition_point(|e| e.date <= yesterday);
                let prev = if prev_idx == 0 {
                    None
                } else {
                    Some(prices[prev_idx - 1].price_eur)
                };

                if let (Some(cur), Some(old)) = (current, prev) {
                    if old > 0.0 {
                        let change = (cur - old) / old * 100.0;
                        if change.abs() >= 2.0 {
                            out.push(PriceAlert {
                                coin: coin.to_string(),
                                change_pct: change,
                            });
                        }
                    }
                }
            }
            out
        };

        if !alerts.is_empty() {
            alerts.sort_by(|a, b| {
                b.change_pct
                    .abs()
                    .partial_cmp(&a.change_pct.abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            self.price_alerts = alerts;
            self.show_alerts = true;
            self.alert_shown_at = Some(Instant::now());
        }
    }

    pub fn check_alert_timeout(&mut self) {
        if self.show_alerts {
            if let Some(shown_at) = self.alert_shown_at {
                if shown_at.elapsed() >= Duration::from_secs(5) {
                    self.show_alerts = false;
                    self.alert_dismissed = true;
                }
            }
        }
    }
}
