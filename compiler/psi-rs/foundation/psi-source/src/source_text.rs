use std::fmt;
use std::sync::Arc;

use super::SourceSpan;

#[derive(Clone, Default, Eq)]
pub struct SourceText {
    text: SourceTextStorage,
    source_span: SourceSpan,
}

#[derive(Clone, Default, PartialEq, Eq)]
enum SourceTextStorage {
    #[default]
    Missing,
    Shared(Arc<str>),
    SourceBacked {
        source: Arc<str>,
        source_span: SourceSpan,
    },
}

impl SourceText {
    pub fn new(text: impl Into<String>, source_span: SourceSpan) -> Self {
        Self {
            text: SourceTextStorage::Shared(Arc::from(text.into().into_boxed_str())),
            source_span,
        }
    }

    pub fn generated(text: impl Into<String>) -> Self {
        Self {
            text: SourceTextStorage::Shared(Arc::from(text.into().into_boxed_str())),
            source_span: SourceSpan::default(),
        }
    }

    pub fn source_backed(source: Arc<str>, source_span: SourceSpan) -> Self {
        Self {
            text: SourceTextStorage::SourceBacked {
                source,
                source_span,
            },
            source_span,
        }
    }

    pub fn as_str(&self) -> &str {
        match &self.text {
            SourceTextStorage::Missing => "",
            SourceTextStorage::Shared(text) => text.as_ref(),
            SourceTextStorage::SourceBacked {
                source,
                source_span,
            } => &source[source_span.span.start..source_span.span.end],
        }
    }

    pub fn shared_text(&self) -> Arc<str> {
        match &self.text {
            SourceTextStorage::Missing => Arc::from(""),
            SourceTextStorage::Shared(text) => Arc::clone(text),
            SourceTextStorage::SourceBacked { .. } => Arc::from(self.as_str()),
        }
    }

    pub fn is_source_backed(&self) -> bool {
        self.source_span.span.start != self.source_span.span.end
    }
}

impl From<&str> for SourceText {
    fn from(text: &str) -> Self {
        Self::generated(text)
    }
}

impl From<String> for SourceText {
    fn from(text: String) -> Self {
        Self::generated(text)
    }
}

impl fmt::Debug for SourceText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceText")
            .field("text", &self.as_str())
            .field("source_span", &self.source_span)
            .finish()
    }
}

impl fmt::Display for SourceText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq for SourceText {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}
