//! The one error a file can produce: the first lexical rule it broke, and where.

use crate::errors::lex_error_kind::LexErrorKind;
use crate::locations::source_span::SourceSpan;
use std::error::Error;
use std::fmt;

/// The first lexical rule a source file broke, and the bytes that broke it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LexError {
    kind: LexErrorKind,
    span: SourceSpan,
}

impl LexError {
    pub fn new(
        kind: LexErrorKind,
        span: SourceSpan,
    ) -> Self {
        Self { kind, span }
    }

    pub fn get_kind(&self) -> LexErrorKind {
        self.kind
    }

    pub fn get_span(&self) -> SourceSpan {
        self.span
    }
}

impl fmt::Display for LexError {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(
            formatter,
            "{} (bytes {}..{})",
            self.kind.get_message(),
            self.span.get_start_offset(),
            self.span.get_end_offset()
        )
    }
}

impl Error for LexError {}
