//! Core of `riso`: read a theme's palette, render templates from it, and write
//! the result where the desktop reads it.
//!
//! Nothing here runs a process or reads the environment, so every part is
//! testable without a graphical session.

pub mod atomic;
pub mod color;
pub mod error;
pub mod gradient;
pub mod palette;
pub mod section;
pub mod template;
pub mod theme;

pub use color::Rgb;
pub use palette::{Palette, Warning};
pub use theme::{render_theme, Outcome, Report};
