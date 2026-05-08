use crate::Span;
use crate::source::FileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceSpan {
    pub file_id: FileId,
    pub span: Span,
}

impl SourceSpan {
    pub fn new(file_id: FileId, span: Span) -> Self {
        Self { file_id, span }
    }
}
