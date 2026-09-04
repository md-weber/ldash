use std::collections::HashSet;

use chrono::{Datelike, Local};

use crate::data::SingleMonth;

use super::{App, CategoryShare, CashOverview, MonthBreakdown, RecurringExpense};

#[derive(Debug, Clone)]
pub struct LiquidAccountChart {
    pub series: Vec<(String, Vec<(f64, f64)>)>,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

impl App {
    /// Point the Dashboard at the current calendar month with hledger forecast
    /// (actuals plus projected remainder from periodic rules).
    pub fn ensure_dashboard_view(&mut self) {
        if self.monthly_year_offset != 0 {
            return;
        }
        let needs_rebuild = !self.show_current_month_forecast;
        self.show_current_month_forecast = true;
        if needs_rebuild {
            self.rebuild_combined_months();
        }
        self.select_calendar_month(Local::now().date_naive().month());
    }

    /// Jump to the current calendar month (Dashboard `G` shortcut).
    pub fn jump_to_current_month(&mut self) {
        if self.monthly_year_offset != 0 {
            self.monthly_year_offset = 0;
            self.reload_monthly_year();
            return;
        }
        self.ensure_dashboard_view();
    }

    /// Index of the selected month on the Jan–Dec axis (0–11).
    pub fn selected_month_index(&self) -> Option<usize> {
        let name = self.current_month()?.month_name.as_str();
        crate::data::MONTH_NAMES
            .iter()
            .position(|m| *m == name)
    }

    fn select_calendar_month(&mut self, month: u32) {
        let cur_name = crate::data::month_name(month as usize);
        if let Some(i) = self
            .combined_months
            .iter()
            .position(|(m, _)| m.month_name == cur_name)
        {
            self.combined_selected = i;
        }
    }

    pub fn month_breakdown(&self) -> MonthBreakdown {
        let empty = SingleMonth::default();
        let month = self.current_month().unwrap_or(&empty);
        let recurring_exp = self.forecast_recurring_expenses();
        let recurring_inc = self.forecast_recurring_income();

        let recurring_expenses = sum_matching(&month.expenses, &recurring_exp);
        let recurring_income = sum_matching(&month.income, &recurring_inc);
        let total_expenses = month.total_expenses;
        let other_expenses = (total_expenses - recurring_expenses).max(0.0);

        MonthBreakdown {
            total_income: month.total_income,
            recurring_income,
            recurring_expenses,
            other_expenses,
            total_expenses,
        }
    }

    pub fn top_expense_categories(&self, limit: usize) -> Vec<CategoryShare> {
        let empty = SingleMonth::default();
        let month = self.current_month().unwrap_or(&empty);
        let leaves = leaf_entries(&month.expenses);
        let total: f64 = leaves.iter().map(|(_, a)| *a).sum();
        if total <= 0.0 {
            return Vec::new();
        }

        let mut ranked: Vec<(String, f64)> = leaves;
        ranked.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        ranked.truncate(limit);

        ranked
            .into_iter()
            .map(|(name, amount)| {
                let short = self
                    .config
                    .strip_account_prefix(&name, &self.config.expenses_account);
                CategoryShare {
                    name: short.to_string(),
                    amount,
                    fraction: amount / total,
                }
            })
            .collect()
    }

    pub fn cash_overview(&self) -> CashOverview {
        let accounts = self.liquid_cash_accounts();
        let total_balance: f64 = accounts.iter().map(|(_, a)| *a).sum();
        let ytd_net_change = self.liquid_ytd_change();

        CashOverview {
            accounts,
            total_balance,
            ytd_net_change,
        }
    }

    /// Cumulative YTD net change per top liquid account for the dashboard chart.
    pub fn liquid_account_chart(&self, limit: usize) -> LiquidAccountChart {
        let by_account = if self.current_month_is_forecast() {
            &self.liquid_accounts_forecast
        } else {
            &self.liquid_accounts_monthly
        };
        let through = self
            .current_month()
            .map(|m| m.month_name.as_str())
            .unwrap_or("");

        let top: Vec<String> = self
            .liquid_cash_accounts()
            .into_iter()
            .take(limit)
            .map(|(n, _)| n)
            .collect();

        let mut series = Vec::new();
        let mut y_min = 0.0_f64;
        let mut y_max = 0.0_f64;

        for short in top {
            let Some(monthly) = find_account_monthly(by_account, &short, &self.config) else {
                continue;
            };
            let mut cum = 0.0;
            let mut points = Vec::new();
            for (month, change) in monthly.iter() {
                let Some(i) = crate::data::MONTH_NAMES
                    .iter()
                    .position(|m| *m == month.as_str())
                else {
                    continue;
                };
                cum += change;
                points.push((i as f64, cum));
                y_min = y_min.min(cum);
                y_max = y_max.max(cum);
                if month == through {
                    break;
                }
            }
            if !points.is_empty() {
                series.push((short, points));
            }
        }

        if series.is_empty() {
            y_max = 1.0;
        }

        LiquidAccountChart {
            series,
            x_max: 11.0,
            y_min,
            y_max: y_max.max(y_min + 1.0),
        }
    }

