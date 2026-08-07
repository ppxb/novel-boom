//! Top-level TUI screens (navigation states).

/// Which full-screen view is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// Main menu + overview.
    Home,
    /// Read-only configuration dump.
    Config,
    /// Reserved feature with a simple placeholder body.
    Placeholder(&'static str),
}
