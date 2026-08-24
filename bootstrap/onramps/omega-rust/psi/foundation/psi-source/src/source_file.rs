use std::path::PathBuf;
use std::sync::Arc;

use crate::{SourceId, SourceSpan, Span};
use psi_core::PackageKeyIdentity;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SourceOrigin {
    #[default]
    User,
    Toolchain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub source_id: SourceId,
    pub path: PathBuf,
    /// Canonical package directory selected by the frontend. Source files are
    /// package members by location, so ownership must not be inferred from a
    /// declaration spelling later in the pipeline.
    pub package_root: PathBuf,
    /// Stable package identity supplied by a reconciled package graph. Paths
    /// remain source-loading locations and diagnostic context; they are not
    /// nominal identity. Transitional, toolchain, and standalone sources may
    /// not have an admitted package identity yet.
    pub package_identity: Option<PackageKeyIdentity>,
    pub origin: SourceOrigin,
    pub source: Arc<str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

impl SourceFile {
    pub fn source_span(&self, span: Span) -> SourceSpan {
        SourceSpan::new(self.source_id, span)
    }

    pub fn text_at(&self, span: Span) -> &str {
        self.source.get(span.start..span.end).unwrap_or("")
    }

    pub fn position_at(&self, byte_offset: usize) -> SourcePosition {
        SourcePosition::of(&self.source, byte_offset)
    }
}

impl SourcePosition {
    pub fn of(source: &str, byte_offset: usize) -> Self {
        let mut line = 1;
        let mut column = 1;

        for (index, character) in source.char_indices() {
            if index >= byte_offset {
                break;
            }

            if character == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }

        SourcePosition { line, column }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::{SourceFile, SourceId, SourceOrigin, Span};

    #[test]
    fn source_span_carries_source_identity() {
        let file = SourceFile {
            source_id: SourceId(7),
            path: PathBuf::from("main.omg"),
            package_root: PathBuf::from("."),
            package_identity: None,
            origin: SourceOrigin::User,
            source: Arc::from("machine main {}"),
        };
        let span = file.source_span(Span::new(8, 12));

        assert_eq!(span.source_id, SourceId(7));
        assert_eq!(file.text_at(span.span), "main");
    }
}
