//! Compact token records borrow their spelling from one stream-owned source.
//!
//! Decoded strings occupy one byte buffer and a small arena of ranges. Ordinary
//! tokens carry the zero payload handle and use their authored source span.
//! Retaining a borrowed stream changes only its source owner: records and decoded
//! storage move unchanged. Parsing borrows this context and retains text only
//! when syntax escapes the token stream. No per-token ownership pass is needed.

pub mod sources;
pub mod token;
pub mod token_kind;
pub mod token_text;

use crate::{Span, Token, TokenKind, TokenText};
use arena::{Arena, Handle};
use std::{ops::Deref, sync::Arc};

#[derive(Debug, Clone)]
enum SourceBacking<'source> {
    Borrowed(&'source str),
    Retained(Arc<str>),
}

impl SourceBacking<'_> {
    fn text(&self) -> &str {
        match self {
            Self::Borrowed(text) => text,
            Self::Retained(text) => text,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct DecodedRange {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone)]
pub struct TokenStream<'source> {
    source: SourceBacking<'source>,
    tokens: Vec<Token>,
    decoded_ranges: Arena<DecodedRange>,
    decoded_bytes: Option<Arc<Vec<u8>>>,
}

impl Default for TokenStream<'_> {
    fn default() -> Self {
        Self::with_capacity("", 0)
    }
}

impl<'source> TokenStream<'source> {
    pub fn with_capacity(source: &'source str, token_capacity: usize) -> Self {
        Self {
            source: SourceBacking::Borrowed(source),
            tokens: Vec::with_capacity(token_capacity),
            decoded_ranges: Arena::default(),
            decoded_bytes: None,
        }
    }

    #[inline]
    pub fn push_source(&mut self, kind: TokenKind, span: Span) {
        self.tokens.push(Token {
            kind,
            span,
            decoded: Handle::invalid(),
        });
    }

    /// Decode directly into the stream's shared byte buffer. The callback must
    /// append exactly this literal's bytes; the lexer owns validation/decoding.
    pub fn push_decoded(&mut self, span: Span, decode: impl FnOnce(&mut Vec<u8>)) {
        let bytes = Arc::make_mut(
            self.decoded_bytes
                .get_or_insert_with(|| Arc::new(Vec::new())),
        );
        let start = bytes.len();
        decode(bytes);
        let decoded = self.decoded_ranges.append(DecodedRange {
            start,
            end: bytes.len(),
        });
        self.tokens.push(Token {
            kind: TokenKind::StringLiteral,
            span,
            decoded,
        });
    }

    /// Retain the exact source allocation used to lex this stream. Pointer and
    /// length identity establish the same byte coordinate system without a
    /// second scan or accepting an unrelated same-length source.
    pub fn retain_source(self, source: Arc<str>) -> TokenStream<'static> {
        assert!(
            std::ptr::eq(self.source.text(), source.as_ref()),
            "retained source must be the exact text used to lex this stream"
        );
        TokenStream {
            source: SourceBacking::Retained(source),
            tokens: self.tokens,
            decoded_ranges: self.decoded_ranges,
            decoded_bytes: self.decoded_bytes,
        }
    }

    pub fn source_text(&self) -> &str {
        self.source.text()
    }

    pub fn source_owner(&self) -> Option<&Arc<str>> {
        match &self.source {
            SourceBacking::Borrowed(_) => None,
            SourceBacking::Retained(source) => Some(source),
        }
    }

    #[inline]
    pub fn text(&self, token: &Token) -> TokenText<'_> {
        if token.decoded.is_valid() {
            let range = self.decoded_ranges.get(token.decoded);
            TokenText::Bytes(
                &self
                    .decoded_bytes
                    .as_ref()
                    .expect("decoded payload has backing bytes")[range.start..range.end],
            )
        } else {
            TokenText::Source(&self.source.text()[token.span.start..token.span.end])
        }
    }

    /// Retain the decoded range without copying its bytes. The stream and all
    /// syntax/semantic consumers share one immutable source-local buffer.
    pub fn literal_bytes(&self, token: &Token) -> source::SourceBytes {
        if !token.decoded.is_valid() {
            return source::SourceBytes::from(self.text(token).as_bytes());
        }
        let range = self.decoded_ranges.get(token.decoded);
        source::SourceBytes::from_shared(
            self.decoded_bytes
                .as_ref()
                .expect("decoded payload has backing bytes")
                .clone(),
            range.start..range.end,
        )
    }

    #[inline]
    pub fn as_slice(&self) -> &[Token] {
        &self.tokens
    }
}

