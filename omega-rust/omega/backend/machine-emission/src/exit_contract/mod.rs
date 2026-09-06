//! Optimizer module role: executable entrance. Whole-function exit-contract staging, replay validation, and canonical identity.

mod compute;
mod error;
mod identity;
mod layout_optimization;
mod model;
mod stage;
mod validation;
mod validation_rules;

pub use error::*;
pub use layout_optimization::*;
pub use model::*;
pub use stage::*;

mod post_allocation;
pub use post_allocation::*;

use post_allocation_machine_to_post_allocation_machine::StagedOptimizedPostAllocationMachineOptimization;
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use register_model::ValidatedPhysicalRegisterModel;
use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

/// Establish the canonical whole-function exit contract for one owning typed
/// post-allocation result. The join independently replays resolved layout and
/// binds the exact typed leaf to normalized encoding and layout custody.
pub fn stage_whole_function_exit_contract_with_post_allocation_machine_optimization_and_frame<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    frame: Option<(
        &crate::frame_layout::ValidatedTargetFrameLayout,
        &crate::ValidatedTargetFrameProtocolEncoding,
    )>,
) -> Result<ValidatedWholeFunctionExitContract, WholeFunctionExitContractError> {
    let layout_custody = validation_rules::post_allocation_layout_custody(
        machine,
        encoding,
        layout.program(),
        optimization,
    )?;
    let contract = match frame {
        Some((frame, protocol)) => compute::compute_with_frame(
            selected,
            machine,
            physical,
            encoding,
            layout.program(),
            layout_custody,
            frame,
            protocol,
        )?,
        None => compute::compute(
            selected,
            machine,
            physical,
            encoding,
            layout.program(),
            layout_custody,
        )?,
    };
    let validated = ValidatedWholeFunctionExitContract {
        contract: std::sync::Arc::new(contract),
    };
    validate_whole_function_exit_contract_with_post_allocation_machine_optimization_and_frame(
        selected,
        machine,
        physical,
        encoding,
        optimization,
        layout,
        frame,
        &validated,
    )?;
    Ok(validated)
}
