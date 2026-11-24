//! Errors, warnings and notes returned by Omega subsystems.
#![forbid(unsafe_code)]

use source_text::SourceSpan;
use std::fmt;
use std::path::PathBuf;

mod message;
mod provision;
mod text;

pub use message::{bracketed_display_names, comma_join_display, error_in_context, join_messages};
pub use provision::{CoverageForm, ExceededProvision};
pub use text::{WriteDiagnosticList, write_bounded_diagnostics, write_diagnostics, write_one_diagnostic};

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub stage: Option<DiagnosticStage>,
    pub build_context: Option<BuildDiagnosticContext>,
    pub exceeded_provision: Option<ExceededProvision>, // Present when refusal stopped at a named private provision (`Incomplete` outcome).
    pub source_span: Option<SourceSpan>, // Optional authored source location attached when declaration spans survive.
    pub resolved_location: Option<ResolvedSourceLocation>, // Span resolved against the compilation's retained source map while alive.
}

/// The source operation that first returned this diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum DiagnosticStage {
    Lexing,
    Parsing,
    Resolution,
    Typing,
    Preparation,
    Lowering,
    Checking,
}

impl DiagnosticStage {
    pub fn attribute(self, mut diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
        for diagnostic in &mut diagnostics {
            diagnostic.stage.get_or_insert(self);
        }
        diagnostics
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Lexing => "lexing",
            Self::Parsing => "parsing",
            Self::Resolution => "name resolution",
            Self::Typing => "typing",
            Self::Preparation => "source preparation",
            Self::Lowering => "lowering",
            Self::Checking => "lowered-form checking",
        }
    }
}

/// The innermost Build operation whose failure is being reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum BuildDiagnosticContext {
    PreparingDependency,
    ResumingBuild,
    RunningRegisteredTests,
}

impl BuildDiagnosticContext {
    pub fn label(self) -> &'static str {
        match self {
            Self::PreparingDependency => "preparing the dependency",
            Self::ResumingBuild => "resuming the Build after dependency preparation",
            Self::RunningRegisteredTests => "running registered tests",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct ResolvedSourceLocation {
    pub package: Option<String>,
    pub path: PathBuf,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum DiagnosticSeverity {
    Error,
    Warning, // Surfaced to the user without failing the build (Decision-12 relaxation for non-proof deadness).
    Note, // A second location of the diagnostic before it (the call an unreachable statement follows); never fails the build.
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            stage: None,
            build_context: None,
            exceeded_provision: None,
            source_span: None,
            resolved_location: None,
        }
    }

    pub fn incomplete(message: impl Into<String>, provision: ExceededProvision) -> Self {
        Self {
            exceeded_provision: Some(provision),
            ..Self::error(message)
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            ..Self::error(message)
        }
    }

    /// A further location of the diagnostic listed just before this one.
    pub fn note(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Note,
            ..Self::error(message)
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
            DiagnosticSeverity::Note => write!(formatter, "note: {}", self.message),
        }
    }
}
