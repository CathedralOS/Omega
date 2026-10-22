//! An unresolved spelling, addressed through its forest's retained source page.
//!
//! Authored occurrences carry a generational page handle and diagnostic span;
//! copying them does not retain a source allocation per occurrence. Reading the
//! spelling requires the owner, so a detached name cannot expose freed storage.
//! Compiler-composed spellings own their bytes because they can differ from the
//! authored span. They move on consumption and copy only when explicitly cloned.
//! Name equality is contextual and compares spelling, not page or span identity.

use super::text_owner::{RetainedSource, SyntaxTextContext};
use arena::Handle;
use std::fmt;

use source::SourceSpan;

#[derive(Clone, Default)]
pub struct Identifier {
    text: IdentifierText,
    source_span: SourceSpan,
}

#[derive(Clone, Default)]
enum IdentifierText {
    #[default]
    Missing,
    Owned(Box<str>),
    SourceBacked {
        source: Handle<RetainedSource>,
    },
}

impl Identifier {
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

    /// This identifier's span narrowed to its trailing `length` source bytes —
    /// the span a final path segment occupies when the path is dotted.
    pub fn trailing_span(&self, length: usize) -> SourceSpan {
        let mut span = self.source_span();
        if span.span.end >= length {
            span.span.start = span.span.end - length;
        }
        span
    }

    /// Whether this identifier spells the `self` receiver binding of an
    /// attached machine or state; see `language_core::receiver_binding`.
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

/// Join unresolved spelling through the forest that owns its source pages.
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
