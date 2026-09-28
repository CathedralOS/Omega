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
mod source_queue;

pub use source_files::{
    lex_sources, load_injected_source, load_package_generated_source, load_sources,
};
pub use source_queue::ImportQueue;

pub use lexer::Lexer;
pub use lexer::lex_error::LexError;
