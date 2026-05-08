use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use omega_core::source::SourceSpan;

#[derive(Clone, Default, Eq)]
pub struct ProgramName {
    text: ProgramNameText,
    source_span: SourceSpan,
}

#[derive(Clone, Default, PartialEq, Eq)]
enum ProgramNameText {
    #[default]
    Missing,
    Source(Arc<str>),
    Generated(String),
}

impl ProgramName {
    pub fn generated(text: impl Into<String>) -> Self {
        Self {
            text: ProgramNameText::Generated(text.into()),
            source_span: SourceSpan::default(),
        }
    }

    pub fn source(source: Arc<str>, source_span: SourceSpan) -> Self {
        Self {
            text: ProgramNameText::Source(source),
            source_span,
        }
    }

    pub fn as_str(&self) -> &str {
        match &self.text {
            ProgramNameText::Missing => "",
            ProgramNameText::Source(source) => source
                .get(self.source_span.span.start..self.source_span.span.end)
                .unwrap_or(""),
            ProgramNameText::Generated(text) => text.as_str(),
        }
    }

    pub fn is_source_backed(&self) -> bool {
        matches!(self.text, ProgramNameText::Source(_))
    }
}

impl From<&str> for ProgramName {
    fn from(text: &str) -> Self {
        Self::generated(text)
    }
}

impl From<String> for ProgramName {
    fn from(text: String) -> Self {
        Self::generated(text)
    }
}

impl Deref for ProgramName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for ProgramName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Debug for ProgramName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProgramName")
            .field("text", &self.as_str())
            .field("source_span", &self.source_span)
            .finish()
    }
}

impl PartialEq for ProgramName {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<&str> for ProgramName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<str> for ProgramName {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<String> for ProgramName {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<ProgramName> for &str {
    fn eq(&self, other: &ProgramName) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<ProgramName> for String {
    fn eq(&self, other: &ProgramName) -> bool {
        self.as_str() == other.as_str()
    }
}
