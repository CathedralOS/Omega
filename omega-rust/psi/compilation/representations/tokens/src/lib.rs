#![forbid(unsafe_code)]

//! Spelling-level tokens for Omega source files, owned by Psi.
//!
//! `TokenStream` owns the source context and decoded byte storage. Each compact
//! `Token` carries its kind, authored span and optional decoded payload handle;
//! `TokenText` is a borrowed view resolved through the stream.
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
pub use token_stream::sources::{LexedSource, LexedSources};
pub use token_text::TokenText;
