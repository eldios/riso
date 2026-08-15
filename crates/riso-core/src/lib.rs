//! Core of `riso`: read a theme's palette, render templates from it, and write
//! the result where the desktop reads it.
//!
//! Nothing here runs a process or reads the environment, so every part is
//! testable without a graphical session.

pub mod apply;
pub mod atomic;
pub mod background;
pub mod builtin;
pub mod catalog;
pub mod color;
pub mod desktop;
pub mod error;
pub mod gradient;
pub mod palette;
pub mod plugin;
pub mod reload;
pub mod section;
pub mod template;
pub mod theme;

pub use apply::{apply, Applied, Request};
pub use color::Rgb;
pub use desktop::{Desktop, Payload};
pub use palette::{Palette, Warning};
pub use reload::{Executor, ProcessExecutor, RecordingExecutor};
pub use theme::{render_theme, Outcome, Report};
