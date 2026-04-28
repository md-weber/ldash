//! Small style-construction macros to reduce `Style::default().fg(c)…` noise
//! across the render code. Behaviour-preserving sugar: each macro expands to
//! exactly the equivalent `Style::default()` chain.
//!
//! ```text
//! fg!(theme.muted)             // Style::default().fg(theme.muted)
//! fg!(theme.muted, bold)       // Style::default().fg(theme.muted).bold()
//! fg_bg!(theme.fg, theme.bg)   // Style::default().fg(...).bg(...)
//! ```

/// `Style::default().fg(<color>)` — optional `, bold` to also set bold.
#[macro_export]
macro_rules! fg {
    ($c:expr) => {
        ratatui::style::Style::default().fg($c)
    };
    ($c:expr, bold) => {
        ratatui::style::Style::default().fg($c).bold()
    };
}

/// `Style::default().bg(<color>)`.
#[macro_export]
macro_rules! bg {
    ($c:expr) => {
        ratatui::style::Style::default().bg($c)
    };
}

/// `Style::default().fg(<fg>).bg(<bg>)` — optional `, bold` suffix.
#[macro_export]
macro_rules! fg_bg {
    ($f:expr, $b:expr) => {
        ratatui::style::Style::default().fg($f).bg($b)
    };
    ($f:expr, $b:expr, bold) => {
        ratatui::style::Style::default().fg($f).bg($b).bold()
    };
}
