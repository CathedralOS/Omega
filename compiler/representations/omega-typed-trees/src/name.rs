use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

/// Transitional spelling wrapper.
///
/// This is deliberately owned text. Source-backed identifiers must not survive
/// past frontend lowering; resolved semantic/native identity should move to
/// symbol handles instead of carrying this type further down the pipeline.
#[derive(Clone, Default, Eq)]
pub struct ProgramName {
    text: ProgramNameText,
}

#[derive(Clone, Default, PartialEq, Eq)]
enum ProgramNameText {
    #[default]
    Missing,
    Static(&'static str),
    Generated(Arc<str>),
}

impl ProgramName {
    pub fn generated(text: impl Into<String>) -> Self {
        Self {
            text: ProgramNameText::Generated(Arc::from(text.into().into_boxed_str())),
        }
    }

    pub fn generated_static(text: &'static str) -> Self {
        Self {
            text: ProgramNameText::Static(text),
        }
    }

    pub fn as_str(&self) -> &str {
        match &self.text {
            ProgramNameText::Missing => "",
            ProgramNameText::Static(text) => text,
            ProgramNameText::Generated(text) => text.as_ref(),
        }
    }
}

impl From<&str> for ProgramName {
    fn from(text: &str) -> Self {
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
        fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl PartialEq for ProgramName {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}
