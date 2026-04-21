#[cfg(target_os = "linux")]
mod actions;
#[cfg(target_os = "linux")]
mod beachhead;
#[cfg(target_os = "linux")]
mod remote;
#[cfg(target_os = "linux")]
mod style;
#[cfg(target_os = "linux")]
mod terminal_adapter;
#[cfg(target_os = "linux")]
#[cfg(feature = "test-support")]
pub mod test_support;
#[cfg(target_os = "linux")]
mod ui;
#[cfg(target_os = "linux")]
mod widgets;

#[cfg(target_os = "linux")]
pub use ui::run;
