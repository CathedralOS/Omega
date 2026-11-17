//! Stage 00 turns loaded UTF-8 source into tokens for the parser.
//! File ingestion lives in `load_sources`; `lex_sources` schedules independent
//! sources; `lexer` dispatches each spelling to its lexical rule.

#![forbid(unsafe_code)]

mod lex_error;
mod lex_sources;
pub mod lexer;
mod load_sources;
mod numeric_literals;
mod quoted_literals;
mod source_cursor;

pub use lex_error::LexError;
pub use lex_sources::lex_sources;
pub use lexer::Lexer;
pub use load_sources::load_sources;
