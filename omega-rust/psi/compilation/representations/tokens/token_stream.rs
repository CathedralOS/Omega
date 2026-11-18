//! Token storage retains source spelling and owns decoded literal bytes.
//! Start with `TokenStream`; its lexemes, spelling views and file batches follow below.

pub use source::Span;
pub use sources::{LexedSource, LexedSources};
pub use token::Token;
pub use token_kind::{
    CommentKind, FloatLiteralKind, IntegerLiteralKind, KeywordKind, NumericBase,
    NumericLiteralKind, PunctuationKind, TokenKind,
};

use arena::{Arena, Handle};
use std::{ops::Deref, sync::Arc};

#[derive(Debug, Clone)]
pub struct TokenStream<'source> {
    source: SourceBacking<'source>,
    tokens: Vec<Token>,
    decoded_ranges: Arena<DecodedRange>,
    decoded_bytes: Option<Arc<Vec<u8>>>,
}

#[derive(Default)]
pub struct TokenScratch {
    tokens: Vec<Token>,
    decoded_ranges: Arena<DecodedRange>,
    decoded_bytes: Option<Arc<Vec<u8>>>,
}

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
pub(crate) struct DecodedRange {
    start: usize,
    end: usize,
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

    pub fn with_scratch(source: &'source str, capacity: usize, scratch: &mut TokenScratch) -> Self {
        let mut tokens = std::mem::take(&mut scratch.tokens);
        tokens.reserve(capacity);
        Self {
            source: SourceBacking::Borrowed(source),
            tokens,
            decoded_ranges: std::mem::take(&mut scratch.decoded_ranges),
            decoded_bytes: scratch.decoded_bytes.take(),
        }
    }

    pub fn recycle(self, scratch: &mut TokenScratch) {
        let Self {
            source: _,
            mut tokens,
            mut decoded_ranges,
            mut decoded_bytes,
        } = self;
        tokens.clear();
        decoded_ranges.reset_retain_capacity();
        if let Some(bytes) = decoded_bytes.as_mut().and_then(Arc::get_mut) {
            bytes.clear();
        } else {
            decoded_bytes = None;
        }
        *scratch = TokenScratch {
            tokens,
            decoded_ranges,
            decoded_bytes,
        };
    }

    #[inline]
    pub fn push_source(&mut self, kind: TokenKind, span: Span) {
        assert!(
            kind != TokenKind::StringLiteral,
            "quoted literals require decoded bytes"
        );
        self.tokens.push(Token {
            kind,
            span,
            decoded: Handle::invalid(),
        });
    }

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

    /// Shares the exact source used to build this stream with a longer-lived consumer.
    pub fn shared_source(&self) -> Arc<str> {
        self.source_owner()
            .cloned()
            .unwrap_or_else(|| Arc::from(self.source_text()))
    }

    pub fn source_owner(&self) -> Option<&Arc<str>> {
        match &self.source {
            SourceBacking::Borrowed(_) => None,
            SourceBacking::Retained(source) => Some(source),
        }
    }

    /// The exact source bytes covered by this token, including a literal's quotes and escapes.
    #[inline]
    pub fn spelling(&self, token: &Token) -> &str {
        &self.source_text()[token.span.start..token.span.end]
    }

    /// The decoded contents of a quoted byte literal, which may contain arbitrary bytes.
    pub fn literal_bytes(&self, token: &Token) -> Option<source::SourceBytes> {
        if !token.decoded.is_valid() {
            return None;
        }
        let range = self.decoded_ranges.get(token.decoded);
        Some(source::SourceBytes::from_shared(
            self.decoded_bytes
                .as_ref()
                .expect("decoded payload has backing bytes")
                .clone(),
            range.start..range.end,
        ))
    }

    #[inline]
    pub fn as_slice(&self) -> &[Token] {
        &self.tokens
    }
}

impl PartialEq for TokenStream<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.source_text() == other.source_text()
            && self.len() == other.len()
            && self.iter().zip(other.iter()).all(|(left, right)| {
                left.kind == right.kind
                    && left.span == right.span
                    && self.literal_bytes(left) == other.literal_bytes(right)
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

#[path = "lexemes/keywords.rs"]
mod keywords;
#[path = "lexemes/punctuation.rs"]
mod punctuation;
#[path = "files/lexed.rs"]
pub mod sources;
#[path = "lexemes/token.rs"]
pub mod token;
#[path = "lexemes/kinds.rs"]
pub mod token_kind;
