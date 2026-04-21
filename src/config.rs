use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

static CONFIG_PATH_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

pub fn set_config_path_override(path: PathBuf) {
    let _ = CONFIG_PATH_OVERRIDE.set(path);
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub journal: Option<String>,
    pub refresh_interval: u64,
    pub default_tab: String,
    pub number_format: String,
    pub currency_symbol: String,
    pub chart_mode: String,
    pub show_portfolio: Option<bool>,
    pub colors: ColorConfig,
    pub theme: ThemeConfig,
    pub budgets: HashMap<String, f64>,
    pub goals: Vec<SavingsGoal>,
    pub export_dir: Option<String>,
    pub export_format: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SavingsGoal {
    pub name: String,
    pub target: f64,
    pub account: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ColorConfig {
    pub expenses: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ThemeConfig {
    /// Built-in preset: "dark" (default), "light", "solarized"
    pub preset: Option<String>,
    pub accent: Option<String>,
    pub positive: Option<String>,
    pub negative: Option<String>,
    pub muted: Option<String>,
    pub gold: Option<String>,
    pub fg: Option<String>,
    pub background: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            journal: None,
            refresh_interval: 300,
            default_tab: "accounts".to_string(),
            number_format: "eu".to_string(),
            currency_symbol: "€".to_string(),
            chart_mode: "stacked".to_string(),
            show_portfolio: None,
            colors: ColorConfig::default(),
            theme: ThemeConfig::default(),
            budgets: HashMap::new(),
            goals: Vec::new(),
            export_dir: None,
            export_format: "html".to_string(),
        }
    }
}

const DEFAULT_CONFIG: &str = r##"# ldash configuration
# See: https://github.com/yourusername/ledger_dashboard#configuration

# Path to hledger journal (overrides $LEDGER_FILE and CLI arg)
# journal = "/path/to/all.journal"

# Auto-refresh interval in seconds (default: 300)
# refresh_interval = 300

# Default tab on startup: "portfolio", "accounts", "monthly"
# default_tab = "portfolio"

# Number format: "eu" (1.000,00) or "us" (1,000.00)
# number_format = "eu"

# Currency symbol shown in UI
# currency_symbol = "€"

# Force Portfolio tab visibility. Default: auto-detect from
# presence of `assets:crypto` accounts.
# show_portfolio = true

# Portfolio chart mode: "stacked" or "unstacked"
#   stacked   — lines show Invested / Purchased value / Total value
#               (gaps between lines = price P/L and staking value)
#   unstacked — lines show Invested / Price gain / Staking gain
#               (independent lines, easy to compare gain sources)
# Toggle at runtime with 's' key.
# chart_mode = "stacked"

# Expense category color overrides
# Colors: red, green, blue, yellow, cyan, magenta, white, darkgray,
#         or RGB hex like "#B48CFF"
# [colors.expenses]
# Wohnen = "blue"
# Essen = "yellow"

# Monthly budget limits per expense category
# Matched case-insensitively against expense accounts.
# Shows a progress bar in the expense table and warns when over budget.
# [budgets]
# "expenses:Essen" = 400.0
# "expenses:Freizeit" = 200.0
# "expenses:Transport" = 150.0

# Theme — color preset and per-color overrides
# preset: "dark" (default), "light", "solarized"
# Individual colors: named (red, green, blue, yellow, cyan, magenta,
#   white, gray, darkgray) or hex (#RRGGBB)
# [theme]
# preset = "dark"
# accent = "cyan"
# positive = "green"
# negative = "red"
# muted = "darkgray"
# gold = "yellow"
# fg = "white"
# background = "#14141e"

# Export format: "html" (default), "json", or "both"
# export_format = "html"

# Seeds the path shown in the export prompt (e key). Default: ~/Downloads
# export_dir = "~/Documents/ldash-exports"

# Savings goals — track progress toward financial targets
# Each goal maps a target amount to an account prefix.
# Shown on the Accounts tab as a progress bar.
# [[goals]]
# name = "Emergency Fund"
# target = 15000.0
# account = "assets:bank:savings"
"##;

impl Config {
    pub fn load() -> (Self, Vec<String>) {
        let path = config_path();
        let explicit = CONFIG_PATH_OVERRIDE.get().is_some();
        let mut warnings = Vec::new();
        let config = match std::fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
                warnings.push(format!("Config parse error: {e}"));
                Config::default()
            }),
            Err(_) => {
                if explicit {
                    warnings.push(format!("Config file not found: {}", path.display()));
                } else {
                    Self::init_default_config(&path);
                }
                Config::default()
            }
        };
        (config, warnings)
    }

    pub fn load_strict() -> anyhow::Result<Self> {
        let path = config_path();
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Cannot read config {}: {}", path.display(), e))?;
        toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Config parse error in {}: {}", path.display(), e))
    }

    /// Hot-reload safe fields from disk. Skips startup-only settings
    /// (journal, default_tab) that would be confusing to change mid-session.
    pub fn hot_reload(&mut self) -> Option<String> {
        let path = config_path();
        let content = std::fs::read_to_string(&path).ok()?;
        let fresh: Config = match toml::from_str(&content) {
            Ok(c) => c,
            Err(e) => return Some(format!("Config reload error: {e}")),
        };
        self.refresh_interval = fresh.refresh_interval;
        self.number_format = fresh.number_format;
        self.currency_symbol = fresh.currency_symbol;
        self.chart_mode = fresh.chart_mode;
        self.show_portfolio = fresh.show_portfolio;
        self.colors = fresh.colors;
        self.theme = fresh.theme;
        self.budgets = fresh.budgets;
        self.goals = fresh.goals;
        self.export_dir = fresh.export_dir;
        self.export_format = fresh.export_format;
        None
    }

    fn init_default_config(path: &PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, DEFAULT_CONFIG);
    }

    pub fn config_mtime() -> Option<std::time::SystemTime> {
        std::fs::metadata(config_path())
            .and_then(|m| m.modified())
            .ok()
    }

    pub fn refresh_duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs(if self.refresh_interval > 0 {
            self.refresh_interval
        } else {
            300
        })
    }

    /// Number-format separators per `number_format` setting.
    /// Returns (thousands_sep, decimal_sep). "us" → (",", "."), else "eu" → (".", ",").
    fn separators(&self) -> (char, char) {
        match self.number_format.as_str() {
            "us" => (',', '.'),
            _ => ('.', ','),
        }
    }

    /// Format a raw number with thousands separator and the configured decimal mark.
    /// No currency symbol attached.
    pub fn fmt_number(&self, amount: f64, decimals: usize) -> String {
        let (thousands, decimal) = self.separators();
        let neg = amount.is_sign_negative() && amount != 0.0;
        let s = format!("{:.*}", decimals, amount.abs());
        let (int_part, frac_part) = match s.find('.') {
            Some(i) => (&s[..i], &s[i + 1..]),
            None => (s.as_str(), ""),
        };
        let int_chars: Vec<char> = int_part.chars().collect();
        let mut int_out = String::with_capacity(int_chars.len() + int_chars.len() / 3);
        for (i, c) in int_chars.iter().enumerate() {
            if i > 0 && (int_chars.len() - i).is_multiple_of(3) {
                int_out.push(thousands);
            }
            int_out.push(*c);
        }
        let mut out = String::new();
        if neg {
            out.push('-');
        }
        out.push_str(&int_out);
        if !frac_part.is_empty() {
            out.push(decimal);
            out.push_str(frac_part);
        }
        out
    }

    /// Format an amount with currency symbol, e.g. `"1.234,56 €"` or `"1,234.56 $"`.
    pub fn fmt_amount(&self, amount: f64, decimals: usize) -> String {
        format!(
            "{} {}",
            self.fmt_number(amount, decimals),
            self.currency_symbol
        )
    }

    /// Like `fmt_amount` but no space between number and symbol — e.g. `"1.234,56€"`.
    /// Used in tight spots (axis labels, P/L cells).
    pub fn fmt_amount_compact(&self, amount: f64, decimals: usize) -> String {
        format!(
            "{}{}",
            self.fmt_number(amount, decimals),
            self.currency_symbol
        )
    }
}

fn config_path() -> PathBuf {
    if let Some(p) = CONFIG_PATH_OVERRIDE.get() {
        return p.clone();
    }
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/ldash/config.toml")
    } else {
        PathBuf::from(".config/ldash/config.toml")
    }
}
