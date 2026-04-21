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
    pub colors: ColorConfig,
    pub budgets: HashMap<String, f64>,
    pub goals: Vec<SavingsGoal>,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            journal: None,
            refresh_interval: 300,
            default_tab: "portfolio".to_string(),
            number_format: "eu".to_string(),
            currency_symbol: "€".to_string(),
            chart_mode: "stacked".to_string(),
            colors: ColorConfig::default(),
            budgets: HashMap::new(),
            goals: Vec::new(),
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
                    warnings.push(format!(
                        "Config file not found: {}",
                        path.display()
                    ));
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
        let content = std::fs::read_to_string(&path).map_err(|e| {
            anyhow::anyhow!("Cannot read config {}: {}", path.display(), e)
        })?;
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
        self.colors = fresh.colors;
        self.budgets = fresh.budgets;
        self.goals = fresh.goals;
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

    pub fn default_tab_index(&self) -> usize {
        match self.default_tab.as_str() {
            "accounts" => 1,
            "monthly" => 2,
            _ => 0,
        }
    }

    pub fn refresh_duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs(if self.refresh_interval > 0 {
            self.refresh_interval
        } else {
            300
        })
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
