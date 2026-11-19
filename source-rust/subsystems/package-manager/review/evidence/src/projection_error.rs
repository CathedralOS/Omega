//! Failures retained by package review for reporting source or policy projection.
use crate::encoding::PackageReviewEncodingError;
use diagnostics::Diagnostic;
use provider_acceptance::AcceptedSemanticBindingRole;

#[derive(Debug, Clone)]
pub enum SemanticBindingProjectionError {
    Ambiguous {
        role: AcceptedSemanticBindingRole,
        candidate_count: usize,
    },
    MissingCallableFacts {
        diagnostics: Vec<Diagnostic>,
    },
}

#[derive(Debug)]
pub enum PackageReviewProjectionError {
    SourceConsumptionMissing,
    Projection(Vec<Diagnostic>),
    IdentityMismatch,
    Encoding(PackageReviewEncodingError),
    PolicyBudget,
    SemanticBinding(SemanticBindingProjectionError),
}
