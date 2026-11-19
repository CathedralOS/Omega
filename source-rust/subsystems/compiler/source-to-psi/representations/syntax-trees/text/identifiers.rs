use crate::text_owner::{RetainedSource, SyntaxTextContext};
use arena::Handle;
use std::fmt;

use source_text::SourceSpan;

#[derive(Clone, Default, record_storage::Record)]
pub struct Identifier {
    text: IdentifierText,
    source_span: SourceSpan,
}

#[derive(Clone, Default, record_storage::Record)]
enum IdentifierText {
    #[default]
    Missing,
    Owned(Box<str>),
    SourceBacked {
        source: Handle<RetainedSource>,
    },
}

// Retained text handles name process-wide text generations, not the
// syntax page, so only the identifier's source span moves.
impl crate::SyntaxRelocate for Identifier {
    fn relocate(&mut self, relocation: &crate::PageRelocation) {
        crate::SyntaxRelocate::relocate(&mut self.source_span, relocation);
    }
}

impl Identifier {
    /// Joins a nonempty path from one source, retaining its full authored span.
    pub fn from_path(context: &impl SyntaxTextContext, members: &[Self]) -> Self {
        let first = members
            .first()
            .expect("joined identifier path must contain a name")
            .source_span();
        let last = members
            .last()
            .expect("joined identifier path must contain a name")
            .source_span();
        debug_assert_eq!(first.source_id, last.source_id);
        Self::new(
            members.joined_path_in(context),
            SourceSpan::new(
                first.source_id,
                source_text::Span::new(first.span.start, last.span.end),
            ),
        )
    }

    pub fn new(text: impl Into<String>, source_span: SourceSpan) -> Self {
        Self {
            text: IdentifierText::Owned(text.into().into_boxed_str()),
            source_span,
        }
    }

    pub fn generated(text: impl Into<String>) -> Self {
        Self {
            text: IdentifierText::Owned(text.into().into_boxed_str()),
            source_span: SourceSpan::default(),
        }
    }

    pub fn source_backed(source: Handle<RetainedSource>, source_span: SourceSpan) -> Self {
        Self {
            text: IdentifierText::SourceBacked { source },
            source_span,
        }
    }

    #[inline]
    pub fn as_str<'text>(&'text self, context: &'text impl SyntaxTextContext) -> &'text str {
        match &self.text {
            IdentifierText::Missing => "",
            IdentifierText::Owned(text) => text.as_ref(),
            IdentifierText::SourceBacked { source } => {
                let span = self.source_span.span;
                &context.source_text(*source)[span.start..span.end]
            }
        }
    }

    pub fn source_span(&self) -> SourceSpan {
        self.source_span
    }

    pub fn trailing_span(&self, length: usize) -> SourceSpan {
        let mut span = self.source_span();
        if span.span.end >= length {
            span.span.start = span.span.end - length;
        }
        span
    }

    pub fn is_self_receiver(&self, context: &impl SyntaxTextContext) -> bool {
        language_core::is_self_receiver(self.as_str(context))
    }

    pub fn is_source_backed(&self) -> bool {
        self.source_span.span.start != self.source_span.span.end
    }

    pub fn into_string(self, context: &impl SyntaxTextContext) -> String {
        match self.text {
            IdentifierText::Missing => String::new(),
            IdentifierText::Owned(text) => text.into_string(),
            IdentifierText::SourceBacked { source } => {
                let span = self.source_span.span;
                context.source_text(source)[span.start..span.end].to_owned()
            }
        }
    }
}

impl From<&str> for Identifier {
    fn from(text: &str) -> Self {
        Self::generated(text)
    }
}

impl fmt::Debug for Identifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut value = formatter.debug_struct("Identifier");
        match &self.text {
            IdentifierText::Missing => {
                value.field("text", &"");
            }
            IdentifierText::Owned(text) => {
                value.field("text", text);
            }
            IdentifierText::SourceBacked { source } => {
                value.field("source", source);
            }
        }
        value.field("source_span", &self.source_span).finish()
    }
}

pub trait JoinedSyntaxPath {
    fn joined_path_in(&self, context: &impl SyntaxTextContext) -> String {
        self.joined_path_with_in(context, "::")
    }
    fn joined_path_with_in(&self, context: &impl SyntaxTextContext, separator: &str) -> String;
}
impl JoinedSyntaxPath for [Identifier] {
    fn joined_path_with_in(&self, context: &impl SyntaxTextContext, separator: &str) -> String {
        let size = self
            .iter()
            .map(|name| name.as_str(context).len())
            .sum::<usize>()
            + separator.len().saturating_mul(self.len().saturating_sub(1));
        let mut result = String::with_capacity(size);
        for (index, name) in self.iter().enumerate() {
            if index != 0 {
                result.push_str(separator);
            }
            result.push_str(name.as_str(context));
        }
        result
    }
}
