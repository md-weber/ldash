use chrono::Local;
use std::path::PathBuf;

use crate::data;

use super::refresh::load_all_data;
use super::{App, Tab};

fn json_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

impl App {
    pub fn export_current_view(&self) -> String {
        let sym = &self.config.currency_symbol;
        match self.tab {
            Tab::Portfolio => {
                let header = format!("Coin,Amount,Price {sym},Value {sym},Allocation %\n");
                let mut csv = header;
                let total = self.total_portfolio_value();
                for h in &self.holdings {
                    let alloc = if total > 0.0 {
                        h.value_eur / total * 100.0
                    } else {
                        0.0
                    };
                    csv.push_str(&format!(
                        "{},{:.6},{:.2},{:.2},{:.1}\n",
                        h.commodity, h.amount, h.price_eur, h.value_eur, alloc
                    ));
                }
                csv
            }
            Tab::Accounts => {
                if let Some(sel) = self.account_state.selected() {
                    if let Some(b) = self.account_balances.get(sel) {
                        return format!("{}\t{}", b.account, self.config.fmt_amount(b.amount, 2));
                    }
                }
                let mut csv = format!("Account,Balance {sym}\n");
                for b in &self.account_balances {
                    csv.push_str(&format!("{},{:.2}\n", b.account, b.amount));
                }
                csv
            }
            Tab::Monthly => {
                let empty = data::SingleMonth::default();
                let m = self.current_month().unwrap_or(&empty);
                let mut csv = format!("# {} Income/Expenses\n", m.month_name);
                csv.push_str("Category,Amount\n");
                for (name, amount) in &m.income {
                    csv.push_str(&format!("{},{:.2}\n", name, amount));
                }
                for (name, amount) in &m.expenses {
                    csv.push_str(&format!("{},-{:.2}\n", name, amount));
                }
                csv
            }
        }
    }

    /// Synchronously load any tabs whose data hasn't been fetched yet, so that
    /// an export always contains the full three-tab snapshot.
    fn ensure_all_tabs_for_export(&mut self) {
        let mut need = [false; 3];
        if self.crypto_enabled() && !self.tabs_loaded[Tab::Portfolio.index()] {
            need[Tab::Portfolio.index()] = true;
        }
        if !self.tabs_loaded[Tab::Accounts.index()] {
            need[Tab::Accounts.index()] = true;
        }
        if !self.tabs_loaded[Tab::Monthly.index()] {
            need[Tab::Monthly.index()] = true;
        }
        if need.iter().any(|&v| v) {
            let jp = self.journal_path.clone();
            let jd = self.journal_dir.clone();
            let nw_period = self.nw_range.period_arg().to_string();
            let currency = self.config.currency_symbol.clone();
            let result = load_all_data(&jp, &jd, &nw_period, &currency, need);
            self.apply_refresh(result);
        }
    }

