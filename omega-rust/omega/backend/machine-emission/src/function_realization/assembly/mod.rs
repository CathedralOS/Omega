//! Optimizer module role: executable entrance.
use super::prelude::*;
use super::{error::*, model::*};
use selected_instructions_to_register_homes::AllocationOutput;

mod allocation;
mod custody;
mod fixed_frame;
mod manifests;
mod rel8;
mod statistics;
mod validation;

pub(super) use allocation::*;
pub(super) use custody::*;
pub(super) use fixed_frame::*;
pub(super) use manifests::{
    expected_allocated_post_allocation_machine_manifest, expected_direct_manifest,
    expected_manifest,
};
pub(super) use rel8::*;
pub(crate) use statistics::{function_relative_statistics, seal_function_relative_manifest};
pub(super) use validation::validate_realization_artifacts;

pub(super) fn build_realization(
    allocation: &AllocationOutput<'_>,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    (
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedResolvedSelectedFormLayout,
        Option<StagedOptimizedX86BranchRelaxation>,
        Option<super::UnitSavedReturnAddressFrame>,
        ValidatedWholeFunctionExitContract,
        ValidatedFunctionRelativeOptimizationRealizationManifest,
    ),
    FunctionRelativeOptimizationRealizationError,
> {
    let selected = allocation.selected();
    let physical = allocation.register_environment().physical();
    let selections = allocation.selections();
    let budget = allocation.budget_per_pass();
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, machine, physical)
            .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let baseline_layout =
        stage_optimized_resolved_selected_form_layout(selected, machine, physical, &encoding)
            .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let relaxation = stage_selected_relaxation(
        selected,
        machine,
        physical,
        &encoding,
        &baseline_layout,
        selections,
        budget,
    )?;
    let frame = super::unit::frame::stage_unit_frame(allocation, machine)?;
    let exit_contract = match frame.as_ref() {
        Some(frame) => stage_whole_function_exit_contract_with_frame(
            selected,
            machine,
            physical,
            &encoding,
            final_layout(&baseline_layout, relaxation.as_ref()),
            frame.layout(),
            frame.protocol(),
        )
        .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?,
        None => stage_exit_contract(
            selected,
            machine,
            physical,
            &encoding,
            &baseline_layout,
            relaxation.as_ref(),
        )?,
    };
    let manifest = expected_manifest(
        allocation,
        machine,
        &encoding,
        &baseline_layout,
        relaxation.as_ref(),
        frame.as_ref(),
        &exit_contract,
    )?;
    Ok((
        encoding,
        baseline_layout,
        relaxation,
        frame,
        exit_contract,
        manifest,
    ))
}
