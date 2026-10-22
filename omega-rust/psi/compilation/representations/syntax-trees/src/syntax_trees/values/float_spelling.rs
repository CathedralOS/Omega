//! Floating syntax keeps exact spelling until numeric admission.
//!
//! Authored occurrences borrow the forest's retained source page by handle;
//! they neither own a reference count nor copy the literal bytes. Generated
//! spellings own their bytes because a diagnostic span need not spell the
//! generated value. Access and equality require the owning forest, just as
//! unresolved names do. Numeric interpretation still belongs to resolution.

use crate::{RetainedSource, SyntaxTextContext, SyntaxTextOwner};
use arena::Handle;
use source::SourceSpan;

#[derive(Clone)]
pub struct FloatSpelling {
    text: FloatSpellingText,
    source_span: SourceSpan,
}

#[derive(Clone)]
enum FloatSpellingText {
    Owned(Box<str>),
    SourceBacked(Handle<RetainedSource>),
}

impl FloatSpelling {
    pub fn new(text: impl Into<String>, source_span: SourceSpan) -> Self {
        Self {
            text: FloatSpellingText::Owned(text.into().into_boxed_str()),
            source_span,
        }
    }

    pub fn generated(text: impl Into<String>) -> Self {
        Self::new(text, SourceSpan::default())
    }

    pub fn source_backed(source: Handle<RetainedSource>, source_span: SourceSpan) -> Self {
        Self {
            text: FloatSpellingText::SourceBacked(source),
            source_span,
        }
    }

    pub fn as_str<'text>(&'text self, context: &'text impl SyntaxTextContext) -> &'text str {
        match &self.text {
            FloatSpellingText::Owned(text) => text,
            FloatSpellingText::SourceBacked(source) => {
                let span = self.source_span.span;
                &context.source_text(*source)[span.start..span.end]
            }
        }
    }
}

impl crate::SyntaxEquivalent for FloatSpelling {
    fn same_syntax(&self, other: &Self, left: &SyntaxTextOwner, right: &SyntaxTextOwner) -> bool {
        // SourceText equality compared exact spelling, not diagnostic coordinates.
        self.as_str(left) == other.as_str(right)
    }
}

impl From<&str> for FloatSpelling {
    fn from(text: &str) -> Self {
        Self::generated(text)
    }
}

impl From<String> for FloatSpelling {
    fn from(text: String) -> Self {
        Self::generated(text)
    }
}

impl std::fmt::Debug for FloatSpelling {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut value = formatter.debug_struct("FloatSpelling");
        match &self.text {
            FloatSpellingText::Owned(text) => {
                value.field("text", text);
            }
            FloatSpellingText::SourceBacked(source) => {
                value.field("source", source);
            }
        }
        value.field("source_span", &self.source_span).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SyntaxEquivalent;
    use source::{SourceId, Span};
    use std::sync::Arc;

    #[test]
    fn equality_uses_exact_spelling_across_owners_and_generated_provenance() {
        let mut left = SyntaxTextOwner::default();
        let mut right = SyntaxTextOwner::default();
        let source = left.retain(Arc::from("1.25f32"));
        let other = right.retain(Arc::from("xx1.25f32"));
        let span = SourceSpan {
            source_id: SourceId(7),
            span: Span { start: 0, end: 7 },
        };
        let authored = FloatSpelling::source_backed(source, span);
        let shifted = FloatSpelling::source_backed(
            other,
            SourceSpan {
                source_id: SourceId(8),
                span: Span { start: 2, end: 9 },
            },
        );
        assert!(authored.same_syntax(&shifted, &left, &right));
        let generated = FloatSpelling::new("2.5f32", span);
        assert_eq!(
            generated.as_str(&left),
            "2.5f32",
            "provenance does not replace generated spelling"
        );
        assert!(!authored.same_syntax(&generated, &left, &right));
        assert!(authored.same_syntax(&FloatSpelling::generated("1.25f32"), &left, &right));
    }

    #[test]
    #[should_panic(expected = "does not belong to this forest")]
    fn another_owner_cannot_resolve_a_float_source_handle() {
        let mut owner = SyntaxTextOwner::default();
        let source = owner.retain(Arc::from("1.0"));
        let spelling = FloatSpelling::source_backed(
            source,
            SourceSpan {
                source_id: SourceId(1),
                span: Span { start: 0, end: 3 },
            },
        );
        spelling.as_str(&SyntaxTextOwner::default());
    }
}
