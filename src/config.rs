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
    pub price_alert_threshold_pct: f64,
    pub show_portfolio: Option<bool>,
    /// Top-level account name for expenses (default "expenses").
    /// Set to "Ausgaben", "Depenses", etc. for non-English journals.
    pub expenses_account: String,
    /// Top-level account name for income (default "income").
    pub income_account: String,
    /// Top-level account name for assets (default "assets").
    pub assets_account: String,
    /// Top-level account name for liabilities (default "liabilities").
    pub liabilities_account: String,
    /// When `true` the currency symbol is placed *before* the amount
    /// (`Eur 100,00` instead of `100,00 €`).  Auto-detected from the journal at
    /// startup; set explicitly in config to override.
    pub currency_prefix: bool,
    pub colors: ColorConfig,
    pub theme: ThemeConfig,
    pub budgets: HashMap<String, f64>,
    pub goals: Vec<SavingsGoal>,
    pub export_dir: Option<String>,
    pub export_format: String,
    /// Optional list of journal paths for quick switching (`:o` → Tab picker).
    pub journals: Vec<String>,
    /// Account prefixes treated as *liquid* cash for the "Liquid Chg" figure
    /// on the Monthly tab (net change in these accounts during the selected
    /// month). Only accounts whose full name starts with one of these
    /// prefixes are summed, so non-liquid asset accounts (AFA, bounded
    /// mortgage savings / Tilgungsaussetzung, investments, …) can be excluded
    /// by simply not listing them. Empty (default) disables the feature.
    pub liquid_accounts: Vec<String>,
    /// Automatic price fetching from CoinGecko.
    /// When `tokens` is non-empty, ldash checks once per day at startup whether
    /// today's prices are already in `prices.journal`. If not, it fetches them
    /// automatically and appends the entries. The `P` key triggers a manual
    /// fetch at any time.
    pub price_fetch: PriceFetchConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SavingsGoal {
    pub name: String,
    pub target: f64,
    pub account: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PriceFetchToken {
    pub symbol: String,
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct PriceFetchConfig {
    pub tokens: Vec<PriceFetchToken>,
    pub currency: String,
}

impl Default for PriceFetchConfig {
    fn default() -> Self {
        Self {
            tokens: Vec::new(),
            currency: "eur".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ColorConfig {
    pub expenses: HashMap<String, String>,
    pub income: HashMap<String, String>,
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
            default_tab: "dashboard".to_string(),
            number_format: "eu".to_string(),
            currency_symbol: "€".to_string(),
            chart_mode: "stacked".to_string(),
            price_alert_threshold_pct: 2.0,
            show_portfolio: None,
            expenses_account: "expenses".to_string(),
            income_account: "income".to_string(),
            assets_account: "assets".to_string(),
            liabilities_account: "liabilities".to_string(),
            currency_prefix: false,
            colors: ColorConfig::default(),
            theme: ThemeConfig::default(),
            budgets: HashMap::new(),
            goals: Vec::new(),
            export_dir: None,
            export_format: "html".to_string(),
            journals: Vec::new(),
            liquid_accounts: Vec::new(),
            price_fetch: PriceFetchConfig::default(),
        }
    }
}

const DEFAULT_CONFIG: &str = r##"# ldash configuration
# See: https://github.com/md-weber/ldash#configuration

# Path to hledger journal (overrides $LEDGER_FILE; CLI -f still wins)
# journal = "/path/to/all.journal"

# Auto-refresh interval in seconds (default: 300)
# refresh_interval = 300

# Default tab on startup: "portfolio", "accounts", "monthly", "register"
# default_tab = "dashboard"   # dashboard | accounts | monthly | register | portfolio

# Number format: "eu" (1.000,00) or "us" (1,000.00)
# number_format = "eu"

# Currency symbol shown in UI
# currency_symbol = "€"

# Place the currency symbol before the amount ("Eur 100,00" instead of "100,00 €").
# Auto-detected from the journal at startup; uncomment to override.
# currency_prefix = false

# Top-level account names used in your journal.
# Only needed when your journal uses non-English account names.
# Matching is always case-insensitive ("Expenses" works without any change).
# expenses_account = "expenses"   # e.g. "Ausgaben", "Depenses"
# income_account   = "income"     # e.g. "Einnahmen", "Revenus"
# assets_account   = "assets"     # e.g. "Aktiva",    "Actifs"
# liabilities_account = "liabilities"  # e.g. "Verbindlichkeiten"

# Force Portfolio tab visibility. Default: true (always shown).
# Set to false to hide the tab entirely.
# show_portfolio = false

# Portfolio chart mode: "stacked" or "unstacked"
#   stacked   — lines show Invested / Purchased value / Total value
#               (gaps between lines = price P/L and staking value)
#   unstacked — lines show Invested / Price gain / Staking gain
#               (independent lines, easy to compare gain sources)
# Toggle at runtime with 's' key.
# chart_mode = "stacked"

# Price alert threshold in percent (default: 2.0)
# Popup shows only coins with absolute day-over-day move >= threshold.
# Set 0.0 to show all daily moves.
# price_alert_threshold_pct = 2.0

# Expense and income category color overrides
# Colors: red, green, blue, yellow, cyan, magenta, white, darkgray,
#         or RGB hex like "#B48CFF"
# Keys match account names case-insensitively. A top-level key colors all
# its sub-accounts; the most specific (longest) matching key wins.
# [colors.expenses]
# wohnen = "blue"            # colors expenses:wohnen and all children
# "wohnen:miete" = "#B48CFF" # overrides just the miete sub-account
# essen = "yellow"
#
# [colors.income]
# gehalt = "cyan"
# nebenjob = "#B48CFF"

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

# Quick-switch journals — press Ctrl-O then ↑/↓ or Tab to cycle
# journals = [
#   "~/Finance/2024.journal",
#   "~/Finance/2025.journal",
#   "~/Finance/2026.journal",
# ]

# Liquid cash — net change, per month, of accounts you can spend freely.
# Only accounts whose name starts with one of these prefixes are summed, so
# non-liquid assets (AFA / depreciation, bounded mortgage savings /
# Tilgungsaussetzung, investments) stay excluded by omission.
# Shown as a "Liquid Chg" line on the Monthly tab for whichever month is
# selected. Past months show the posted change. The current month includes
# the forecast remainder only when F is on. Future months always include
# periodic-rule projections. Empty (default) hides it.
# liquid_accounts = [
#   "assets:bank:checking",
#   "assets:bank:savings",
#   "assets:cash",
# ]

# Savings goals — track progress toward financial targets
# Each goal maps a target amount to an account prefix.
# Shown on the Accounts tab as a progress bar.
# [[goals]]
# name = "Emergency Fund"
# target = 15000.0
# account = "assets:bank:savings"

# Automatic price fetching from CoinGecko.
# When tokens are configured, ldash fetches today's prices once at startup
# (skipped if today is already in prices.journal) and writes them to
# prices.journal in the journal's directory. Press `P` to fetch manually.
# currency = target vs-currency (lowercase), e.g. "eur" or "usd".
# Each [[price_fetch.tokens]] entry maps a commodity symbol (as used in your
# journal) to the CoinGecko coin ID.
# [price_fetch]
# currency = "eur"
# [[price_fetch.tokens]]
# symbol = "BTC"
# id     = "bitcoin"
# [[price_fetch.tokens]]
# symbol = "ETH"
# id     = "ethereum"
# [[price_fetch.tokens]]
# symbol = "SOL"
# id     = "solana"
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
        warnings.extend(config.validate());
        (config, warnings)
    }

    pub fn load_strict() -> anyhow::Result<Self> {
        let path = config_path();
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Cannot read config {}: {}", path.display(), e))?;
        toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Config parse error in {}: {}", path.display(), e))
    }

    /// Sanity-check semantically-loose fields (those that parse but make no
    /// sense). Returns a list of human-readable warnings; an empty Vec means
    /// the config is internally consistent.
    fn validate(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.refresh_interval == 0 {
            out.push(
                "refresh_interval = 0 is invalid; falling back to 300s. \
                 Set a positive value or remove the key."
                    .to_string(),
            );
        }
        if self.price_alert_threshold_pct < 0.0 {
            out.push(
                "price_alert_threshold_pct < 0 is invalid; falling back to 2.0. \
                 Set a non-negative value or remove the key."
                    .to_string(),
            );
        }
        out
    }

    /// Strip a root account prefix from a full account name for display.
    ///
    /// Matching is **case-insensitive** so both `"expenses:food"` and
    /// `"Expenses:Food"` are stripped correctly when `root = "expenses"`.
    /// Returns the original slice unchanged when the prefix is absent.
    pub fn strip_account_prefix<'a>(&self, name: &'a str, root: &str) -> &'a str {
        let prefix_len = root.len() + 1; // root + ":"
        if name.len() > prefix_len
            && name[..root.len()].eq_ignore_ascii_case(root)
            && name.as_bytes().get(root.len()) == Some(&b':')
        {
            &name[prefix_len..]
        } else {
            name
        }
    }

    /// Returns `true` when expense account `name` falls under budget `category`.
    ///
    /// The configured `expenses_account` root is prepended when the category
    /// does not already start with it, so `"food"` and `"expenses:food"` (or
    /// `"Ausgaben:food"` when `expenses_account = "ausgaben"`) all match the
    /// same entries. Matching is case-insensitive.
    pub fn budget_matches(&self, category: &str, name: &str) -> bool {
        let root = self.expenses_account.to_lowercase();
        let prefix = format!("{}:", root);
        let cat_lower = category.to_lowercase();
        let cat_full = if cat_lower.starts_with(&prefix) {
            cat_lower
        } else {
            format!("{}{}", prefix, cat_lower)
        };
        let name_lower = name.to_lowercase();
        name_lower == cat_full || name_lower.starts_with(&format!("{}:", cat_full))
    }

    /// Sum of expenses in `month_expenses` that match `category`.
    ///
    /// Only leaf accounts are counted (an account is a leaf when no child entry
    /// is present in the same list) to avoid double-counting parent accounts
    /// that hledger emits alongside their sub-accounts.
    pub fn budget_spent(&self, category: &str, expenses: &[(String, f64)]) -> f64 {
        expenses
            .iter()
            .filter(|(name, _)| self.budget_matches(category, name))
            .filter(|(name, _)| {
                let name_lower = name.to_lowercase();
                !expenses.iter().any(|(other, _)| {
                    other
                        .to_lowercase()
                        .starts_with(&format!("{}:", name_lower))
                })
            })
            .map(|(_, amount)| *amount)
            .sum()
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
        // Only update currency_symbol / currency_prefix when the user has
        // explicitly set a non-default value in the config file. If the config
        // still holds the defaults they were likely auto-detected at startup
        // and must not be reverted on hot-reload.
        if fresh.currency_symbol != Config::default().currency_symbol {
            self.currency_symbol = fresh.currency_symbol;
        }
        if fresh.currency_prefix != Config::default().currency_prefix {
            self.currency_prefix = fresh.currency_prefix;
        }
        self.chart_mode = fresh.chart_mode;
        self.expenses_account = fresh.expenses_account;
        self.income_account = fresh.income_account;
        self.assets_account = fresh.assets_account;
        self.liabilities_account = fresh.liabilities_account;
        self.price_alert_threshold_pct = fresh.price_alert_threshold_pct;
        self.show_portfolio = fresh.show_portfolio;
        self.colors = fresh.colors;
        self.theme = fresh.theme;
        self.budgets = fresh.budgets;
        self.goals = fresh.goals;
        self.export_dir = fresh.export_dir;
        self.export_format = fresh.export_format;
        self.journals = fresh.journals;
        self.liquid_accounts = fresh.liquid_accounts;
        self.price_fetch = fresh.price_fetch;
        None
    }

    fn init_default_config(path: &PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, DEFAULT_CONFIG);
    }

    /// Persist `journal = "<path>"` to the user's config file.
    ///
    /// Preserves every other line, comment, and table — only the top-level
    /// `journal` key is rewritten. Behaviour:
    /// - Existing `journal = "..."` line → replaced in place.
    /// - Existing commented `# journal = "..."` line → replaced (uncommented).
    /// - Neither present → key is appended near the top of the file.
    /// - File missing → seeded with the default template, then the key is set.
    ///
    /// Lines inside `[table]` sections are left alone — only the unsectioned
    /// preamble is searched, matching the layout produced by the default
    /// template.
    pub fn persist_journal(journal: &str) -> std::io::Result<PathBuf> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let original = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => DEFAULT_CONFIG.to_string(),
            Err(e) => return Err(e),
        };
        let updated = rewrite_journal_line(&original, journal);
        atomic_write(&path, &updated)?;
        Ok(path)
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

    /// Format an amount with currency symbol.
    ///
    /// Suffix (default): `"1.234,56 €"` / `"1,234.56 $"`
    /// Prefix:           `"€ 1.234,56"` / `"Eur 1.234,56"`
    pub fn fmt_amount(&self, amount: f64, decimals: usize) -> String {
        let n = self.fmt_number(amount, decimals);
        if self.currency_prefix {
            format!("{} {}", self.currency_symbol, n)
        } else {
            format!("{} {}", n, self.currency_symbol)
        }
    }

    /// Compact number with no currency symbol — e.g. `"1.234,56"`.
    /// Use for axis labels and other tight contexts where the symbol is shown
    /// elsewhere or would add visual noise.
    pub fn fmt_compact(&self, amount: f64, decimals: usize) -> String {
        self.fmt_number(amount, decimals)
    }

    /// Like `fmt_amount` but no space between number and symbol.
    ///
    /// Suffix: `"1.234,56€"` — Prefix: `"Eur1.234,56"`
    /// Used in tight spots (P/L cells, goal progress). For axis labels prefer
    /// `fmt_compact` so the symbol doesn't crowd the tick marks.
    pub fn fmt_amount_compact(&self, amount: f64, decimals: usize) -> String {
        let n = self.fmt_number(amount, decimals);
        if self.currency_prefix {
            format!("{}{}", self.currency_symbol, n)
        } else {
            format!("{}{}", n, self.currency_symbol)
        }
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

/// Rewrite the top-level `journal` key in `original`, returning the new file
/// body. Pure function — exposed (crate-private) for unit tests.
fn rewrite_journal_line(original: &str, journal: &str) -> String {
    let new_line = format!("journal = \"{}\"", escape_toml_string(journal));
    let mut out = String::with_capacity(original.len() + new_line.len());
    let mut wrote_key = false;
    let mut in_table = false;

    for line in original.split_inclusive('\n') {
        let trimmed = line.trim_start();

        if !in_table {
            let stripped = trimmed
                .strip_prefix('#')
                .map(|s| s.trim_start())
                .unwrap_or(trimmed);
            if stripped.starts_with("journal") && is_journal_assignment(stripped) {
                if !wrote_key {
                    out.push_str(&new_line);
                    out.push('\n');
                    wrote_key = true;
                }
                continue;
            }
        }

        if trimmed.starts_with('[') {
            in_table = true;
        }
        out.push_str(line);
    }

    if !wrote_key {
        // No matching line — insert near the top, after any leading comment
        // block, so the key shows up where users expect it.
        let insert_at = find_insertion_offset(original);
        let mut combined = String::with_capacity(original.len() + new_line.len() + 2);
        combined.push_str(&original[..insert_at]);
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(&new_line);
        combined.push('\n');
        combined.push_str(&original[insert_at..]);
        return combined;
    }

    out
}

/// Returns `true` when the line looks like `journal[ ]*=[ ]*...`. Keeps us
/// from accidentally rewriting a `journals = [...]` array.
fn is_journal_assignment(stripped: &str) -> bool {
    let rest = match stripped.strip_prefix("journal") {
        Some(r) => r,
        None => return false,
    };
    let rest = rest.trim_start();
    rest.starts_with('=')
}

/// Pick the byte offset where a freshly-added `journal = "…"` line should
/// land. Skips the file's leading comment / blank-line block so the key sits
/// at the top of the configuration proper, but never crosses into the first
/// `[table]` section.
fn find_insertion_offset(original: &str) -> usize {
    let mut offset = 0usize;
    for line in original.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            break;
        }
        if trimmed.starts_with('#') || trimmed.trim().is_empty() {
            offset += line.len();
            continue;
        }
        break;
    }
    offset
}

