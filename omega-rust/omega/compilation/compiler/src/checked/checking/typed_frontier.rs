// Developer harnesses can retain the frozen source the lowered-form checker
// receives even when judgment or later product preparation refuses. The shared
// SourcePreparationRequest returns that frontier directly after Psi 04; the
// product path copies it only when a caller supplies an optional capture slot.
// A captured frontier is source data, not a second judgment or an acceptance.

use products::{CheckedCompilation, CompileFailure};

/// What to check: either form the session already checks.
pub enum TypedFrontierRequest {
    /// A package Product, as `CompilationSession::check_package_build` takes it.
    Package(crate::PackageBuild),
    /// A standalone source and its returned Build, as
    /// `CompilationSession::complete_source_check` takes them.
    Source {
        source: super::SourceCompilation,
        returned: Result<Option<build_settings::ReturnedBuild>, CompileFailure>,
    },
}

pub use source_compiler::TypedFrontier;

/// What the product path concluded after receiving its typed source.
pub enum ProductCheckOutcome {
    /// The check the product path runs returned a checked compilation.
    Accepted(Box<CheckedCompilation>),
    /// Structural judgment or a later product step refused it.
    Rejected(CompileFailure),
}

/// The outcome of one check that kept its typed frontier.
pub enum TypedFrontierCheck {
    /// Psi 00 through 04 completed for the Product program.
    Reached {
        frontier: Box<TypedFrontier>,
        verdict: ProductCheckOutcome,
    },
    /// The compilation stopped before Psi 04 produced the Product's frozen
    /// typed trees: a parse error or another refusal in Psi 00 through 03, a
    /// Build that failed, or a refusal in Psi 04 itself. There are no trees to
    /// compare on.
    StoppedBeforeTypedFrontier(CompileFailure),
}

impl TypedFrontierCheck {
    /// Pair what the check returned with what it left in the slot. An
    /// accepted check that left no trees is reported as a refusal rather than
    /// as an acceptance with nothing to compare.
    pub(crate) fn join(
        frontier: Option<TypedFrontier>,
        result: Result<CheckedCompilation, CompileFailure>,
    ) -> Self {
        match (frontier, result) {
            (Some(frontier), Ok(checked)) => Self::Reached {
                frontier: Box::new(frontier),
                verdict: ProductCheckOutcome::Accepted(Box::new(checked)),
            },
            (Some(frontier), Err(failure)) => Self::Reached {
                frontier: Box::new(frontier),
                verdict: ProductCheckOutcome::Rejected(failure),
            },
            (None, Err(failure)) => Self::StoppedBeforeTypedFrontier(failure),
            (None, Ok(_)) => Self::StoppedBeforeTypedFrontier(
                vec![diagnostics::Diagnostic::error(
                    "the check accepted a program without retaining its typed frontier",
                )]
                .into(),
            ),
        }
    }
}
