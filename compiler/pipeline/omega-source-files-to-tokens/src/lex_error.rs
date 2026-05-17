use omega_core::Span;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: Arc<str>,
    pub span: Span,
}

impl LexError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: Arc::from(message.into().into_boxed_str()),
            span,
        }
    }
}
