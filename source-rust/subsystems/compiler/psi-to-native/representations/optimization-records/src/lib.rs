//! Work budgets and the identities native optimization phases bind to.
//!
//! Each native optimization phase runs under an `OptimizationWorkBudget`,
//! reports its `OptimizationWorkUsage`, and names its inputs and results by the
//! digests in `identities`. The rule contracts, decision logs and pass
//! manifests of the abstract-operations optimizer are archived with native
//! stage 03 under `_recycling/`.
#![forbid(unsafe_code)]

mod budget;
mod identities;

pub use budget::{InvalidOptimizationWorkBudget, OptimizationWorkBudget, OptimizationWorkUsage};
pub use identities::{
    AcceptedObligationFactIdentity, CanonicalIdentityEncoder, NativeOptimizationProjectionIdentity,
    OptimizationUnitIdentity, OptimizationValidatorIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedOperatorOccurrenceIdentity,
};
pub use optimization_selections::{
    Optimization, OptimizationCatalogDescriptor, OptimizationExecutionPhase,
    OptimizationPhaseMismatch, OptimizationPhaseSelections, OptimizationSelectionIdentity,
    OptimizationSelections, PostTerminalOptimizationSelectionProjection,
    PostTerminalOptimizationSelections, PsiOptimizationSelectionProjection,
};