    /// Recurring expense accounts inferred from hledger `--forecast` periodic
    /// rules (stable amounts across projected months).
    pub fn forecast_recurring_expenses(&self) -> Vec<RecurringExpense> {
        detect_recurring(
            self.monthly_forecast
                .months
                .iter()
                .map(|m| m.expenses.as_slice()),
        )
    }

    /// Recurring income accounts inferred from hledger `--forecast` periodic rules.
    pub fn forecast_recurring_income(&self) -> Vec<RecurringExpense> {
        detect_recurring(
            self.monthly_forecast
                .months
                .iter()
                .map(|m| m.income.as_slice()),
        )
    }

    fn liquid_cash_accounts(&self) -> Vec<(String, f64)> {
        let prefixes: Vec<String> = if !self.config.liquid_accounts.is_empty() {
            self.config.liquid_accounts.clone()
        } else {
            let assets = &self.config.assets_account;
            vec![format!("{assets}:bank"), format!("{assets}:cash")]
        };

        let mut out: Vec<(String, f64)> = self
            .account_balances
            .iter()
            .filter(|b| prefixes.iter().any(|p| b.account.starts_with(p)))
            .map(|b| {
                let short = self
                    .config
                    .strip_account_prefix(&b.account, &self.config.assets_account);
                (short.to_string(), b.amount)
            })
            .collect();

        out.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }

    fn liquid_ytd_change(&self) -> f64 {
        if self.monthly_year_offset != 0 {
            return 0.0;
        }
        let month_name = self
            .current_month()
            .map(|m| m.month_name.as_str())
            .unwrap_or("");
        let series = if self.current_month_is_forecast() {
            &self.liquid_cash_forecast
        } else {
            &self.liquid_cash_monthly
        };
        let mut total = 0.0;
        for (name, change) in series {
            total += change;
            if *name == month_name {
                break;
            }
        }
        total
    }
}

fn find_account_monthly<'a>(
    by_account: &'a [(String, Vec<(String, f64)>)],
    short_name: &str,
    config: &crate::config::Config,
) -> Option<&'a [(String, f64)]> {
    by_account
        .iter()
        .find(|(full, _)| {
            config.strip_account_prefix(full, &config.assets_account) == short_name
        })
        .map(|(_, s)| s.as_slice())
}

