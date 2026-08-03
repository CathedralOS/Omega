use std::fmt;

use psi_source::SourceSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    /// Optional authored source location. Semantic phases may attach this
    /// after frontend lowering when the relevant declaration span survives.
    pub source_span: Option<SourceSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    /// Surfaced to the user but never fails the build. Introduced for the
    /// Decision-12 relaxation (owner, 2026-07-12): a program must compile
    /// uniformly regardless of context -- deadness concerns outside proof
    /// contexts warn instead of rejecting.
    Warning,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            source_span: None,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
            source_span: None,
        }
    }

    pub fn with_source_span(mut self, source_span: SourceSpan) -> Self {
        self.source_span = Some(source_span);
        self
    }

    pub fn is_error(&self) -> bool {
        matches!(self.severity, DiagnosticSeverity::Error)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.severity {
            DiagnosticSeverity::Error => write!(formatter, "error: {}", self.message),
            DiagnosticSeverity::Warning => write!(formatter, "warning: {}", self.message),
        }
    }
}
