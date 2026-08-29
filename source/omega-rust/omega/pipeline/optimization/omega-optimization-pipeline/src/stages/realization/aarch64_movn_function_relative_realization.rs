use omega_optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationExecutionPhase,
    OptimizationSelections,
};

use crate::stages::realization::function_relative_realization::expected_aarch64_movn_manifest;
use crate::{
    FunctionRelativeOptimizationRealizationError,
    OptimizedAarch64MovnResolvedSelectedFormLayoutError,
    StagedOptimizedAarch64MovnResolvedSelectedFormLayout,
    StagedOptimizedAarch64MovnResolvedSelectedFormLayoutCustodyReceipt,
    StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout,
    StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayoutCustodyReceipt,
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
    WholeFunctionExitContractError, WholeFunctionExitContractIdentity,
    stage_selected_lowering_whole_function_exit_contract_after_aarch64_movn_materialization,
    stage_whole_function_exit_contract_after_aarch64_movn_materialization,
    validate_optimized_aarch64_movn_resolved_selected_form_layout,
    validate_selected_lowering_aarch64_movn_resolved_selected_form_layout,
    validate_selected_lowering_whole_function_exit_contract_after_aarch64_movn_materialization,
    validate_whole_function_exit_contract_after_aarch64_movn_materialization,
};

/// Completed direct-homes MOVN realization at the function-relative exit
/// boundary. The resolved-layout carrier remains owned intact so downstream
/// replay cannot detach the optimized bytes from their baseline comparison.
#[derive(Debug)]
pub struct StagedOptimizedAarch64MovnFunctionRelativeRealization {
    source: StagedOptimizedAarch64MovnResolvedSelectedFormLayout,
    exit_contract: ValidatedWholeFunctionExitContract,
    manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    custody: StagedOptimizedAarch64MovnFunctionRelativeRealizationCustodyReceipt,
}

impl StagedOptimizedAarch64MovnFunctionRelativeRealization {
    pub const fn source(&self) -> &StagedOptimizedAarch64MovnResolvedSelectedFormLayout {
        &self.source
    }

    pub const fn homes(&self) -> &crate::StagedOptimizedRegisterHomes {
        self.source.homes()
    }

    pub const fn machine(&self) -> &crate::StagedOptimizedPostAllocationMachinePlan {
        self.source.machine()
    }

    pub const fn materialization(&self) -> &crate::StagedOptimizedAarch64MovnMaterialization {
        self.source.materialization()
    }

    pub const fn baseline_encoding(&self) -> &crate::StagedOptimizedSelectedFormEncoding {
        self.source.baseline_encoding()
    }

    pub const fn encoding(&self) -> &crate::StagedOptimizedSelectedFormEncoding {
        self.source.encoding()
    }

    pub const fn baseline_layout(&self) -> &crate::StagedOptimizedResolvedSelectedFormLayout {
        self.source.baseline_layout()
    }

    pub const fn layout(&self) -> &crate::StagedOptimizedResolvedSelectedFormLayout {
        self.source.layout()
    }

    pub const fn exit_contract(&self) -> &ValidatedWholeFunctionExitContract {
        &self.exit_contract
    }

    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }

    pub const fn custody(
        &self,
    ) -> &StagedOptimizedAarch64MovnFunctionRelativeRealizationCustodyReceipt {
        &self.custody
    }
}

/// Completed selected-lowering-plus-MOVN realization. The exact selected-
/// lowering completion remains nested in the source homes carrier.
#[derive(Debug)]
pub struct StagedSelectedLoweringAarch64MovnFunctionRelativeRealization {
    source: StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout,
    exit_contract: ValidatedWholeFunctionExitContract,
    manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    custody: StagedSelectedLoweringAarch64MovnFunctionRelativeRealizationCustodyReceipt,
}

impl StagedSelectedLoweringAarch64MovnFunctionRelativeRealization {
    pub const fn source(&self) -> &StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout {
        &self.source
    }

    pub const fn homes(&self) -> &crate::StagedOptimizedRegisterHomesAfterSelectedLowering {
        self.source.homes()
    }

    pub const fn machine(&self) -> &crate::StagedOptimizedPostAllocationMachinePlan {
        self.source.machine()
    }

    pub const fn materialization(&self) -> &crate::StagedOptimizedAarch64MovnMaterialization {
        self.source.materialization()
    }

    pub const fn baseline_encoding(&self) -> &crate::StagedOptimizedSelectedFormEncoding {
        self.source.baseline_encoding()
    }

    pub const fn encoding(&self) -> &crate::StagedOptimizedSelectedFormEncoding {
        self.source.encoding()
    }

    pub const fn baseline_layout(&self) -> &crate::StagedOptimizedResolvedSelectedFormLayout {
        self.source.baseline_layout()
    }

    pub const fn layout(&self) -> &crate::StagedOptimizedResolvedSelectedFormLayout {
        self.source.layout()
    }

