//! Format and syntax-highlight fk programs.
//!
//! This module is modular and composable: themes map token kinds to styles,
//! segment building turns source + tokens into (byte range, style) runs,
//! and the highlighter merges runs and emits styled output. The pretty-printer
//! formats the AST with indentation and line-breaking.

mod highlight;
mod pretty;
mod theme;

pub use highlight::{highlight, highlight_to_stderr};
pub use pretty::format_program;
pub use theme::{AnsiTheme, Style, Theme};
