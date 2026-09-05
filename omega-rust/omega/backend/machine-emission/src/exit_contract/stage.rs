use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::ValidatedTargetFrameProtocolEncoding;
use crate::frame_layout::ValidatedTargetFrameLayout;
use post_allocation_machine_to_post_allocation_machine::StagedOptimizedAarch64CbnzFusion;
use post_allocation_machine_to_resolved_layout::selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use post_allocation_machine_to_resolved_layout::{
    StagedOptimizedResolvedSelectedFormLayout, StagedOptimizedX86BranchRelaxation,
    validate_optimized_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    validate_optimized_x86_branch_relaxation,
};
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

use super::{
    compute::{compute, compute_with_frame},
    error::WholeFunctionExitContractError,
    model::{ValidatedWholeFunctionExitContract, WholeFunctionExitLayoutCustody},
};

/// Establish a baseline-layout exit contract whose otherwise-forbidden call,
/// preservation, and link-register effects are discharged by one exact
/// validated target frame and its canonical byte protocol.
#[allow(clippy::too_many_arguments)]
pub fn stage_whole_function_exit_contract_with_frame<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    frame: &ValidatedTargetFrameLayout,
    protocol: &ValidatedTargetFrameProtocolEncoding,
) -> Result<ValidatedWholeFunctionExitContract, WholeFunctionExitContractError> {
    let contract = compute_with_frame(
        selected,
        machine,
        physical,
        encoding,
        layout,
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
        frame,
        protocol,
    )?;
    let validated = ValidatedWholeFunctionExitContract {
        contract: std::sync::Arc::new(contract),
    };
    validate_whole_function_exit_contract_with_frame(
        selected, machine, physical, encoding, layout, frame, protocol, &validated,
    )?;
    Ok(validated)
}

#[allow(clippy::too_many_arguments)]
pub fn validate_whole_function_exit_contract_with_frame<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    frame: &ValidatedTargetFrameLayout,
    protocol: &ValidatedTargetFrameProtocolEncoding,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), WholeFunctionExitContractError> {
    validate_optimized_resolved_selected_form_layout(selected, machine, physical, encoding, layout)
        .map_err(WholeFunctionExitContractError::Layout)?;
    super::validation::validate(
        selected,
        machine,
        physical,
        encoding,
        layout,
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
        Some((frame, protocol)),
        contract.contract(),
    )
}

/// Establish the baseline whole-function exit contract over an independently
/// validated resolved layout. This compatibility wrapper remains distinct
/// from the canonical typed post-allocation join in the module entrance.
pub fn stage_whole_function_exit_contract<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<ValidatedWholeFunctionExitContract, WholeFunctionExitContractError> {
    let contract = compute(
        selected,
        machine,
        physical,
        encoding,
        layout,
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
    )?;
    let validated = ValidatedWholeFunctionExitContract {
        contract: std::sync::Arc::new(contract),
    };
    validate_whole_function_exit_contract(
        selected, machine, physical, encoding, layout, &validated,
    )?;
    Ok(validated)
}

/// Replay the baseline compatibility join and reject any detached contract.
pub fn validate_whole_function_exit_contract<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), WholeFunctionExitContractError> {
    validate_optimized_resolved_selected_form_layout(selected, machine, physical, encoding, layout)
        .map_err(WholeFunctionExitContractError::Layout)?;
    super::validation::validate(
        selected,
        machine,
        physical,
        encoding,
        layout,
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
        None,
        contract.contract(),
    )
}

/// Stage an exit contract over an independently validated x86 branch-relaxed
/// layout. This path retains the relaxation receipt in the contract rather
/// than treating the transformed layout as baseline layout authority.
pub fn stage_whole_function_exit_contract_after_x86_branch_relaxation<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    source_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: &StagedOptimizedX86BranchRelaxation,
) -> Result<ValidatedWholeFunctionExitContract, WholeFunctionExitContractError> {
    let layout_custody = WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
        relaxation: relaxation.identity(),
    };
    let contract = compute(
        selected,
        machine,
        physical,
        encoding,
        relaxation.layout(),
        layout_custody,
    )?;
    let validated = ValidatedWholeFunctionExitContract {
        contract: std::sync::Arc::new(contract),
    };
    validate_whole_function_exit_contract_after_x86_branch_relaxation(
        selected,
        machine,
        physical,
        encoding,
        source_layout,
        relaxation,
        &validated,
    )?;
    Ok(validated)
}

/// Independently validate the source near layout and replay the x86 branch
/// relaxation before admitting its transformed layout to exit validation.
pub fn validate_whole_function_exit_contract_after_x86_branch_relaxation<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    source_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: &StagedOptimizedX86BranchRelaxation,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), WholeFunctionExitContractError> {
    validate_optimized_x86_branch_relaxation(
        selected,
        machine,
        physical,
        encoding,
        source_layout,
        relaxation,
    )
    .map_err(WholeFunctionExitContractError::Relaxation)?;
    let layout_custody = WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
        relaxation: relaxation.identity(),
    };
    super::validation::validate(
        selected,
        machine,
        physical,
        encoding,
        relaxation.layout(),
        layout_custody,
        None,
        contract.contract(),
    )
}

/// Stage an exit contract over the independently replayed final CBNZ layout.
/// The symbolic fusion receipt remains explicit authority for the zero-byte
/// compare and fused branch; neither is admitted as an ordinary baseline row.
pub fn stage_whole_function_exit_contract_after_aarch64_cbnz_fusion<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<ValidatedWholeFunctionExitContract, WholeFunctionExitContractError> {
    let layout_custody =
        WholeFunctionExitLayoutCustody::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
            fusion: fusion.fusion().receipt().identity(),
        };
    let contract = compute(
        selected,
        machine,
        physical,
        encoding,
        layout,
        layout_custody,
    )?;
    let validated = ValidatedWholeFunctionExitContract {
        contract: std::sync::Arc::new(contract),
    };
    validate_whole_function_exit_contract_after_aarch64_cbnz_fusion(
        selected, machine, physical, encoding, fusion, layout, &validated,
    )?;
    Ok(validated)
}

/// Independently reconstruct the CBNZ encoding and final layout before
/// accepting its whole-function exit contract.
pub fn validate_whole_function_exit_contract_after_aarch64_cbnz_fusion<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), WholeFunctionExitContractError> {
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
        selected, machine, physical, encoding, fusion, layout,
    )
    .map_err(WholeFunctionExitContractError::Layout)?;
    let layout_custody =
        WholeFunctionExitLayoutCustody::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
            fusion: fusion.fusion().receipt().identity(),
        };
    super::validation::validate(
        selected,
        machine,
        physical,
        encoding,
        layout,
        layout_custody,
        None,
        contract.contract(),
    )
}
