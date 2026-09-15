#![forbid(unsafe_code)]

//! Psi-owned lexical analysis for Omega source text.
//!
//! Start at `lexer.rs`: `Lexer::tokenize` turns one loaded source file into a
//! `Tokenization`, with numeric and string spellings handled beneath it.
//! `lex_error` owns the closed failure surface and `observation` the optional
//! per-file observations. No syntax is recognized at this stage.

pub mod lex_error;
pub mod lexer;
pub mod observation;

pub use lex_error::LexError;
pub use lexer::{Lexer, Tokenization};
