#![forbid(unsafe_code)]

//! Psi-owned lexical analysis for Omega source text.

pub mod lex_error;
pub mod lexer;
pub mod observation;

pub use lex_error::LexError;
pub use lexer::{Lexer, Tokenization};
