//! The one error a source file can produce in this layer: the first place it left the grammar.

use crate::errors::parse_error_kind::ParseErrorKind;
use omega_lexer::SourceSpan;
use std::error::Error;
use std::fmt;

/// The first place a token list stopped matching the grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseError {
    kind: ParseErrorKind,
    expected: &'static str, // What the parser was looking for, phrased to follow the word "expected".
    span: SourceSpan,       // The token that was found, or an empty range at the end of the source.
}

impl ParseError {
    pub fn new(
        kind: ParseErrorKind,
        expected: &'static str,
        span: SourceSpan,
    ) -> Self {
        Self { kind, expected, span }
    }

    pub fn get_kind(&self) -> ParseErrorKind {
        self.kind
    }

    pub fn get_expected(&self) -> &'static str {
        self.expected
    }

    pub fn get_span(&self) -> SourceSpan {
        self.span
    }
}

impl fmt::Display for ParseError {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(
            formatter,
            "{} (bytes {}..{})",
            self.kind.get_message(self.expected),
            self.span.get_start_offset(),
            self.span.get_end_offset()
        )
    }
}

impl Error for ParseError {}
