#![forbid(unsafe_code)]

//! Psi-owned lexical analysis for Omega source text: source files in, tokens out.
//!
//! The stage has two steps, one module each. `load_sources.rs` takes in the
//! input, reading an ordered list of files into memory. Source ownership assigns
//! coordinates to those bytes before `lex_sources.rs` turns them into retained
//! token streams with each source's identity,
//! scope and origin unchanged; in-memory inputs start there. `lexer.rs` owns
//! the per-text `Lexer::tokenize` operation, also usable on borrowed snippets.
//! Numeric and string spellings and the closed failure surface live beneath
//! it; no syntax is recognized here.

mod lex_sources;
pub mod lexer;
mod load_sources;

pub use lex_sources::lex_sources;
pub use load_sources::load_sources;

pub use lexer::Lexer;
pub use lexer::lex_error::LexError;
