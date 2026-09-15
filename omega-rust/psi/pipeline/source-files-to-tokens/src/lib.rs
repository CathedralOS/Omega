#![forbid(unsafe_code)]

//! Psi-owned lexical analysis for Omega source text.
//!
//! Start at `lexer.rs`: `Lexer::tokenize` turns one loaded source file into a
//! `Tokenization`, with numeric and string spellings and the closed failure
//! surface handled beneath it. The `observation` folder owns the optional
//! per-file checkpoint observations. No syntax is recognized at this stage.

pub mod lexer;
pub mod observation;

pub use lexer::lex_error::LexError;
pub use lexer::{Lexer, Tokenization};
