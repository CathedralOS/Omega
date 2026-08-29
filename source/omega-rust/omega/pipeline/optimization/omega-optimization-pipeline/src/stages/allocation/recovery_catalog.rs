//! Exact allocation-recovery rule catalog.

use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

/// Canonical allocation-recovery rule order.
pub const ORDERED_ALLOCATION_RECOVERY_RULES: [Optimization; 2] = [
    Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
    Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AllocationRecoveryRoute {
    SharedEntryFixedViewCopy,
    ActiveResidentImmediateRematerialization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AllocationRecoveryCatalogError {
    UnsupportedSelection(Optimization),
    UnsupportedComposition,
}

pub(crate) fn selected_allocation_recovery_route(
    selections: &OptimizationSelections,
) -> Result<Option<AllocationRecoveryRoute>, AllocationRecoveryCatalogError> {
    let phase = selections.for_phase(OptimizationExecutionPhase::AllocationRecovery);
    match phase.as_slice() {
        [] => Ok(None),
        [Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1] => {
            Ok(Some(AllocationRecoveryRoute::SharedEntryFixedViewCopy))
        }
        [Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1] => Ok(Some(
            AllocationRecoveryRoute::ActiveResidentImmediateRematerialization,
        )),
        [unsupported] => Err(AllocationRecoveryCatalogError::UnsupportedSelection(
            *unsupported,
        )),
        _ => Err(AllocationRecoveryCatalogError::UnsupportedComposition),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_catalog_covers_every_declared_allocation_recovery_optimization_once() {
        let declared = Optimization::ALL
            .into_iter()
            .filter(|optimization| {
                optimization.execution_phase() == OptimizationExecutionPhase::AllocationRecovery
            })
            .collect::<Vec<_>>();
        assert_eq!(declared, ORDERED_ALLOCATION_RECOVERY_RULES);
        assert_eq!(
            selected_allocation_recovery_route(
                &OptimizationSelections::new([
                    Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
                ])
                .unwrap(),
            ),
            Ok(Some(AllocationRecoveryRoute::SharedEntryFixedViewCopy))
        );
        assert_eq!(
            selected_allocation_recovery_route(
                &OptimizationSelections::new([
                    Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
                ])
                .unwrap(),
            ),
            Ok(Some(
                AllocationRecoveryRoute::ActiveResidentImmediateRematerialization
            ))
        );
        assert_eq!(
            selected_allocation_recovery_route(
                &OptimizationSelections::new(ORDERED_ALLOCATION_RECOVERY_RULES).unwrap(),
            ),
            Err(AllocationRecoveryCatalogError::UnsupportedComposition)
        );
    }
}
