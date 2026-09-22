//! Optimizer module role: executable entrance. Pre-allocation rule entrance.
//!
//! [`PRE_ALLOCATION_RULE_CATALOG`] is the only enable/order declaration for
//! this phase. Catalog rows compose their exact payloads; the copy-removal
//! family owns candidate admission and independent validation.

mod catalog;
mod execution;
mod model;
#[cfg(any(test, feature = "test-support"))]
mod test_support;

use optimization_core::{
    OptimizationExecutionPhase, OptimizationPhaseSelections, OptimizationSelections,
};

use super::catalog::selected_stage_catalog_contains;
use crate::StagedOptimizedAllocationLegality;

pub use catalog::{
    ORDERED_PRE_ALLOCATION_RULES, PRE_ALLOCATION_RULE_CATALOG, PreAllocationRuleCatalogEntry,
    PreAllocationRuleCatalogError, PreAllocationRuleCatalogPayload,
};
pub use execution::validate_pre_allocation_optimization_custody;
pub use model::{
    CopyRemovalPolicy, OptimizedCopyRemovalCustodyError, StagedOptimizedCopyRemovalAttempt,
    StagedOptimizedCopyRemovalAttemptReceipt, StagedOptimizedCopyRemovalIterationReceipt,
    StagedOptimizedCopyRemovalStep, StagedPreAllocationOptimizationCustodyReceipt,
    StagedPreAllocationOptimizationRun,
};
#[cfg(any(test, feature = "test-support"))]
pub use test_support::PreAllocationOptimizationCustodyFieldForTest;

impl From<PreAllocationRuleCatalogError> for OptimizedCopyRemovalCustodyError {
    fn from(error: PreAllocationRuleCatalogError) -> Self {
        match error {
            PreAllocationRuleCatalogError::WrongPhase(_) => Self::SelectionProjectionMismatch,
            PreAllocationRuleCatalogError::MissingSelection => {
                Self::MissingPreAllocationOptimization
            }
            PreAllocationRuleCatalogError::UnsupportedSelection(optimization) => {
                Self::UnsupportedPreAllocationOptimization(optimization)
            }
        }
    }
}

/// Resolve every pre-allocation catalog row in canonical catalog order.
///
/// The returned policy is a derived set of exact catalog payloads, never a
/// separately declared combined rule. Appending a row therefore requires an
/// explicit payload and cannot fall through an old whole-catalog special case.
pub fn resolve_pre_allocation_rules(
    selections: &OptimizationPhaseSelections,
) -> Result<(OptimizationSelections, CopyRemovalPolicy), PreAllocationRuleCatalogError> {
    let phase = selections
        .require_phase(OptimizationExecutionPhase::PreAllocation)
        .map_err(PreAllocationRuleCatalogError::WrongPhase)?;
    if phase.is_empty() {
        return Err(PreAllocationRuleCatalogError::MissingSelection);
    }
    if let Some(unsupported) = phase.as_slice().iter().find(|selected| {
        !selected_stage_catalog_contains(OptimizationExecutionPhase::PreAllocation, **selected)
    }) {
        return Err(PreAllocationRuleCatalogError::UnsupportedSelection(
            *unsupported,
        ));
    }
    let policy = PRE_ALLOCATION_RULE_CATALOG
        .iter()
        .filter(|entry| phase.contains(entry.optimization()))
        .fold(CopyRemovalPolicy::empty(), |policy, entry| {
            policy.union(entry.payload().policy())
        });
    Ok((phase.clone(), policy))
}

/// Execute the exact pre-allocation projection to a validated fixed point.
pub fn run_pre_allocation_optimizations(
    source: StagedOptimizedAllocationLegality,
) -> Result<StagedPreAllocationOptimizationRun, OptimizedCopyRemovalCustodyError> {
    let selections = source.selections().clone();
    let pre_allocation = selections.project_phase(OptimizationExecutionPhase::PreAllocation);
    let (selected, policy) = resolve_pre_allocation_rules(&pre_allocation)?;
    execution::execute_pre_allocation_optimizations(source, selections, selected, policy)
}
