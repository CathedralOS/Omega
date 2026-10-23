use std::fmt;
use std::path::PathBuf;

use source::SourceSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    /// Present when this refusal stopped at a named private provision instead
    /// of finding the source invalid: the compiler request's `Incomplete`
    /// outcome. The message still renders the refusal for people; outcome
    /// publication reads this record, never the wording.
    pub exceeded_provision: Option<ExceededProvision>,
    /// Optional authored source location. Semantic phases may attach this
    /// after frontend lowering when the relevant declaration span survives.
    pub source_span: Option<SourceSpan>,
    /// Resolved source attribution: the `source_span` interpreted against the
    /// compilation's retained source map at the boundary where that map is
    /// still alive. Error paths outside the map's reach read this record
    /// instead of rereading source storage.
    pub resolved_location: Option<ResolvedSourceLocation>,
}

/// A `source_span` resolved against the compilation's source map: the staged
/// path, its line:column, and the owning package's display name when the
/// compilation was package-aware. The package key is deliberately not carried
/// past this record — error paths name packages, they do not select them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSourceLocation {
    pub package: Option<String>,
    pub path: PathBuf,
    pub line: u32,
    pub column: u32,
}

/// A named private provision a compilation exceeded — the
/// [compiler request](../../../../../wiki/spec/build/compiler_request.md#validation-and-failures)'s
/// `Incomplete` outcome: a resource or search bound declined, which is no
/// verdict on the source it left unexamined.
///
/// `resource` is the scalar-resource name the outcome table publishes. `limit`
/// is the provisioned bound and `requested` the total demand the declined draw
/// placed on it, so a refusal has `requested > limit`. `coordinate` is the
/// primary coordinate in the space the resource's table row assigns — for the
/// proof-search provisions, the identity of the obligation left unproved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExceededProvision {
    pub resource: &'static str,
    pub limit: u64,
    pub requested: u64,
    pub coordinate: u64,
}

/// The compiler request's coverage provisions (scalar-resource codes 17 to
/// 23): a valid construct this compiler does not implement yet. Each has limit
/// zero and is anchored at the first canonical-source byte of the refused
/// construct, so a refusal at one names no verdict on the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CoverageForm {
    MachineHeader,
    EntrySignature,
    StateSignature,
    StatementForm,
    CallForm,
    ExpressionForm,
    TransitionForm,
}

impl CoverageForm {
    /// The resource name the outcome table publishes.
    pub const fn resource(self) -> &'static str {
        match self {
            Self::MachineHeader => "coverage_machine_headers",
            Self::EntrySignature => "coverage_entry_signatures",
            Self::StateSignature => "coverage_state_signatures",
            Self::StatementForm => "coverage_statement_forms",
            Self::CallForm => "coverage_call_forms",
            Self::ExpressionForm => "coverage_expression_forms",
            Self::TransitionForm => "coverage_transition_forms",
        }
    }

    /// One refused construct starting at canonical-source byte `first_byte`.
    pub const fn exceeded(self, first_byte: u64) -> ExceededProvision {
        ExceededProvision {
            resource: self.resource(),
            limit: 0,
            requested: 1,
            coordinate: first_byte,
        }
    }
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
            exceeded_provision: None,
            source_span: None,
            resolved_location: None,
        }
    }

    /// An error that stopped at `provision` rather than refuting the source.
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
