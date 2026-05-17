use omega_core::source::SourceSpan;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: Arc<str>,
    pub source_span: SourceSpan,
}

impl ParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: Arc::from(message.into().into_boxed_str()),
            source_span: SourceSpan::default(),
        }
    }

    pub fn at_source_span(message: impl Into<String>, source_span: SourceSpan) -> Self {
        Self {
            message: Arc::from(message.into().into_boxed_str()),
            source_span,
        }
    }
}