fn detect_recurring<'a>(
    months: impl Iterator<Item = &'a [(String, f64)]>,
) -> Vec<RecurringExpense> {
    let mut appearances: std::collections::HashMap<&str, Vec<f64>> =
        std::collections::HashMap::new();

    for entries in months {
        for (name, amount) in entries {
            if *amount <= 0.0 {
                continue;
            }
            let name_lower = name.to_lowercase();
            let has_child = entries.iter().any(|(other, _)| {
                other
                    .to_lowercase()
                    .starts_with(&format!("{name_lower}:"))
            });
            if !has_child {
                appearances.entry(name.as_str()).or_default().push(*amount);
            }
        }
    }

    let mut recurring: Vec<RecurringExpense> = appearances
        .into_iter()
        .filter(|(_, amounts)| amounts.len() >= 2)
        .filter(|(_, amounts)| {
            let max = amounts.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let min = amounts.iter().cloned().fold(f64::INFINITY, f64::min);
            min > 0.0 && max / min < 1.15
        })
        .map(|(name, amounts)| {
            let avg = amounts.iter().sum::<f64>() / amounts.len() as f64;
            RecurringExpense {
                name: name.to_string(),
                monthly_avg: avg,
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

fn sum_matching(entries: &[(String, f64)], recurring: &[RecurringExpense]) -> f64 {
    if recurring.is_empty() {
        return 0.0;
    }
    let keys: HashSet<String> = recurring
        .iter()
        .map(|r| r.name.to_lowercase())
        .collect();
    entries
        .iter()
        .filter(|(name, _)| {
            let nl = name.to_lowercase();
            keys.iter()
                .any(|k| nl == *k || nl.starts_with(&format!("{k}:")))
        })
        .map(|(_, amt)| *amt)
        .sum()
}

fn leaf_entries(entries: &[(String, f64)]) -> Vec<(String, f64)> {
    entries
        .iter()
        .filter(|(name, amount)| {
            if *amount <= 0.0 {
                return false;
            }
            let name_lower = name.to_lowercase();
            !entries.iter().any(|(other, _)| {
                other
                    .to_lowercase()
                    .starts_with(&format!("{name_lower}:"))
            })
        })
        .map(|(n, a)| (n.clone(), *a))
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::{Datelike, Local};

    use crate::app::App;
    use crate::data::{MonthlyData, SingleMonth};

    fn month(name: &str, income: f64, expenses: Vec<(&str, f64)>) -> SingleMonth {
        let total_expenses: f64 = expenses.iter().map(|(_, a)| *a).sum();
        SingleMonth {
            month_name: name.to_string(),
            income: vec![("income:salary".to_string(), income)],
            expenses: expenses
                .into_iter()
                .map(|(n, a)| (n.to_string(), a))
                .collect(),
            total_income: income,
            total_expenses,
        }
    }

    #[test]
    fn month_breakdown_uses_forecast_recurring() {
        let mut app = App::fixture_with_monthly();
        app.show_current_month_forecast = true;
        app.monthly = MonthlyData::default();
        app.monthly_forecast = MonthlyData {
            months: (1..=12)
                .map(|i| {
                    month(
                        crate::data::MONTH_NAMES[i - 1],
                        3000.0,
                        vec![("expenses:housing", 1200.0), ("expenses:food", 500.0)],
                    )
                })
                .collect(),
        };
        app.rebuild_combined_months();
        app.combined_selected = 0;

        let rec = app.forecast_recurring_expenses();
        assert!(
            !rec.is_empty(),
            "expected forecast recurring entries, got {rec:?}"
        );
        let b = app.month_breakdown();
        assert_eq!(b.total_income, 3000.0);
        assert!(
            b.recurring_expenses > 0.0,
            "recurring_expenses={} rec={rec:?}",
            b.recurring_expenses
        );
        assert!((b.total_expenses - b.recurring_expenses - b.other_expenses).abs() < 0.01);
    }

    #[test]
    fn top_expense_categories_returns_top_five() {
        let mut app = App::fixture_with_monthly();
        app.monthly.months[0].expenses = vec![
            ("expenses:a".to_string(), 100.0),
            ("expenses:b".to_string(), 200.0),
            ("expenses:c".to_string(), 300.0),
            ("expenses:d".to_string(), 400.0),
            ("expenses:e".to_string(), 500.0),
            ("expenses:f".to_string(), 50.0),
        ];
        app.monthly.months[0].total_expenses = 1550.0;
        app.rebuild_combined_months();
        app.combined_selected = 0;

        let top = app.top_expense_categories(5);
        assert_eq!(top.len(), 5);
        assert_eq!(top[0].name, "e");
    }

    #[test]
    fn jump_to_current_month_selects_calendar_month() {
        let mut app = App::fixture_with_monthly();
        app.combined_selected = 0;
        app.jump_to_current_month();
        assert!(app.show_current_month_forecast);
        let cur = crate::data::month_name(Local::now().date_naive().month() as usize);
        if app
            .combined_months
            .iter()
            .any(|(m, _)| m.month_name == cur)
        {
            assert_eq!(app.selected_month_index(), Some(Local::now().date_naive().month() as usize - 1));
        }
    }

    #[test]
    fn ensure_dashboard_view_selects_current_month_with_forecast() {
        let mut app = App::fixture_with_monthly();
        app.show_current_month_forecast = false;
        app.combined_selected = 0;
        app.ensure_dashboard_view();
        assert!(app.show_current_month_forecast);
        let cur = crate::data::month_name(Local::now().date_naive().month() as usize);
        if app
            .combined_months
            .iter()
            .any(|(m, _)| m.month_name == cur)
        {
            assert_eq!(
                app.current_month().map(|m| m.month_name.as_str()),
                Some(cur)
            );
        }
    }
}
