use std::fmt;
use std::sync::Arc;

use crate::Span;

/// Logical token spelling or decoded literal bytes, independent of storage.
/// Authored provenance belongs to the containing token's kind and span; a
/// shared range only locates these bytes within their retained allocation.
#[derive(Clone)]
pub enum TokenText<'source> {
    Source(&'source str),
    Shared { source: Arc<str>, span: Span },
    Owned(String),
    OwnedBytes(Vec<u8>),
}

impl PartialEq for TokenText<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for TokenText<'_> {}

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

impl<'source> TokenText<'source> {
    pub fn source(value: &'source str) -> Self {
        Self::Source(value)
    }

    pub fn shared(source: Arc<str>, span: Span) -> Self {
        Self::Shared { source, span }
    }

    pub fn owned(value: String) -> Self {
        Self::Owned(value)
    }

    pub fn owned_bytes(value: Vec<u8>) -> Self {
        match String::from_utf8(value) {
            Ok(value) => Self::Owned(value),
            Err(error) => Self::OwnedBytes(error.into_bytes()),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Source(value) => value.as_bytes(),
            Self::Shared { source, span } => source[span.start..span.end].as_bytes(),
            Self::Owned(value) => value.as_bytes(),
            Self::OwnedBytes(value) => value.as_slice(),
        }
    }

    pub fn try_as_str(&self) -> Option<&str> {
        std::str::from_utf8(self.as_bytes()).ok()
    }

    pub fn as_str(&self) -> &str {
        self.try_as_str()
            .expect("token text is not valid UTF-8; use `as_bytes` for byte literals")
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

#[cfg(test)]
mod tests {
    use super::TokenText;
    use crate::{Span, Token, TokenKind};
    use std::sync::Arc;

    #[test]
    fn equality_observes_spelling_not_backing_storage() {
        let first = TokenText::shared(Arc::from("name first backing"), Span::new(0, 4));
        let second = TokenText::shared(Arc::from("other name backing"), Span::new(6, 10));
        assert_eq!(first, second);
        assert_eq!(first, TokenText::source("name"));
        assert_eq!(first, TokenText::owned("name".to_owned()));
        assert_eq!(first, TokenText::OwnedBytes(b"name".to_vec()));
        assert_ne!(first, TokenText::source("different"));
        assert_eq!(
            TokenText::OwnedBytes(vec![255]),
            TokenText::OwnedBytes(vec![255])
        );
        assert_ne!(
            TokenText::OwnedBytes(vec![255]),
            TokenText::OwnedBytes(vec![254])
        );
    }

    #[test]
    fn token_equality_keeps_kind_and_authored_span() {
        let token = Token {
            kind: TokenKind::Identifier,
            lexeme: TokenText::source("name"),
            span: Span::new(0, 4),
        };
        let mut other = token.clone();
        other.span = Span::new(6, 10);
        assert_ne!(token, other);
        other.span = token.span;
        other.kind = TokenKind::StringLiteral;
        assert_ne!(token, other);
    }

    #[test]
    fn debug_prints_only_logical_text_or_bytes() {
        let text = TokenText::shared(Arc::from("name unrelated source contents"), Span::new(0, 4));
        let debug = format!("{text:?}");
        assert!(debug.contains("name"));
        assert!(!debug.contains("unrelated source contents"));
        assert_eq!(debug, format!("{:?}", TokenText::source("name")));
        assert!(format!("{:?}", TokenText::OwnedBytes(vec![255])).contains("255"));
    }
}
