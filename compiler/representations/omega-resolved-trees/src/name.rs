use std::fmt;
use std::ops::Deref;

/// Diagnostic spelling wrapper.
///
/// This is deliberately owned text. Source-backed identifiers must not survive
/// past frontend lowering; symbol-resolved identity should move to symbol
/// handles instead of carrying this type further down the pipeline.
#[derive(Clone, Default, Eq)]
pub struct DiagnosticName {
    text: DiagnosticNameText,
}

#[derive(Clone, Default, PartialEq, Eq)]
enum DiagnosticNameText {
    #[default]
    Missing,
    Generated(String),
}

impl DiagnosticName {
    pub fn generated(text: impl Into<String>) -> Self {
        Self {
            text: DiagnosticNameText::Generated(text.into()),
        }
    }

    pub fn as_str(&self) -> &str {
        match &self.text {
            DiagnosticNameText::Missing => "",
            DiagnosticNameText::Generated(text) => text.as_str(),
        }
    }
}

impl From<&str> for DiagnosticName {
    fn from(text: &str) -> Self {
        Self::generated(text)
    }
}

impl Deref for DiagnosticName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for DiagnosticName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Debug for DiagnosticName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl PartialEq for DiagnosticName {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<&str> for DiagnosticName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<str> for DiagnosticName {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<DiagnosticName> for &str {
    fn eq(&self, other: &DiagnosticName) -> bool {
        *self == other.as_str()
    }
}
