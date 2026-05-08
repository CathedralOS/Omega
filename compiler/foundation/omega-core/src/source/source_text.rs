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
    Source(Arc<str>),
    Generated(String),
}

impl SourceText {
    pub fn source(source: Arc<str>, source_span: SourceSpan) -> Self {
        Self {
            text: SourceTextStorage::Source(source),
            source_span,
        }
    }

    pub fn generated(text: impl Into<String>) -> Self {
        Self {
            text: SourceTextStorage::Generated(text.into()),
            source_span: SourceSpan::default(),
        }
    }

    pub fn as_str(&self) -> &str {
        match &self.text {
            SourceTextStorage::Missing => "",
            SourceTextStorage::Source(source) => source
                .get(self.source_span.span.start..self.source_span.span.end)
                .unwrap_or(""),
            SourceTextStorage::Generated(text) => text.as_str(),
        }
    }

    pub fn is_source_backed(&self) -> bool {
        matches!(self.text, SourceTextStorage::Source(_))
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
