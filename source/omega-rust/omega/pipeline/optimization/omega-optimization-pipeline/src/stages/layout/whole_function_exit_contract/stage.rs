use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    validate_optimized_aarch64_movn_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    validate_optimized_x86_branch_relaxation,
    validate_selected_lowering_aarch64_movn_resolved_selected_form_layout,
    StagedOptimizedAarch64CbnzFusion, StagedOptimizedAarch64MovnResolvedSelectedFormLayout,
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedSelectedFormEncoding, StagedOptimizedX86BranchRelaxation,
    StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout,
};

use super::{
    compute::compute,
    error::WholeFunctionExitContractError,
    model::{ValidatedWholeFunctionExitContract, WholeFunctionExitLayoutCustody},
};

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
    let validated = ValidatedWholeFunctionExitContract { contract };
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
    let replayed = compute(
        selected,
        machine,
        physical,
        encoding,
        layout,
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
    )?;
    if replayed != contract.contract {
        return Err(WholeFunctionExitContractError::ArtifactMismatch);
    }
    Ok(())
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
    let validated = ValidatedWholeFunctionExitContract { contract };
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
    let replayed = compute(
        selected,
        machine,
        physical,
        encoding,
        relaxation.layout(),
        layout_custody,
    )?;
    if replayed != contract.contract {
        return Err(WholeFunctionExitContractError::ArtifactMismatch);
    }
    Ok(())
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
    let validated = ValidatedWholeFunctionExitContract { contract };
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
    let replayed = compute(
        selected,
        machine,
        physical,
        encoding,
        layout,
        layout_custody,
    )?;
    if replayed != contract.contract {
        return Err(WholeFunctionExitContractError::ArtifactMismatch);
    }
    Ok(())
}

/// Stage an exit contract over the direct-homes owning shortest-MOVN layout
/// carrier. The exact materialization identity remains explicit exit custody;
/// transformed bytes are never admitted through the baseline layout mode.
pub fn stage_whole_function_exit_contract_after_aarch64_movn_materialization(
    staged: &StagedOptimizedAarch64MovnResolvedSelectedFormLayout,
) -> Result<ValidatedWholeFunctionExitContract, WholeFunctionExitContractError> {
    validate_optimized_aarch64_movn_resolved_selected_form_layout(staged)
        .map_err(|_| WholeFunctionExitContractError::MovnLayout)?;
    let selected_stage = staged
        .homes()
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let layout_custody =
        WholeFunctionExitLayoutCustody::Aarch64SelectShortestMovnSeededI64MaterializationV1 {
            materialization: staged
                .materialization()
                .materialization()
                .receipt()
                .identity(),
        };
    let contract = compute(
        selected_stage.selected(),
        staged.machine(),
        selected_stage.register_environment().physical(),
        staged.encoding(),
        staged.layout(),
        layout_custody,
    )?;
    let validated = ValidatedWholeFunctionExitContract { contract };
    validate_whole_function_exit_contract_after_aarch64_movn_materialization(staged, &validated)?;
    Ok(validated)
}

/// Independently replay the owning direct-homes MOVN carrier before accepting
/// its whole-function exit contract.
pub fn validate_whole_function_exit_contract_after_aarch64_movn_materialization(
    staged: &StagedOptimizedAarch64MovnResolvedSelectedFormLayout,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), WholeFunctionExitContractError> {
    validate_optimized_aarch64_movn_resolved_selected_form_layout(staged)
        .map_err(|_| WholeFunctionExitContractError::MovnLayout)?;
    let selected_stage = staged
        .homes()
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let layout_custody =
        WholeFunctionExitLayoutCustody::Aarch64SelectShortestMovnSeededI64MaterializationV1 {
            materialization: staged
                .materialization()
                .materialization()
                .receipt()
                .identity(),
        };
    let replayed = compute(
        selected_stage.selected(),
        staged.machine(),
        selected_stage.register_environment().physical(),
        staged.encoding(),
        staged.layout(),
        layout_custody,
    )?;
    if replayed != contract.contract {
        return Err(WholeFunctionExitContractError::ArtifactMismatch);
    }
    Ok(())
}

/// Stage the same exact MOVN exit custody after a named selected-lowering run.
/// The transformed selected plan is derived only from the retained completion.
pub fn stage_selected_lowering_whole_function_exit_contract_after_aarch64_movn_materialization(
    staged: &StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout,
) -> Result<ValidatedWholeFunctionExitContract, WholeFunctionExitContractError> {
    validate_selected_lowering_aarch64_movn_resolved_selected_form_layout(staged)
        .map_err(|_| WholeFunctionExitContractError::MovnLayout)?;
    let run = staged.homes().selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let layout_custody =
        WholeFunctionExitLayoutCustody::Aarch64SelectShortestMovnSeededI64MaterializationV1 {
            materialization: staged
                .materialization()
                .materialization()
                .receipt()
                .identity(),
        };
    let contract = match run.steps().last() {
        Some(step) => compute(
            step.fold(),
            staged.machine(),
            selected_stage.register_environment().physical(),
            staged.encoding(),
            staged.layout(),
            layout_custody,
        ),
        None => compute(
            selected_stage.selected(),
            staged.machine(),
            selected_stage.register_environment().physical(),
            staged.encoding(),
            staged.layout(),
            layout_custody,
        ),
    }?;
    let validated = ValidatedWholeFunctionExitContract { contract };
    validate_selected_lowering_whole_function_exit_contract_after_aarch64_movn_materialization(
        staged, &validated,
    )?;
    Ok(validated)
}

/// Independently replay selected lowering, homes, MOVN materialization, and
/// resolved layout before accepting the selected-lowering exit contract.
pub fn validate_selected_lowering_whole_function_exit_contract_after_aarch64_movn_materialization(
    staged: &StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), WholeFunctionExitContractError> {
    validate_selected_lowering_aarch64_movn_resolved_selected_form_layout(staged)
        .map_err(|_| WholeFunctionExitContractError::MovnLayout)?;
    let run = staged.homes().selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let layout_custody =
        WholeFunctionExitLayoutCustody::Aarch64SelectShortestMovnSeededI64MaterializationV1 {
            materialization: staged
                .materialization()
                .materialization()
                .receipt()
                .identity(),
        };
    let replayed = match run.steps().last() {
        Some(step) => compute(
            step.fold(),
            staged.machine(),
            selected_stage.register_environment().physical(),
            staged.encoding(),
            staged.layout(),
            layout_custody,
        ),
        None => compute(
            selected_stage.selected(),
            staged.machine(),
            selected_stage.register_environment().physical(),
            staged.encoding(),
            staged.layout(),
            layout_custody,
        ),
    }?;
    if replayed != contract.contract {
        return Err(WholeFunctionExitContractError::ArtifactMismatch);
    }
    Ok(())
}
