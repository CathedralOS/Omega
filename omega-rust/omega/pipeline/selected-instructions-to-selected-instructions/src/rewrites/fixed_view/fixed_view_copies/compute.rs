use crate::{FixedViewCopyPolicy, ValidatedFixedViewCopies, materialize_fixed_view_copies};
use optimization_core::OptimizationWorkBudget;

use crate::StagedOptimizedFixedPrecoloredSegmentHomes;

use super::model::OptimizedFixedViewCopyCustodyError;

pub(super) fn compute_fixed_view_copies(
    source: &StagedOptimizedFixedPrecoloredSegmentHomes,
    policy: FixedViewCopyPolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedFixedViewCopies, OptimizedFixedViewCopyCustodyError> {
    let legality = source.source_legality_stage();
    let selected_stage = legality
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let environment = selected_stage.register_environment();
    materialize_fixed_view_copies(
        selected_stage.selected(),
        legality.live_range_stage().ranges(),
        legality.legality(),
        source.fixed_intervals(),
        source.split_requirements(),
        source.segment_homes(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        policy,
        budget,
    )
    .map_err(OptimizedFixedViewCopyCustodyError::Materialization)
}
