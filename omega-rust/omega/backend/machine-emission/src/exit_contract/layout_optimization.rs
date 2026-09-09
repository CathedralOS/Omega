//! Optimizer module role: executable entrance. One exit join over the layout phase's current data and explicit replay evidence.

use super::{
    ValidatedWholeFunctionExitContract, WholeFunctionExitContractError as Error,
    WholeFunctionExitLayoutCustody,
};
use super::{compute, validation};
use crate::ValidatedTargetFrameProtocolEncoding;
use crate::frame_layout::ValidatedTargetFrameLayout;
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
    baseline: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &ResolvedLayoutOptimization,
    frame: Option<(
        &ValidatedTargetFrameLayout,
        &ValidatedTargetFrameProtocolEncoding,
    )>,
) -> Result<ValidatedWholeFunctionExitContract, Error> {
    validate_layout(selected, machine, physical, encoding, baseline, layout)?;
    let custody = layout_custody(layout);
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
    baseline: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &ResolvedLayoutOptimization,
    frame: Option<(
        &ValidatedTargetFrameLayout,
        &ValidatedTargetFrameProtocolEncoding,
    )>,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), Error> {
    validate_layout(selected, machine, physical, encoding, baseline, layout)?;
    validate_exit_record_for_replayed_layout(
        selected, machine, physical, encoding, layout, frame, contract,
    )
}

/// Check the exit record after this invocation has replayed the exact borrowed
/// selected/machine/encoding/layout tuple. Fixed-frame admission additionally
/// binds encoding to its actual frame and layout to its source phase selections.
/// This is crate-private reuse of that replay, not a cached admission result.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_exit_record_for_replayed_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &ResolvedLayoutOptimization,
    frame: Option<(
        &ValidatedTargetFrameLayout,
        &ValidatedTargetFrameProtocolEncoding,
    )>,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), Error> {
    validation::validate(
        selected,
        machine,
        physical,
        encoding,
        layout.layout(),
        layout_custody(layout),
        frame,
        contract.contract(),
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &ResolvedLayoutOptimization,
) -> Result<(), Error> {
    validate_resolved_layout_optimization(
        selected,
        machine,
        physical,
        encoding,
        baseline,
        layout.selections(),
        layout,
    )
    .map_err(Error::LayoutOptimization)
}

fn layout_custody(layout: &ResolvedLayoutOptimization) -> WholeFunctionExitLayoutCustody {
    // Evidence selects the custody tag, never the current program accessor.
    if let Some(relaxation) = layout.relaxation() {
        WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
            relaxation: relaxation.identity(),
        }
    } else {
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1
    }
}
