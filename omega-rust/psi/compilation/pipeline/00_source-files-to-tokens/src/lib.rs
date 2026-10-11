#![forbid(unsafe_code)]

//! Psi-owned lexical analysis for Omega source text.
//!
//! Start at `source_files.rs`: `load_sources` reads an ordered frontier and
//! `lex_sources` produces retained token streams with each source's identity,
//! scope and origin unchanged. `lexer.rs` owns the per-file `Lexer::tokenize`
//! operation, also usable on borrowed snippets. Numeric and string spellings
//! and the closed failure surface live beneath it; no syntax is recognized here.

pub mod lexer;
mod source_files;

pub use source_files::{lex_sources, load_sources};

pub use lexer::Lexer;
pub use lexer::lex_error::LexError;