impl PartialEq for TokenStream<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self.iter().zip(other.iter()).all(|(left, right)| {
                left.kind == right.kind
                    && left.span == right.span
                    && self.text(left) == other.text(right)
            })
    }
}
impl Eq for TokenStream<'_> {}

impl Deref for TokenStream<'_> {
    type Target = [Token];
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_literals_share_storage_and_survive_token_mutation() {
        let mut stream = TokenStream::default();
        stream.push_decoded(Span::default(), |bytes| bytes.extend_from_slice(&[0, 255]));
        stream.push_decoded(Span::default(), |_| {});
        let first = stream.literal_bytes(&stream[0]);
        let empty = stream.literal_bytes(&stream[1]);
        assert_eq!(first.as_ptr(), stream.text(&stream[0]).as_bytes().as_ptr());
        let original = first.as_ptr();
        stream.push_decoded(Span::default(), |bytes| bytes.extend_from_slice(b"more"));
        assert_eq!(first.as_ptr(), original);
        assert_eq!(first.as_ref(), &[0, 255]);
        drop(stream);
        assert_eq!(first.as_ref(), &[0, 255]);
        assert!(empty.is_empty());
    }

    #[test]
    fn retained_stream_moves_records_and_owns_one_source_reference() {
        let source: Arc<str> = Arc::from("alpha beta");
        let mut stream = TokenStream::with_capacity(&source, 2);
        stream.push_source(TokenKind::Identifier, Span::new(0, 5));
        stream.push_source(TokenKind::Identifier, Span::new(6, 10));
        let records = stream.as_ptr();
        let retained = stream.retain_source(source.clone());
        assert_eq!(retained.as_ptr(), records);
        assert_eq!(Arc::strong_count(&source), 2);
        let cloned = retained.clone();
        assert_eq!(Arc::strong_count(&source), 3);
        drop(source);
        drop(retained);
        assert_eq!(cloned.text(&cloned[0]), "alpha");
        assert_eq!(cloned.text(&cloned[1]), "beta");
    }

    #[test]
    fn decoded_ranges_survive_growth_clone_and_empty_payloads() {
        let mut stream = TokenStream::default();
        for count in 0..100 {
            stream.push_decoded(Span::default(), |bytes| {
                bytes.extend(std::iter::repeat_n(255, count));
            });
        }
        let cloned = stream.clone();
        assert_eq!(cloned, stream);
        drop(stream);
        for (count, token) in cloned.iter().enumerate() {
            assert_eq!(cloned.text(token).as_bytes(), vec![255; count]);
        }
        assert_eq!(cloned.text(&cloned[0]).as_str(), "");
        assert_eq!(cloned.text(&cloned[1]).try_as_str(), None);
    }

    #[test]
    #[should_panic(expected = "retained source must be the exact text")]
    fn cannot_retain_an_unrelated_equal_length_source() {
        let stream = TokenStream::with_capacity("alpha", 0);
        let _ = stream.retain_source(Arc::from("other"));
    }

    #[test]
    fn stream_equality_observes_spelling_and_spans_not_source_ownership() {
        let source: Arc<str> = Arc::from("alpha");
        let mut borrowed = TokenStream::with_capacity(&source, 1);
        borrowed.push_source(TokenKind::Identifier, Span::new(0, 5));
        let retained = borrowed.clone().retain_source(source.clone());
        assert_eq!(borrowed, retained);
        let mut other = TokenStream::with_capacity("bravo", 1);
        other.push_source(TokenKind::Identifier, Span::new(0, 5));
        assert_ne!(borrowed, other);
        assert!(!format!("{:?}", borrowed.text(&borrowed[0])).contains("bravo"));
    }
}
