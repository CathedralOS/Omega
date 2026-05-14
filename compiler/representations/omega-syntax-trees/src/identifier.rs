use std::fmt;
use std::ops::Deref;

use omega_core::source::SourceSpan;

#[derive(Clone, Default, Eq)]
pub struct Identifier {
    text: IdentifierText,
    source_span: SourceSpan,
}

#[derive(Clone, Default, PartialEq, Eq)]
enum IdentifierText {
    #[default]
    Missing,
    Owned(String),
}

impl Identifier {
    pub fn new(text: impl Into<String>, source_span: SourceSpan) -> Self {
        Self {
            text: IdentifierText::Owned(text.into()),
            source_span,
        }
    }

    pub fn generated(text: impl Into<String>) -> Self {
        Self {
            text: IdentifierText::Owned(text.into()),
            source_span: SourceSpan::default(),
        }
    }

    pub fn as_str(&self) -> &str {
        match &self.text {
            IdentifierText::Missing => "",
            IdentifierText::Owned(text) => text.as_str(),
        }
    }

    pub fn source_span(&self) -> SourceSpan {
        self.source_span
    }

    pub fn is_source_backed(&self) -> bool {
        self.source_span.span.start != self.source_span.span.end
    }

    pub fn into_string(self) -> String {
        self.as_str().to_owned()
    }
}

impl From<&str> for Identifier {
    fn from(text: &str) -> Self {
        Self::generated(text)
    }
}

impl Deref for Identifier {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Debug for Identifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Identifier")
            .field("text", &self.as_str())
            .field("source_span", &self.source_span)
            .finish()
    }
}

impl PartialEq for Identifier {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<&str> for Identifier {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<str> for Identifier {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<Identifier> for &str {
    fn eq(&self, other: &Identifier) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<Identifier> for str {
    fn eq(&self, other: &Identifier) -> bool {
        self == other.as_str()
    }
}
