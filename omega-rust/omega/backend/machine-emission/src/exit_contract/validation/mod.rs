//! Optimizer module role: executable entrance. Direct exit-record admission.
//!
//! Replay consumes the claimed rows instead of constructing a second contract.
//! Target catalogs and effect predicates are shared semantics; record production
//! and the roster/field checks below remain separate algorithms.

mod context;
mod ordinary;
mod returned;
mod structural;

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use super::{
    WholeFunctionExitContract, WholeFunctionExitContractError, WholeFunctionExitLayoutCustody,
};
use crate::ValidatedTargetFrameProtocolEncoding;
use post_allocation_machine_to_frame_layout::ValidatedTargetFrameLayout;
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;

struct Inputs<'a> {
    selected: &'a selected_instructions::SelectedInstructionPlan,
    machine: &'a physical_instructions::PostAllocationMachinePlan,
    physical: &'a ValidatedPhysicalRegisterModel,
    encoding: &'a StagedOptimizedSelectedFormEncoding,
    layout: &'a StagedOptimizedResolvedSelectedFormLayout,
    frame: Option<(
        &'a ValidatedTargetFrameLayout,
        &'a ValidatedTargetFrameProtocolEncoding,
    )>,
    contract: &'a WholeFunctionExitContract,
}

fn require(condition: bool) -> Result<(), WholeFunctionExitContractError> {
    condition
        .then_some(())
        .ok_or(WholeFunctionExitContractError::ArtifactMismatch)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    custody: WholeFunctionExitLayoutCustody,
    frame: Option<(
        &ValidatedTargetFrameLayout,
        &ValidatedTargetFrameProtocolEncoding,
    )>,
    contract: &WholeFunctionExitContract,
) -> Result<(), WholeFunctionExitContractError> {
    super::validation_rules::validate_layout_custody(machine, encoding, layout, custody)?;
    if selected.selected_identity() != machine.machine().plan().selected {
        return Err(WholeFunctionExitContractError::RootMismatch);
    }
    let inputs = Inputs {
        selected: selected.selected_plan(),
        machine: machine.machine().plan(),
        physical,
        encoding,
        layout,
        frame,
        contract,
    };
    let context = context::check(&inputs, custody)?;
    if inputs.selected.structural_unit_functions.is_empty() {
        ordinary::check(&inputs, &context)?;
    } else {
        structural::check(&inputs, &context)?;
    }
    require(contract.identity == contract.recomputed_identity())
}
