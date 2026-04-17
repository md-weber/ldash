use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub journal: Option<String>,
    pub refresh_interval: u64,
    pub default_tab: String,
    pub number_format: String,
    pub currency_symbol: String,
    pub colors: ColorConfig,
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
            colors: ColorConfig::default(),
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

# Expense category color overrides
# Colors: red, green, blue, yellow, cyan, magenta, white, darkgray,
#         or RGB hex like "#B48CFF"
# [colors.expenses]
# Wohnen = "blue"
# Essen = "yellow"
"##;

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
                eprintln!("Warning: config parse error: {e}");
                Config::default()
            }),
            Err(_) => {
                Self::init_default_config(&path);
                Config::default()
            }
        }
    }

    fn init_default_config(path: &PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, DEFAULT_CONFIG);
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
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/ldash/config.toml")
    } else {
        PathBuf::from(".config/ldash/config.toml")
    }
}