    /// Write export to `dest_path`. Format is inferred from the file extension
    /// (.json → JSON, anything else → HTML). "both" config writes an additional
    /// sibling file with the other extension.
    pub fn export_to_file(&mut self, dest_path: &str) -> Result<String, String> {
        self.ensure_all_tabs_for_export();
        let path = PathBuf::from(dest_path);
        if let Some(dir) = path.parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("Cannot create dir {}: {e}", dir.display()))?;
            }
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("html");
        let fmt = self.config.export_format.as_str();

        let write_html = fmt == "both" || ext != "json";
        let write_json = fmt == "both" || ext == "json";

        let mut written: Vec<String> = Vec::new();

        if write_html {
            let html_path = path.with_extension("html");
            std::fs::write(&html_path, self.render_html())
                .map_err(|e| format!("Cannot write {}: {e}", html_path.display()))?;
            written.push(html_path.display().to_string());
        }

        if write_json {
            let json_path = path.with_extension("json");
            std::fs::write(&json_path, self.render_json())
                .map_err(|e| format!("Cannot write {}: {e}", json_path.display()))?;
            written.push(json_path.display().to_string());
        }

        Ok(written.join(" + "))
    }

    fn render_html(&self) -> String {
        let sym = &self.config.currency_symbol;
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S");

        let mut portfolio_rows = String::new();
        let total = self.total_portfolio_value();
        for h in &self.holdings {
            let alloc = if total > 0.0 {
                h.value_eur / total * 100.0
            } else {
                0.0
            };
            portfolio_rows.push_str(&format!(
                "<tr><td>{}</td><td>{:.6}</td><td>{:.2}</td><td>{:.2}</td><td>{:.1}%</td></tr>\n",
                h.commodity, h.amount, h.price_eur, h.value_eur, alloc
            ));
        }

        let mut account_rows = String::new();
        for b in &self.account_balances {
            account_rows.push_str(&format!(
                "<tr><td>{}</td><td>{:.2}</td></tr>\n",
                b.account, b.amount
            ));
        }

        let mut monthly_rows = String::new();
        let empty = data::SingleMonth::default();
        let m = self.current_month().unwrap_or(&empty);
        for (name, amount) in &m.income {
            monthly_rows.push_str(&format!(
                "<tr><td>{}</td><td class=\"pos\">{:.2}</td></tr>\n",
                name, amount
            ));
        }
        for (name, amount) in &m.expenses {
            monthly_rows.push_str(&format!(
                "<tr><td>{}</td><td class=\"neg\">-{:.2}</td></tr>\n",
                name, amount
            ));
        }
        let month_title = &m.month_name;

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>ldash export {ts}</title>
<style>
  body {{ font-family: system-ui, sans-serif; background: #0f0f17; color: #cdd6f4; margin: 2rem; }}
  h1   {{ color: #cba6f7; font-size: 1.1rem; margin-bottom: 1.5rem; }}
  h2   {{ color: #89b4fa; font-size: 0.95rem; margin: 2rem 0 0.5rem; border-bottom: 1px solid #313244; padding-bottom: 0.25rem; }}
  table {{ border-collapse: collapse; width: 100%; max-width: 700px; font-size: 0.85rem; }}
  th   {{ text-align: left; color: #6c7086; font-weight: 600; padding: 0.3rem 0.8rem; border-bottom: 1px solid #313244; }}
  td   {{ padding: 0.25rem 0.8rem; }}
  tr:hover td {{ background: #1e1e2e; }}
  .pos {{ color: #a6e3a1; }}
  .neg {{ color: #f38ba8; }}
  footer {{ margin-top: 3rem; font-size: 0.75rem; color: #45475a; }}
</style>
</head>
<body>
<h1>ldash export &mdash; {ts}</h1>

<h2>Portfolio</h2>
<table>
<tr><th>Coin</th><th>Amount</th><th>Price {sym}</th><th>Value {sym}</th><th>Allocation</th></tr>
{portfolio_rows}</table>

<h2>Accounts</h2>
<table>
<tr><th>Account</th><th>Balance {sym}</th></tr>
{account_rows}</table>

<h2>Monthly &mdash; {month_title}</h2>
<table>
<tr><th>Category</th><th>Amount {sym}</th></tr>
{monthly_rows}</table>

<footer>Generated by ldash &bull; {ts}</footer>
</body>
</html>
"#
        )
    }

    fn render_json(&self) -> String {
        let sym = &self.config.currency_symbol;
        let ts = Local::now().format("%Y-%m-%dT%H:%M:%S");

        let total = self.total_portfolio_value();
        let portfolio: Vec<String> = self.holdings.iter().map(|h| {
            let alloc = if total > 0.0 { h.value_eur / total * 100.0 } else { 0.0 };
            format!(
                r#"    {{"coin":{},"amount":{:.6},"price_{}":{},"value_{}":{},"allocation_pct":{:.1}}}"#,
                json_str(&h.commodity), h.amount,
                sym, h.price_eur,
                sym, h.value_eur,
                alloc
            )
        }).collect();

        let accounts: Vec<String> = self
            .account_balances
            .iter()
            .map(|b| {
                format!(
                    r#"    {{"account":{},"balance":{:.2}}}"#,
                    json_str(&b.account),
                    b.amount
                )
            })
            .collect();

        let empty = data::SingleMonth::default();
        let m = self.current_month().unwrap_or(&empty);
        let mut monthly: Vec<String> = Vec::new();
        for (name, amount) in &m.income {
            monthly.push(format!(
                r#"    {{"category":{},"type":"income","amount":{:.2}}}"#,
                json_str(name),
                amount
            ));
        }
        for (name, amount) in &m.expenses {
            monthly.push(format!(
                r#"    {{"category":{},"type":"expense","amount":{:.2}}}"#,
                json_str(name),
                -amount
            ));
        }

        format!(
            "{{\n  \"exported\": \"{ts}\",\n  \"currency\": \"{sym}\",\
             \n  \"portfolio\": [\n{}\n  ],\
             \n  \"accounts\": [\n{}\n  ],\
             \n  \"monthly\": {{\n    \"month\": {},\n    \"entries\": [\n{}\n    ]\n  }}\n}}",
            portfolio.join(",\n"),
            accounts.join(",\n"),
            json_str(&m.month_name),
            monthly.join(",\n"),
        )
    }

    pub fn open_export_prompt(&mut self) {
        let ext = match self.config.export_format.as_str() {
            "json" => "json",
            _ => "html",
        };
        let base_dir = self
            .config
            .export_dir
            .as_deref()
            .map(|d| {
                if d.starts_with('~') {
                    std::env::var("HOME")
                        .map(|h| d.replacen('~', &h, 1))
                        .unwrap_or_else(|_| d.to_string())
                } else {
                    d.to_string()
                }
            })
            .unwrap_or_else(|| {
                std::env::var("HOME")
                    .map(|h| format!("{}/Downloads", h))
                    .unwrap_or_else(|_| ".".to_string())
            });
        let ts = Local::now().format("%Y-%m-%d-%H%M%S");
        self.export_prompt_path = format!("{}/ldash-{}.{}", base_dir, ts, ext);
        self.export_prompt_active = true;
    }

    pub fn cancel_export_prompt(&mut self) {
        self.export_prompt_active = false;
        self.export_prompt_path.clear();
    }
}
