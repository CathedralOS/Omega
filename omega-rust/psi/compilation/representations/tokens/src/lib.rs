#![forbid(unsafe_code)]

/*
We represent the lexical token stream (`TokenStream`) and borrowed token views
(`Token`, `TokenText`) for Omega source files in Psi. Each compact `Token`
stores its `TokenKind`, authored `Span`, and an optional decoded-byte arena
handle inside the owning `TokenStream`.
*/

/*
We keep lexical storage stream-owned so ordinary tokens borrow their spelling
directly from the source buffer without per-token heap allocation. Only decoded
string or byte literals allocate into the stream's shared byte buffer, and
`TokenScratch` recycles unshared token and range buffers across files.
*/

/*
- Produced by: `Psi 00` (`00_source-files-to-tokens`).
- Consumed by: `Psi 01` (`01_tokens-to-syntax-trees`) and `package-manager`.
- Flat module layout:
  - `lib.rs`: `TokenStream`, `TokenScratch`, `LexedSource`, `LexedSources`, `Token`, and `TokenText`.
  - `token_kind.rs`: `TokenKind`, `NumericLiteralKind`, and `CommentKind`.
  - `keywords.rs`: `KeywordKind` and keyword spelling lookup.
  - `punctuation.rs`: `PunctuationKind` and longest-match punctuation dispatch.
*/

pub use source::Span;
pub use sources::{LexedSource, LexedSources};
pub use token::Token;
pub use token_kind::{
    CommentKind, FloatLiteralKind, IntegerLiteralKind, KeywordKind, NumericBase,
    NumericLiteralKind, PunctuationKind, TokenKind,
};
pub use token_text::TokenText;

pub mod sources {
    use crate::TokenStream;
    use arena::{Arena, HandleSpan};
    use source::{DependencyScope, SourceId, SourceOrigin};
    use std::path::PathBuf;
    use std::sync::Arc;

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct LexedSource {
        pub source_id: SourceId,
        pub path: PathBuf,
        pub source: Arc<str>,
        pub origin: Option<SourceOrigin>,
        pub tokens: TokenStream<'static>,
        pub scope: DependencyScope,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct LexedSources {
        pub sources: Arena<LexedSource>,
        pub batch: HandleSpan<LexedSource>,
    }
}
pub mod token {
    use super::DecodedRange;
    use crate::{
        CommentKind, FloatLiteralKind, IntegerLiteralKind, KeywordKind, NumericLiteralKind,
        PunctuationKind, Span, TokenKind,
    };
    use arena::Handle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Token {
        pub kind: TokenKind,
        pub(crate) decoded: Handle<DecodedRange>,
        pub span: Span,
    }

    impl Token {
        pub fn is_string_literal(&self) -> bool {
            matches!(self.kind, TokenKind::StringLiteral)
        }

        pub fn keyword(&self) -> Option<KeywordKind> {
            match self.kind {
                TokenKind::Keyword(keyword) => Some(keyword),
                _ => None,
            }
        }

        pub fn punctuation(&self) -> Option<PunctuationKind> {
            match self.kind {
                TokenKind::Punctuation(punctuation) => Some(punctuation),
                _ => None,
            }
        }

        pub fn integer_literal_kind(&self) -> Option<IntegerLiteralKind> {
            match self.kind {
                TokenKind::NumericLiteral(NumericLiteralKind::Integer(kind)) => Some(kind),
                _ => None,
            }
        }

        pub fn float_literal_kind(&self) -> Option<FloatLiteralKind> {
            match self.kind {
                TokenKind::NumericLiteral(NumericLiteralKind::Float(kind)) => Some(kind),
                _ => None,
            }
        }

        pub fn is_whitespace(&self) -> bool {
            matches!(self.kind, TokenKind::Whitespace)
        }

        pub fn comment(&self) -> Option<CommentKind> {
            match self.kind {
                TokenKind::Comment(kind) => Some(kind),
                _ => None,
            }
        }

        pub fn is_non_semantic(&self) -> bool {
            matches!(self.kind, TokenKind::Whitespace | TokenKind::Comment(_))
        }
    }
}
mod keywords;
mod punctuation;
pub mod token_kind;
pub mod token_text {
    use std::fmt;

    #[derive(Clone, Copy)]
    pub enum TokenText<'text> {
        Source(&'text str),
        Bytes(&'text [u8]),
    }

    impl PartialEq for TokenText<'_> {
        fn eq(&self, other: &Self) -> bool {
            self.as_bytes() == other.as_bytes()
        }
    }
    impl Eq for TokenText<'_> {}

    impl<'text> TokenText<'text> {
        #[inline]
        pub fn as_bytes(self) -> &'text [u8] {
            match self {
                Self::Source(text) => text.as_bytes(),
                Self::Bytes(bytes) => bytes,
            }
        }
        #[inline]
        pub fn try_as_str(self) -> Option<&'text str> {
            match self {
                Self::Source(text) => Some(text),
                Self::Bytes(bytes) => std::str::from_utf8(bytes).ok(),
            }
        }
        #[inline]
        pub fn as_str(self) -> &'text str {
            self.try_as_str()
                .expect("token text is not valid UTF-8; use `as_bytes` for byte literals")
        }
    }

    impl fmt::Debug for TokenText<'_> {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self.try_as_str() {
                Some(text) => formatter.debug_tuple("TokenText").field(&text).finish(),
                None => formatter
                    .debug_tuple("TokenBytes")
                    .field(&self.as_bytes())
                    .finish(),
            }
        }
    }

    impl PartialEq<&str> for TokenText<'_> {
        fn eq(&self, other: &&str) -> bool {
            self.as_bytes() == other.as_bytes()
        }
    }
    impl PartialEq<TokenText<'_>> for &str {
        fn eq(&self, other: &TokenText<'_>) -> bool {
            self.as_bytes() == other.as_bytes()
        }
    }
}

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
pub(crate) struct DecodedRange {
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

#[derive(Default)]
pub struct TokenScratch {
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

pub mod token_stream {
    pub use crate::*;
}
