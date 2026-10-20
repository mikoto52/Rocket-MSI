#[cfg(windows)]
#[allow(dead_code)]
mod action_protocol;
#[cfg(windows)]
pub mod actions;
#[cfg(windows)]
pub mod cabinet;
#[cfg(windows)]
pub mod config;
#[cfg(windows)]
pub mod finish;
#[cfg(windows)]
pub mod license;
#[cfg(windows)]
pub mod msi;
#[cfg(windows)]
pub mod package;
#[cfg(windows)]
pub mod payload;
#[cfg(windows)]
pub mod project;
#[cfg(all(windows, test))]
mod quick_policy;
#[cfg(windows)]
pub mod registry;
#[cfg(windows)]
pub mod shortcut;
#[cfg(windows)]
pub mod signing;
#[cfg(windows)]
pub mod terms;
#[cfg(windows)]
pub mod upgrade;
