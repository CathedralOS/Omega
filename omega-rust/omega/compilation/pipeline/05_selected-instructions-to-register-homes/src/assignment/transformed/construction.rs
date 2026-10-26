use crate::assignment::post_allocation_manifest::project_post_allocation_optimization_manifest;
use crate::{RegisterHomeError, ValidatedRegisterHomes, assign_register_homes};

use selected_instructions_to_selected_instructions::{
    StagedPreAllocationOptimizationCustodyReceipt, StagedPreAllocationOptimizationRun,
    StagedSelectedLoweringOptimizationCustodyReceipt, StagedSelectedLoweringOptimizationRun,
};

use super::custody::{pre_allocation_home_custody_receipt, selected_lowering_home_custody_receipt};
use super::projection::{
    pre_allocation_final_analysis, pre_allocation_transformations,
    selected_lowering_final_analysis, selected_lowering_transformations,
};
use super::{
    OptimizedPostPreAllocationHomeCustodyError, OptimizedPostSelectedLoweringHomeCustodyError,
    StagedOptimizedRegisterHomesAfterPreAllocation,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};

/// Assignment probe over a completed pre-allocation run's final facts.
/// Computing the assignment without consuming the run lets the route answer
/// residual `NoCompatibleHome` pressure with runtime spill instead of
/// post-run homes.
pub(super) fn assign_register_homes_after_pre_allocation(
    run: &StagedPreAllocationOptimizationRun,
    reuse: Option<&mut crate::assignment::home_assignment::compute::FunctionHomeReuse>,
) -> Result<ValidatedRegisterHomes, RegisterHomeError> {
    let (ranges, legality) = pre_allocation_final_analysis(run);
    let environment = run.source_legality_stage().register_environment();
    crate::assignment::home_assignment::assign_register_homes_reusing(
        legality,
        ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        reuse,
    )
}

/// The same construction over an already-computed assignment, for the
/// composing route that probed assignment while it still owned the run.
pub(super) fn construct_register_homes_after_pre_allocation_with_assignment(
    run: StagedPreAllocationOptimizationRun,
    homes: ValidatedRegisterHomes,
) -> Result<
    StagedOptimizedRegisterHomesAfterPreAllocation,
    OptimizedPostPreAllocationHomeCustodyError,
> {
    let source = run.custody().clone();
    finish_register_homes_after_pre_allocation(run, source, homes)
}

fn finish_register_homes_after_pre_allocation(
    run: StagedPreAllocationOptimizationRun,
    source: StagedPreAllocationOptimizationCustodyReceipt,
    homes: ValidatedRegisterHomes,
) -> Result<
    StagedOptimizedRegisterHomesAfterPreAllocation,
    OptimizedPostPreAllocationHomeCustodyError,
> {
    let (ranges, legality) = pre_allocation_final_analysis(&run);
    let transformations = pre_allocation_transformations(&source);
    let manifest = project_post_allocation_optimization_manifest(
        source.source().manifest(),
        None,
        Some(source.identity()),
        &transformations,
        ranges,
        legality,
        &homes,
    )
    .map_err(OptimizedPostPreAllocationHomeCustodyError::Manifest)?;
    let custody = pre_allocation_home_custody_receipt(source, &homes, &manifest);
    Ok(StagedOptimizedRegisterHomesAfterPreAllocation {
        run,
        homes,
        manifest,
        custody,
    })
}

/// Assignment probe over a completed selected-lowering run's final facts.
/// Computing the assignment without consuming the run lets the route answer
/// residual `NoCompatibleHome` pressure with runtime spill instead of
/// post-run homes.
pub(super) fn assign_register_homes_after_selected_lowering(
    run: &StagedSelectedLoweringOptimizationRun,
    reuse: Option<&mut crate::assignment::home_assignment::compute::FunctionHomeReuse>,
) -> Result<ValidatedRegisterHomes, RegisterHomeError> {
    let (ranges, legality) = selected_lowering_final_analysis(run);
    let environment = run.source_legality_stage().register_environment();
    crate::assignment::home_assignment::assign_register_homes_reusing(
        legality,
        ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        reuse,
    )
}

/// The same construction over an already-computed assignment, for the
/// composing route that probed assignment while it still owned the run.
pub(super) fn construct_register_homes_after_selected_lowering_with_assignment(
    run: StagedSelectedLoweringOptimizationRun,
    homes: ValidatedRegisterHomes,
) -> Result<
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    OptimizedPostSelectedLoweringHomeCustodyError,
> {
    let source = run.custody().clone();
    finish_register_homes_after_selected_lowering(run, source, homes)
}

fn finish_register_homes_after_selected_lowering(
    run: StagedSelectedLoweringOptimizationRun,
    source: StagedSelectedLoweringOptimizationCustodyReceipt,
    homes: ValidatedRegisterHomes,
) -> Result<
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    OptimizedPostSelectedLoweringHomeCustodyError,
> {
    let (ranges, legality) = selected_lowering_final_analysis(&run);
    let transformations = selected_lowering_transformations(&source);
    let manifest = project_post_allocation_optimization_manifest(
        source.source().manifest(),
        Some(source.identity()),
        None,
        &transformations,
        ranges,
        legality,
        &homes,
    )
    .map_err(OptimizedPostSelectedLoweringHomeCustodyError::Manifest)?;
    let custody = selected_lowering_home_custody_receipt(source, &homes, &manifest);
    Ok(StagedOptimizedRegisterHomesAfterSelectedLowering {
        run,
        homes,
        manifest,
        custody,
    })
}