fn escape_toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

/// Write `content` to `path` via a sibling temp file + rename, so a crash
/// mid-write can never leave a half-written config behind.
fn atomic_write(path: &std::path::Path, content: &str) -> std::io::Result<()> {
    let tmp = match path.file_name() {
        Some(name) => {
            let mut tmp_name = std::ffi::OsString::from(".");
            tmp_name.push(name);
            tmp_name.push(".tmp");
            path.with_file_name(tmp_name)
        }
        None => path.with_extension("tmp"),
    };
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_existing_journal_line() {
        let input = "journal = \"/old/path.journal\"\nrefresh_interval = 60\n";
        let out = rewrite_journal_line(input, "/new/path.journal");
        assert_eq!(
            out,
            "journal = \"/new/path.journal\"\nrefresh_interval = 60\n"
        );
    }

    #[test]
    fn uncomments_commented_journal_line() {
        let input = "# journal = \"/path/old.journal\"\nrefresh_interval = 60\n";
        let out = rewrite_journal_line(input, "/path/new.journal");
        assert_eq!(
            out,
            "journal = \"/path/new.journal\"\nrefresh_interval = 60\n"
        );
    }

    #[test]
    fn appends_journal_when_missing() {
        let input = "# ldash configuration\n# Some banner.\n\nrefresh_interval = 60\n";
        let out = rewrite_journal_line(input, "/path/new.journal");
        assert!(
            out.contains("journal = \"/path/new.journal\""),
            "expected journal key in output, got: {out}"
        );
        assert!(
            out.contains("refresh_interval = 60"),
            "rest of file dropped: {out}"
        );
    }

    #[test]
    fn does_not_touch_journals_array() {
        let input = "journals = [\"~/a.journal\"]\n# journal = \"/x\"\n";
        let out = rewrite_journal_line(input, "/new.journal");
        // The `journals = [...]` array must survive unchanged.
        assert!(out.contains("journals = [\"~/a.journal\"]"), "got: {out}");
        // The commented `journal = ...` line should have been replaced.
        assert!(out.contains("journal = \"/new.journal\""), "got: {out}");
    }

    #[test]
    fn preserves_table_section_keys() {
        let input = "\n[budgets]\n\"expenses:Essen\" = 400.0\n";
        let out = rewrite_journal_line(input, "/new.journal");
        // Inserted before the [budgets] table, after the leading blank line.
        assert!(
            out.contains("journal = \"/new.journal\""),
            "expected journal key, got: {out}"
        );
        assert!(
            out.contains("[budgets]\n\"expenses:Essen\" = 400.0\n"),
            "table dropped: {out}"
        );
        // The journal line must precede the [budgets] header.
        let j = out.find("journal = ").unwrap();
        let b = out.find("[budgets]").unwrap();
        assert!(j < b, "journal line should appear before [budgets]: {out}");
    }

    #[test]
    fn escapes_special_characters() {
        let out = rewrite_journal_line("", "/path with \"quotes\".journal");
        assert!(
            out.contains("journal = \"/path with \\\"quotes\\\".journal\""),
            "got: {out}"
        );
    }

    #[test]
    fn replaces_only_first_journal_line() {
        // Defensive: if a user duplicated the key, only one canonical line
        // survives — duplicates are dropped.
        let input = "journal = \"/a\"\njournal = \"/b\"\nrefresh_interval = 60\n";
        let out = rewrite_journal_line(input, "/c");
        let count = out.matches("journal = ").count();
        assert_eq!(count, 1, "expected exactly one journal line, got: {out}");
        assert!(out.contains("journal = \"/c\""), "got: {out}");
    }

    #[test]
    fn persist_journal_round_trips_through_config_load() {
        // End-to-end: write a config via persist_journal, then re-parse it
        // with toml::from_str to confirm the journal key is recognised.
        let dir = std::env::temp_dir().join(format!(
            "ldash-config-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "refresh_interval = 60\n").unwrap();

        // Drive the rewrite through the pure helper; persist_journal itself
        // hits the global override which other tests may also touch.
        let original = std::fs::read_to_string(&path).unwrap();
        let updated = rewrite_journal_line(&original, "/path/to/x.journal");
        std::fs::write(&path, &updated).unwrap();

        let parsed: Config = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(parsed.journal.as_deref(), Some("/path/to/x.journal"));
        assert_eq!(parsed.refresh_interval, 60);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
