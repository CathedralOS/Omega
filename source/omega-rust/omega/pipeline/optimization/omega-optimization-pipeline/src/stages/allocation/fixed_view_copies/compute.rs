use omega_optimization_core::OptimizationWorkBudget;
use omega_regalloc::{
    FixedViewCopyPolicy, ValidatedFixedViewCopies, materialize_fixed_view_copies,
};

use crate::StagedOptimizedAllocationLegality;

use super::model::OptimizedFixedViewCopyCustodyError;

pub(super) fn compute_fixed_view_copies(
    source: &StagedOptimizedAllocationLegality,
    policy: FixedViewCopyPolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedFixedViewCopies, OptimizedFixedViewCopyCustodyError> {
    let selected_stage = source.live_range_stage().liveness_stage().selected_stage();
    let environment = selected_stage.register_environment();
    materialize_fixed_view_copies(
        selected_stage.selected(),
        source.live_range_stage().ranges(),
        source.legality(),
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
