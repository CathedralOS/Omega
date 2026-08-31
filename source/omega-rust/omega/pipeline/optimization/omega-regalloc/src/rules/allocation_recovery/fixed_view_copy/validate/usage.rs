use omega_optimization_core::OptimizationWorkUsage;
use omega_target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::{FixedViewCopyError, FixedViewCopyPolicy, ValidatedAllocationLegality};

pub(super) fn replay_usage(
    selected: &ValidatedSelectedInstructions,
    legality: &ValidatedAllocationLegality,
    policy: FixedViewCopyPolicy,
) -> Result<OptimizationWorkUsage, FixedViewCopyError> {
    let functions = u64::try_from(selected.plan().functions.len())
        .map_err(|_| FixedViewCopyError::WorkOverflow)?;
    let requirements = legality
        .plan()
        .functions
        .iter()
        .flat_map(|function| &function.virtual_registers)
        .flat_map(|register| &register.entry_transitions)
        .try_fold(0_u64, |count, _| count.checked_add(1))
        .ok_or(FixedViewCopyError::WorkOverflow)?;
    let commits = match policy {
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1 => requirements,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => legality
            .plan()
            .functions
            .iter()
            .try_fold(0_u64, |count, function| {
                count.checked_add(u64::from(
                    function
                        .virtual_registers
                        .iter()
                        .any(|r| !r.entry_transitions.is_empty()),
                ))
            })
            .ok_or(FixedViewCopyError::WorkOverflow)?,
    };
    Ok(OptimizationWorkUsage {
        rule_evaluations: functions,
        candidates: requirements,
        validation_steps: requirements,
        commits,
        iterations: 1,
    })
}
