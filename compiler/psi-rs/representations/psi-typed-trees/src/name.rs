use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

/// Transitional spelling wrapper.
///
/// This is deliberately owned text. Source-backed identifiers must not survive
/// past frontend lowering; resolved semantic/native identity should move to
/// symbol handles instead of carrying this type further down the pipeline.
#[derive(Clone, Default, Eq)]
pub struct Identifier {
    text: IdentifierText,
}

#[derive(Clone, Default, PartialEq, Eq)]
enum IdentifierText {
    #[default]
    Missing,
    Static(&'static str),
    Generated(Arc<str>),
}

impl Identifier {
    pub fn generated(text: impl Into<String>) -> Self {
        Self {
            text: IdentifierText::Generated(Arc::from(text.into().into_boxed_str())),
        }
    }

    pub fn generated_static(text: &'static str) -> Self {
        Self {
            text: IdentifierText::Static(text),
        }
    }

    pub fn as_str(&self) -> &str {
        match &self.text {
            IdentifierText::Missing => "",
            IdentifierText::Static(text) => text,
            IdentifierText::Generated(text) => text.as_ref(),
        }
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
        fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl PartialEq for Identifier {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}
