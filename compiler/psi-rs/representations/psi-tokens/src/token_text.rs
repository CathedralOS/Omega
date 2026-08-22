use std::sync::Arc;

use crate::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenText<'source> {
    Source(&'source str),
    Shared { source: Arc<str>, span: Span },
    Owned(String),
    OwnedBytes(Vec<u8>),
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
