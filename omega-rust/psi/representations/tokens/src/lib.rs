#![forbid(unsafe_code)]

//! Spelling-level tokens for Omega source files, owned by Psi.

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
