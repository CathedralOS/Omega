#![forbid(unsafe_code)]

//! Psi-owned lexical analysis for Omega source text.
//!
//! Start at `lexer.rs`: `Lexer::tokenize` turns one loaded source file into a
//! token stream, with numeric and string spellings and the closed failure
//! surface handled beneath it. No syntax is recognized at this stage.

pub mod lexer;

pub use lexer::Lexer;
pub use lexer::lex_error::LexError;