    pub const fn exit_contract(&self) -> &ValidatedWholeFunctionExitContract {
        &self.exit_contract
    }

    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }

    pub const fn custody(
        &self,
    ) -> &StagedSelectedLoweringAarch64MovnFunctionRelativeRealizationCustodyReceipt {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedAarch64MovnFunctionRelativeRealizationCustodyReceipt {
    source: StagedOptimizedAarch64MovnResolvedSelectedFormLayoutCustodyReceipt,
    exit_contract: WholeFunctionExitContractIdentity,
    realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedOptimizedAarch64MovnFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(
        &self,
    ) -> &StagedOptimizedAarch64MovnResolvedSelectedFormLayoutCustodyReceipt {
        &self.source
    }

    pub const fn exit_contract(&self) -> WholeFunctionExitContractIdentity {
        self.exit_contract
    }

    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedSelectedLoweringAarch64MovnFunctionRelativeRealizationCustodyReceipt {
    source: StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayoutCustodyReceipt,
    exit_contract: WholeFunctionExitContractIdentity,
    realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedSelectedLoweringAarch64MovnFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(
        &self,
    ) -> &StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayoutCustodyReceipt {
        &self.source
    }

    pub const fn exit_contract(&self) -> WholeFunctionExitContractIdentity {
        self.exit_contract
    }

    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug)]
pub enum OptimizedAarch64MovnFunctionRelativeRealizationError {
    Source(OptimizedAarch64MovnResolvedSelectedFormLayoutError),
    ExitContract(WholeFunctionExitContractError),
    Manifest(FunctionRelativeOptimizationRealizationError),
    RootMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedAarch64MovnFunctionRelativeRealizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized AArch64 MOVN function-relative realization failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedAarch64MovnFunctionRelativeRealizationError {}

pub fn stage_optimized_aarch64_movn_function_relative_realization(
    source: StagedOptimizedAarch64MovnResolvedSelectedFormLayout,
) -> Result<
    StagedOptimizedAarch64MovnFunctionRelativeRealization,
    OptimizedAarch64MovnFunctionRelativeRealizationError,
> {
    let source_custody = validate_optimized_aarch64_movn_resolved_selected_form_layout(&source)
        .map_err(OptimizedAarch64MovnFunctionRelativeRealizationError::Source)?;
    let exit_contract =
        stage_whole_function_exit_contract_after_aarch64_movn_materialization(&source)
            .map_err(OptimizedAarch64MovnFunctionRelativeRealizationError::ExitContract)?;
    let manifest = direct_manifest(&source, &exit_contract)?;
    let custody = StagedOptimizedAarch64MovnFunctionRelativeRealizationCustodyReceipt {
        source: source_custody,
        exit_contract: exit_contract.identity(),
        realization: manifest.record().identity,
    };
    let staged = StagedOptimizedAarch64MovnFunctionRelativeRealization {
        source,
        exit_contract,
        manifest,
        custody,
    };
    validate_optimized_aarch64_movn_function_relative_realization(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_aarch64_movn_function_relative_realization(
    staged: &StagedOptimizedAarch64MovnFunctionRelativeRealization,
) -> Result<
    StagedOptimizedAarch64MovnFunctionRelativeRealizationCustodyReceipt,
    OptimizedAarch64MovnFunctionRelativeRealizationError,
> {
    let source_custody =
        validate_optimized_aarch64_movn_resolved_selected_form_layout(&staged.source)
            .map_err(OptimizedAarch64MovnFunctionRelativeRealizationError::Source)?;
    validate_whole_function_exit_contract_after_aarch64_movn_materialization(
        &staged.source,
        &staged.exit_contract,
    )
    .map_err(OptimizedAarch64MovnFunctionRelativeRealizationError::ExitContract)?;
    let manifest = direct_manifest(&staged.source, &staged.exit_contract)?;
    if manifest.record() != staged.manifest.record() {
        return Err(OptimizedAarch64MovnFunctionRelativeRealizationError::RootMismatch);
    }
    let custody = StagedOptimizedAarch64MovnFunctionRelativeRealizationCustodyReceipt {
        source: source_custody,
        exit_contract: staged.exit_contract.identity(),
        realization: manifest.record().identity,
    };
    if custody != staged.custody {
        return Err(OptimizedAarch64MovnFunctionRelativeRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

pub fn stage_selected_lowering_aarch64_movn_function_relative_realization(
    source: StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout,
) -> Result<
    StagedSelectedLoweringAarch64MovnFunctionRelativeRealization,
    OptimizedAarch64MovnFunctionRelativeRealizationError,
> {
    let source_custody =
        validate_selected_lowering_aarch64_movn_resolved_selected_form_layout(&source)
            .map_err(OptimizedAarch64MovnFunctionRelativeRealizationError::Source)?;
    let exit_contract = selected_lowering_exit_contract(&source)?;
    let manifest = selected_lowering_manifest(&source, &exit_contract)?;
    let custody = StagedSelectedLoweringAarch64MovnFunctionRelativeRealizationCustodyReceipt {
        source: source_custody,
        exit_contract: exit_contract.identity(),
        realization: manifest.record().identity,
    };
    let staged = StagedSelectedLoweringAarch64MovnFunctionRelativeRealization {
        source,
        exit_contract,
        manifest,
        custody,
    };
    validate_selected_lowering_aarch64_movn_function_relative_realization(&staged)?;
    Ok(staged)
}

pub fn validate_selected_lowering_aarch64_movn_function_relative_realization(
    staged: &StagedSelectedLoweringAarch64MovnFunctionRelativeRealization,
) -> Result<
    StagedSelectedLoweringAarch64MovnFunctionRelativeRealizationCustodyReceipt,
    OptimizedAarch64MovnFunctionRelativeRealizationError,
> {
    let source_custody =
        validate_selected_lowering_aarch64_movn_resolved_selected_form_layout(&staged.source)
            .map_err(OptimizedAarch64MovnFunctionRelativeRealizationError::Source)?;
    validate_selected_lowering_exit_contract(&staged.source, &staged.exit_contract)?;
    let manifest = selected_lowering_manifest(&staged.source, &staged.exit_contract)?;
    if manifest.record() != staged.manifest.record() {
        return Err(OptimizedAarch64MovnFunctionRelativeRealizationError::RootMismatch);
    }
    let custody = StagedSelectedLoweringAarch64MovnFunctionRelativeRealizationCustodyReceipt {
        source: source_custody,
        exit_contract: staged.exit_contract.identity(),
        realization: manifest.record().identity,
    };
    if custody != staged.custody {
        return Err(OptimizedAarch64MovnFunctionRelativeRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

fn selected_lowering_exit_contract(
    source: &StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout,
) -> Result<ValidatedWholeFunctionExitContract, OptimizedAarch64MovnFunctionRelativeRealizationError>
{
    stage_selected_lowering_whole_function_exit_contract_after_aarch64_movn_materialization(source)
        .map_err(OptimizedAarch64MovnFunctionRelativeRealizationError::ExitContract)
}

fn validate_selected_lowering_exit_contract(
    source: &StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), OptimizedAarch64MovnFunctionRelativeRealizationError> {
    validate_selected_lowering_whole_function_exit_contract_after_aarch64_movn_materialization(
        source,
        exit_contract,
    )
    .map_err(OptimizedAarch64MovnFunctionRelativeRealizationError::ExitContract)
}

fn direct_manifest(
    source: &StagedOptimizedAarch64MovnResolvedSelectedFormLayout,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    OptimizedAarch64MovnFunctionRelativeRealizationError,
> {
    let selected_stage = source
        .homes()
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selections = selected_stage.optimized_target().optimized().selections();
    let post = source.homes().post_allocation_manifest().record();
    expected_aarch64_movn_manifest(
        selections,
        OptimizationSelections::default().identity(),
        None,
        source.homes().custody().manifest(),
        post.identity,
        post.selected,
        post.target,
        source.machine(),
        source.materialization(),
        source.baseline_encoding(),
        source.encoding(),
        source.baseline_layout(),
        source.layout(),
        exit_contract,
    )
    .map_err(OptimizedAarch64MovnFunctionRelativeRealizationError::Manifest)
}

fn selected_lowering_manifest(
    source: &StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    OptimizedAarch64MovnFunctionRelativeRealizationError,
> {
    let run = source.homes().selected_lowering_run();
    let completion = run.custody();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selections = selected_stage.optimized_target().optimized().selections();
    let post = source.homes().post_allocation_manifest().record();
    expected_aarch64_movn_manifest(
        selections,
        selections
            .for_phase(OptimizationExecutionPhase::SelectedLowering)
            .identity(),
        Some(completion.identity()),
        completion.source().manifest(),
        post.identity,
        post.selected,
        post.target,
        source.machine(),
        source.materialization(),
        source.baseline_encoding(),
        source.encoding(),
        source.baseline_layout(),
        source.layout(),
        exit_contract,
    )
    .map_err(OptimizedAarch64MovnFunctionRelativeRealizationError::Manifest)
}

#[cfg(test)]
pub(crate) fn corrupt_aarch64_movn_function_relative_source_for_test(
    staged: &mut StagedOptimizedAarch64MovnFunctionRelativeRealization,
) {
    crate::stages::layout::aarch64_movn_resolved_selected_form_layout::corrupt_aarch64_movn_resolved_layout_byte_for_test(
        &mut staged.source,
    );
}

#[cfg(test)]
pub(crate) fn corrupt_aarch64_movn_function_relative_exit_for_test(
    staged: &mut StagedOptimizedAarch64MovnFunctionRelativeRealization,
) {
    staged.exit_contract.contract_mut().result_view =
        omega_register_model::RegisterViewId(u16::MAX);
}

#[cfg(test)]
pub(crate) fn corrupt_aarch64_movn_function_relative_manifest_for_test(
    staged: &mut StagedOptimizedAarch64MovnFunctionRelativeRealization,
) {
    staged.manifest.record_mut().aarch64_movn_materialization = None;
}

#[cfg(test)]
pub(crate) fn corrupt_aarch64_movn_function_relative_receipt_for_test(
    staged: &mut StagedOptimizedAarch64MovnFunctionRelativeRealization,
) {
    staged.custody.realization =
        FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes([0x4d; 32]);
}
