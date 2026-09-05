use optimization_core::OptimizationWorkUsage;
use target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use std::collections::BTreeSet;

use crate::{FixedViewCopyError, FixedViewCopyPolicy};

pub(super) fn replay_usage(
    selected: &ValidatedSelectedInstructions,
    boundaries: &[crate::rewrites::allocation_recovery::fixed_view_copy::evidence::AuthenticatedFixedViewBoundary],
    policy: FixedViewCopyPolicy,
) -> Result<OptimizationWorkUsage, FixedViewCopyError> {
    let functions = u64::try_from(selected.plan().functions.len())
        .map_err(|_| FixedViewCopyError::WorkOverflow)?;
    let requirements =
        u64::try_from(boundaries.len()).map_err(|_| FixedViewCopyError::WorkOverflow)?;
    let commits = match policy {
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1 => requirements,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => boundaries
            .iter()
            .map(|boundary| boundary.function)
            .collect::<BTreeSet<_>>()
            .len()
            .try_into()
            .map_err(|_| FixedViewCopyError::WorkOverflow)?,
    };
    Ok(OptimizationWorkUsage {
        rule_evaluations: functions,
        candidates: requirements,
        validation_steps: requirements,
        commits,
        iterations: 1,
    })
}
