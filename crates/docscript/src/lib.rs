//! DocScript — a code-first document language.
//!
//! Structure is expressed with explicit named calls (`p`, `h1`, `list`, `bold`),
//! text is written inside text blocks (`p { ... }`) with `{expr}` interpolation,
//! and documents have real variables, functions, conditionals and loops.

pub mod ast;
pub mod error;
pub mod eval;
pub mod lexer;
pub mod model;
pub mod parser;
pub mod theme;
pub mod value;

pub use error::Diagnostic;
pub use eval::{evaluate, evaluate_with_base};
pub use model::{
    Document, Image, ImageSize, Inline, ListItem, Node, ReferenceEntry, TableCell, TableRow,
};
pub use theme::{Align, PageStyle, Style, Theme};

pub fn compile(source: &str) -> Result<Document, Vec<Diagnostic>> {
    evaluate(source)
}

pub fn compile_with_base(
    source: &str,
    base_dir: Option<std::path::PathBuf>,
) -> Result<Document, Vec<Diagnostic>> {
    evaluate_with_base(source, base_dir)
}
