use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use source::SourceSpan;

/// Diagnostic spelling wrapper.
///
/// This is deliberately owned text. Source-backed identifiers must not survive
/// past frontend lowering; symbol-resolved identity should move to symbol
/// handles instead of carrying this type further down the pipeline.
#[derive(Clone, Default, Eq)]
pub struct DiagnosticName {
    text: DiagnosticNameText,
    source_span: SourceSpan,
}

#[derive(Clone, Default, PartialEq, Eq)]
enum DiagnosticNameText {
    #[default]
    Missing,
    Static(&'static str),
    Generated(Arc<str>),
}

impl DiagnosticName {
    pub fn new(text: impl Into<String>, source_span: SourceSpan) -> Self {
        Self::from_str(&text.into(), source_span)
    }

    /// Copies borrowed spelling into shared owned storage, retaining its provenance.
    pub fn from_str(text: &str, source_span: SourceSpan) -> Self {
        Self {
            text: DiagnosticNameText::Generated(Arc::from(text)),
            source_span,
        }
    }

    pub fn generated(text: impl Into<String>) -> Self {
        Self::new(text, SourceSpan::default())
    }

    pub fn generated_static(text: &'static str) -> Self {
        Self {
            text: DiagnosticNameText::Static(text),
            source_span: SourceSpan::default(),
        }
    }

    pub fn as_str(&self) -> &str {
        match &self.text {
            DiagnosticNameText::Missing => "",
            DiagnosticNameText::Static(text) => text,
            DiagnosticNameText::Generated(text) => text.as_ref(),
        }
    }

    pub fn source_span(&self) -> SourceSpan {
        self.source_span
    }

    pub fn is_source_backed(&self) -> bool {
        self.source_span.span.start != self.source_span.span.end
    }
}

impl From<&str> for DiagnosticName {
    fn from(text: &str) -> Self {
        Self::from_str(text, SourceSpan::default())
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

#[cfg(test)]
mod tests {
    use super::DiagnosticName;
    use source::{SourceSpan, Span};

    #[test]
    fn copied_names_own_spelling_and_preserve_provenance() {
        let mut spelling = String::from("naïve::name");
        let source_span = SourceSpan::new(Default::default(), Span::new(7, 19));
        let name = DiagnosticName::from_str(&spelling, source_span);
        let cloned = name.clone();
        assert_ne!(name.as_str().as_ptr(), spelling.as_ptr());
        assert_eq!(name.as_str().as_ptr(), cloned.as_str().as_ptr());
        spelling.clear();
        drop(spelling);

        assert_eq!(name.as_str(), "naïve::name");
        assert_eq!(name.source_span(), source_span);
        assert!(name.is_source_backed());
        assert_eq!(name, DiagnosticName::generated_static("naïve::name"));
        assert_eq!(name.to_string(), "naïve::name");
        assert_eq!(format!("{name:?}"), "\"naïve::name\"");
        assert_eq!(DiagnosticName::from(""), DiagnosticName::default());
    }

    #[test]
    fn constructors_keep_string_conversion_compatibility() {
        struct Spelling;
        impl From<Spelling> for String {
            fn from(_: Spelling) -> Self {
                String::from("generated")
            }
        }

        assert_eq!(
            DiagnosticName::new(Spelling, SourceSpan::default()),
            "generated"
        );
        assert_eq!(DiagnosticName::generated(Spelling), "generated");
        assert_eq!(DiagnosticName::from("generated"), "generated");
    }

    #[test]
    fn static_generated_names_do_not_allocate_owned_text() {
        let name = DiagnosticName::generated_static("self");

        assert_eq!(name.as_str(), "self");
        assert_eq!(name, "self");
        assert!(!name.is_source_backed());
    }
}
