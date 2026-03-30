//! SwiftTerm-backed terminal emulator bridge for macOS.
//!
//! This crate provides a type-safe Rust API over the Swift `ExatermTerminalBridge`
//! class, which wraps SwiftTerm's `TerminalView`. The bridge is compiled from Swift
//! sources and linked via `build.rs`.
//!
//! # Usage
//!
//! ```ignore
//! use exaterm_swiftterm::TerminalBridge;
//!
//! let bridge = TerminalBridge::new(frame);
//! let view = bridge.view(); // NSView to embed in your window
//! bridge.feed(b"Hello, terminal!\r\n");
//! ```

#[cfg(target_os = "macos")]
mod ffi;
#[cfg(target_os = "macos")]
mod terminal;

#[cfg(target_os = "macos")]
pub use terminal::{TerminalAppearance, TerminalBridge, TerminalSize};

#[cfg(not(target_os = "macos"))]
mod unsupported {
    use exaterm_ui::theme::Color;

    #[derive(Clone, Debug, PartialEq)]
    pub struct TerminalAppearance {
        pub font_name: String,
        pub font_size: f64,
        pub foreground: Color,
        pub background: Color,
        pub cursor: Color,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct TerminalSize {
        pub rows: u16,
        pub cols: u16,
    }

    pub struct TerminalBridge;
}

#[cfg(not(target_os = "macos"))]
pub use unsupported::{TerminalAppearance, TerminalBridge, TerminalSize};
