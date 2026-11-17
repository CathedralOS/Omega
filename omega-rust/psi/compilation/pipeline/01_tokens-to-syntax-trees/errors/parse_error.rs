use source::SourceSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub source_span: SourceSpan,
    pub(crate) location: Option<(std::path::PathBuf, source::SourcePosition)>,
}

impl ParseError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source_span: SourceSpan::default(),
            location: None,
        }
    }

    pub(crate) fn at_source_span(message: impl Into<String>, source_span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            source_span,
            location: None,
        }
    }
}

impl ParseError {
    /// Formats the source location captured by the stage with its error message.
    pub fn into_diagnostic(self) -> diagnostics::Diagnostic {
        match self.location {
            Some((path, position)) => diagnostics::Diagnostic::error(format!(
                "{}:{}:{}: {}",
                path.display(),
                position.line,
                position.column,
                self.message
            )),
            None => diagnostics::Diagnostic::error(self.message),
        }
    }
}
