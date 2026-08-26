use psi_source::SourceSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub source_span: SourceSpan,
}

impl ParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source_span: SourceSpan::default(),
        }
    }

    pub fn at_source_span(message: impl Into<String>, source_span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            source_span,
        }
    }
}
