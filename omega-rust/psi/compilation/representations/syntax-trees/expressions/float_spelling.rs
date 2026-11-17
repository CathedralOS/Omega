use crate::{RetainedSource, SyntaxTextContext, SyntaxTextOwner};
use arena::Handle;
use source::SourceSpan;

#[derive(Clone, compiler_storage::Record)]
pub struct FloatSpelling {
    text: FloatSpellingText,
    source_span: SourceSpan,
}

// Retained text handles are page-independent; see `Identifier`.
impl crate::SyntaxRelocate for FloatSpelling {
    fn relocate(&mut self, relocation: &crate::PageRelocation) {
        crate::SyntaxRelocate::relocate(&mut self.source_span, relocation);
    }
}

#[derive(Clone, compiler_storage::Record)]
enum FloatSpellingText {
    Owned(Box<str>),
    SourceBacked(Handle<RetainedSource>),
    Materialized(numerics::literals::FloatLiteral),
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

    pub fn materialized(value: numerics::literals::FloatLiteral, source_span: SourceSpan) -> Self {
        Self {
            text: FloatSpellingText::Materialized(value),
            source_span,
        }
    }

    pub fn literal(
        &self,
        context: &impl SyntaxTextContext,
    ) -> Option<numerics::literals::FloatLiteral> {
        match &self.text {
            FloatSpellingText::Materialized(value) => Some(value.clone()),
            _ => numerics::literals::FloatLiteral::parse(self.as_str(context)),
        }
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
            FloatSpellingText::Materialized(value) => value.text(),
            FloatSpellingText::SourceBacked(source) => {
                let span = self.source_span.span;
                &context.source_text(*source)[span.start..span.end]
            }
        }
    }
}

impl crate::SyntaxEquivalent for FloatSpelling {
    fn same_syntax(&self, other: &Self, left: &SyntaxTextOwner, right: &SyntaxTextOwner) -> bool {
        self.as_str(left) == other.as_str(right)
            && match (&self.text, &other.text) {
                (FloatSpellingText::Materialized(left), FloatSpellingText::Materialized(right)) => {
                    left == right
                }
                (FloatSpellingText::Materialized(_), _)
                | (_, FloatSpellingText::Materialized(_)) => false,
                _ => true,
            }
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
            FloatSpellingText::Materialized(literal) => {
                value.field("materialized", literal);
            }
        }
        value.field("source_span", &self.source_span).finish()
    }
}

#[cfg(test)]
#[path = "../tests/unit/float_spelling_tests.rs"]
mod tests;
