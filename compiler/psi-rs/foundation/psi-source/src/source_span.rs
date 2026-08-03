use crate::SourceId;
use crate::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceSpan {
    pub source_id: SourceId,
    pub span: Span,
}

impl SourceSpan {
    pub fn new(source_id: SourceId, span: Span) -> Self {
        Self { source_id, span }
    }
}
