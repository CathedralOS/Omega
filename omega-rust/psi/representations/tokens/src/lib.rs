#![forbid(unsafe_code)]

//! Spelling-level tokens for Omega source files, owned by Psi.
//!
//! `TokenStream` is the lexer's output: each `Token` pairs a `TokenKind` with the
//! `Span` it was read from and, for literals and identifiers, its `TokenText`.
//! Keywords, punctuation, comments, and numeric bases are classified here;
//! nothing about grammar or meaning is decided at this level.

pub mod token_stream;
pub use token_stream::{token, token_kind, token_text};

pub use source::Span;
pub use token::Token;
pub use token_kind::{
    CommentKind, FloatLiteralKind, IntegerLiteralKind, KeywordKind, NumericBase,
    NumericLiteralKind, PunctuationKind, TokenKind,
};
pub use token_stream::TokenStream;
pub use token_text::TokenText;
