use crate::{FixedViewCopyPolicy, ValidatedFixedViewCopies, materialize_fixed_view_copies};
use optimization_core::OptimizationWorkBudget;

use crate::StagedOptimizedFixedPrecoloredSegmentHomes;

use super::model::OptimizedFixedViewCopyCustodyError;

pub(super) fn compute_fixed_view_copies(
    source: &StagedOptimizedFixedPrecoloredSegmentHomes,
    policy: FixedViewCopyPolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedFixedViewCopies, OptimizedFixedViewCopyCustodyError> {
    let environment = source.register_environment();
    materialize_fixed_view_copies(
        source.selected(),
        source.ranges(),
        source.legality(),
        source.fixed_intervals(),
        source.split_requirements(),
        source.segment_homes(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        policy,
        budget,
    )
    .map_err(OptimizedFixedViewCopyCustodyError::Materialization)
}
