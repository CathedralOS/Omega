//! Optimizer module role: executable entrance. Post-allocation machine-plan custody construction.
//!
//! Lineage leaves admit and project their exact source. Every route then
//! rejoins here for symbolic-machine analysis and custody sealing.

mod active_resident;
mod baseline;
mod fixed_view;
mod literal_fold;
mod selected_lowering;

pub use active_resident::stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization;
pub use baseline::stage_optimized_post_allocation_machine_plan;
pub use fixed_view::stage_optimized_post_allocation_machine_plan_after_fixed_view_copies;
pub use literal_fold::stage_optimized_post_allocation_machine_plan_after_literal_folds;
pub use selected_lowering::stage_optimized_post_allocation_machine_plan_after_selected_lowering;

use omega_machine_optimizer::analyze_post_allocation_machine_plan;
use omega_regalloc::{
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedPostAllocationOptimizationManifest,
    ValidatedRegisterHomes, ValidatedSelectedAnalysis,
};

use crate::{StagedOptimizedMachineEffects, ValidatedTargetRegisterEnvironment};

use super::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostAllocationMachineSourceCustodyReceipt, seal_staged_post_allocation_machine,
};

#[allow(clippy::too_many_arguments)]
fn analyze_and_seal<S: ValidatedSelectedAnalysis>(
    source: StagedOptimizedPostAllocationMachineSourceCustodyReceipt,
    selected: &S,
    effects: StagedOptimizedMachineEffects,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<StagedOptimizedPostAllocationMachinePlan, OptimizedPostAllocationMachinePipelineError> {
    let machine = analyze_post_allocation_machine_plan(
        selected,
        effects.effects(),
        ranges,
        legality,
        homes,
        manifest,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    Ok(seal_staged_post_allocation_machine(
        source, effects, machine,
    ))
}
