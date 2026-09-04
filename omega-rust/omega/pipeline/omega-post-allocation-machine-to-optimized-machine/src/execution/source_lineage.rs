use omega_optimization_core::OptimizationSelections;

use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};

pub(super) fn register_home_selections(
    source: &StagedOptimizedRegisterHomes,
) -> &OptimizationSelections {
    source
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections()
}

pub(super) fn selected_lowering_selections(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
) -> &OptimizationSelections {
    source
        .selected_lowering_run()
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections()
}

pub(super) fn active_resident_selections(
    source: &StagedOptimizedActiveResidentRematerialization,
) -> &OptimizationSelections {
    source
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections()
}
