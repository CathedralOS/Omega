//! One exit join over the layout phase's current data and explicit replay evidence.

use super::{
    ValidatedWholeFunctionExitContract, WholeFunctionExitContractError as Error,
    WholeFunctionExitLayoutCustody,
};
use super::{compute, validation, validation_rules};
use crate::ValidatedTargetFrameProtocolEncoding;
use crate::frame_layout::ValidatedTargetFrameLayout;
use post_allocation_machine_to_post_allocation_machine::StagedOptimizedPostAllocationMachineOptimization;
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use register_model::ValidatedPhysicalRegisterModel;
use resolved_layout_to_resolved_layout::{
    ResolvedLayoutOptimization, validate_resolved_layout_optimization,
};
use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

/// Validate the layout phase, then construct and independently check the exit
/// contract against its one current layout. Frames remain orthogonal evidence.
#[allow(clippy::too_many_arguments)]
pub fn stage_whole_function_exit_contract_for_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
    baseline: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &ResolvedLayoutOptimization,
    frame: Option<(
        &ValidatedTargetFrameLayout,
        &ValidatedTargetFrameProtocolEncoding,
    )>,
) -> Result<ValidatedWholeFunctionExitContract, Error> {
    let custody = validated_layout_custody(
        selected,
        machine,
        physical,
        encoding,
        optimization,
        baseline,
        layout,
    )?;
    let contract = compute::compute_inner(
        selected,
        machine,
        physical,
        encoding,
        layout.layout(),
        custody,
        frame,
    )?;
    validation::validate(
        selected,
        machine,
        physical,
        encoding,
        layout.layout(),
        custody,
        frame,
        &contract,
    )?;
    Ok(ValidatedWholeFunctionExitContract {
        contract: std::sync::Arc::new(contract),
    })
}

/// Replay source layout, optional transformation, and the claimed exit record.
/// Reading the current layout never selects a path from optimization history.
#[allow(clippy::too_many_arguments)]
pub fn validate_whole_function_exit_contract_for_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
    baseline: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &ResolvedLayoutOptimization,
    frame: Option<(
        &ValidatedTargetFrameLayout,
        &ValidatedTargetFrameProtocolEncoding,
    )>,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), Error> {
    let custody = validated_layout_custody(
        selected,
        machine,
        physical,
        encoding,
        optimization,
        baseline,
        layout,
    )?;
    validation::validate(
        selected,
        machine,
        physical,
        encoding,
        layout.layout(),
        custody,
        frame,
        contract.contract(),
    )
}

#[allow(clippy::too_many_arguments)]
fn validated_layout_custody<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
    baseline: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &ResolvedLayoutOptimization,
) -> Result<WholeFunctionExitLayoutCustody, Error> {
    validate_resolved_layout_optimization(
        selected,
        machine,
        physical,
        encoding,
        optimization,
        baseline,
        layout.selections(),
        layout,
    )
    .map_err(Error::LayoutOptimization)?;
    // Evidence selects the custody tag, never the current program accessor.
    if let Some(relaxation) = layout.relaxation() {
        Ok(
            WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                relaxation: relaxation.identity(),
            },
        )
    } else if let Some(optimization) = optimization {
        validation_rules::post_allocation_layout_custody(
            machine,
            encoding,
            layout.layout(),
            optimization,
        )
    } else {
        Ok(WholeFunctionExitLayoutCustody::BaselineNearLayoutV1)
    }
}
