use omega_optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationExecutionPhase,
    OptimizationSelections,
};
use omega_regalloc::PostAllocationSelectedTransformation;

use crate::function_relative_realization::{
    function_relative_statistics, seal_function_relative_manifest,
};
use crate::{
    FunctionRelativeOptimizationRealizationError, FunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationUnavailableData,
    OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError,
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt,
    TerminalWholeFunctionExitContractError,
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    ValidatedTerminalWholeFunctionExitContract, stage_terminal_whole_function_exit_contract,
    validate_optimized_active_resident_rematerialization_resolved_selected_form_layout,
    validate_terminal_whole_function_exit_contract,
};

/// Explicit-staging-only completion of the active-resident rematerialization
/// vertical at the function-relative, frameless whole-function-exit boundary.
/// It grants no frame, emission, section, object, image, installation, or
/// publication authority.
#[derive(Debug)]
pub struct StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization {
    source: StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
    exit_contract: ValidatedTerminalWholeFunctionExitContract,
    manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    custody:
        StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt,
}

impl StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization {
    pub const fn source(
        &self,
    ) -> &StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout {
        &self.source
    }

    pub const fn exit_contract(&self) -> &ValidatedTerminalWholeFunctionExitContract {
        &self.exit_contract
    }

    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }

    pub const fn custody(
        &self,
    ) -> &StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt
    {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt {
    source: StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt,
    exit_contract: crate::TerminalWholeFunctionExitContractIdentity,
    realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(
        &self,
    ) -> &StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt
    {
        &self.source
    }

    pub const fn exit_contract(&self) -> crate::TerminalWholeFunctionExitContractIdentity {
        self.exit_contract
    }

    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedActiveResidentRematerializationFunctionRelativeRealizationError {
    Source(OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError),
    ExitContract(TerminalWholeFunctionExitContractError),
    Manifest(FunctionRelativeOptimizationRealizationError),
    LaterPhaseSelected,
    RootMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display
    for OptimizedActiveResidentRematerializationFunctionRelativeRealizationError
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized active-resident rematerialization function-relative realization failed: {self:?}"
        )
    }
}

impl std::error::Error
    for OptimizedActiveResidentRematerializationFunctionRelativeRealizationError
{
}

pub fn stage_optimized_active_resident_rematerialization_function_relative_realization(
    source: StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
) -> Result<
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
> {
    let source_custody =
        validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(&source)
            .map_err(
                OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::Source,
            )?;
    let artifacts = artifacts(&source)?;
    let exit_contract = stage_terminal_whole_function_exit_contract(
        artifacts.selected,
        artifacts.machine,
        artifacts.physical,
        artifacts.encoding,
        artifacts.layout,
    )
    .map_err(
        OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::ExitContract,
    )?;
    let manifest = expected_manifest(&source, &exit_contract)?;
    let custody = custody_receipt(source_custody, &exit_contract, &manifest);
    let staged = StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization {
        source,
        exit_contract,
        manifest,
        custody,
    };
    validate_optimized_active_resident_rematerialization_function_relative_realization(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_active_resident_rematerialization_function_relative_realization(
    staged: &StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
) -> Result<
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt,
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
> {
    let source_custody =
        validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
            &staged.source,
        )
        .map_err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::Source,
        )?;
    let artifacts = artifacts(&staged.source)?;
    validate_terminal_whole_function_exit_contract(
        artifacts.selected,
        artifacts.machine,
        artifacts.physical,
        artifacts.encoding,
        artifacts.layout,
        &staged.exit_contract,
    )
    .map_err(
        OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::ExitContract,
    )?;
    let manifest = expected_manifest(&staged.source, &staged.exit_contract)?;
    if manifest.record() != staged.manifest.record() {
        return Err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::RootMismatch,
        );
    }
    let custody = custody_receipt(source_custody, &staged.exit_contract, &manifest);
    if custody != staged.custody {
        return Err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::ReceiptMismatch,
        );
    }
    Ok(custody)
}

struct SourceArtifacts<'source> {
    selected: &'source omega_regalloc::ValidatedTerminalPressureRematerialization,
    machine: &'source crate::StagedOptimizedPostAllocationMachinePlan,
    physical: &'source omega_register_model::ValidatedPhysicalRegisterModel,
    encoding: &'source crate::StagedOptimizedSelectedFormEncoding,
    layout: &'source crate::StagedOptimizedResolvedSelectedFormLayout,
}

fn artifacts(
    source: &StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
) -> Result<
    SourceArtifacts<'_>,
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
> {
    let rematerialization = source.pre_layout().source();
    let selected_stage = rematerialization
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selections = selected_stage.optimized_target().optimized().selections();
    if !selections
        .for_phase(OptimizationExecutionPhase::SelectedLowering)
        .is_empty()
        || !selections
            .for_phase(OptimizationExecutionPhase::AllocationRecovery)
            .is_empty()
        || !selections
            .for_phase(OptimizationExecutionPhase::PostAllocationMachine)
            .is_empty()
        || !selections
            .for_phase(OptimizationExecutionPhase::FunctionRelativeLayout)
            .is_empty()
    {
        return Err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::LaterPhaseSelected,
        );
    }
    Ok(SourceArtifacts {
        selected: rematerialization.rematerialization(),
        machine: source.pre_layout().machine(),
        physical: selected_stage.register_environment().physical(),
        encoding: source.pre_layout().encoding(),
        layout: source.layout(),
    })
}

fn expected_manifest(
    source: &StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
    exit_contract: &ValidatedTerminalWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
> {
    let artifacts = artifacts(source)?;
    let rematerialization = source.pre_layout().source();
    let selected_stage = rematerialization
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selections = selected_stage.optimized_target().optimized().selections();
    let empty = OptimizationSelections::default().identity();
    let source_custody = rematerialization.custody().source();
    let post = rematerialization.post_allocation_manifest().record();
    let rematerialization_identity = rematerialization.rematerialization().receipt().identity();
    let machine = artifacts.machine.machine().receipt();
    if post.pre_physical != source_custody.manifest()
        || post.selected_lowering_completion.is_some()
        || post.selected_transformations
            != [
                PostAllocationSelectedTransformation::PressureRematerialization(
                    rematerialization_identity,
                ),
            ]
        || post.selected
            != rematerialization
                .rematerialization()
                .receipt()
                .transformed_selected()
        || post.target != artifacts.layout.target()
        || machine.post_allocation_manifest() != post.identity
        || machine.selected() != post.selected
        || artifacts.encoding.selected() != post.selected
        || artifacts.encoding.machine() != machine.identity()
        || artifacts.layout.selected() != post.selected
        || artifacts.layout.machine() != machine.identity()
        || artifacts.layout.pre_layout() != artifacts.encoding.identity()
        || exit_contract.contract().selected != post.selected
        || exit_contract.contract().post_allocation_manifest != post.identity
        || exit_contract.contract().post_allocation_machine != machine.identity()
        || exit_contract.contract().pre_layout != artifacts.encoding.identity()
        || exit_contract.contract().resolved_layout != artifacts.layout.identity()
        || !matches!(
            exit_contract.contract().layout_custody,
            crate::TerminalWholeFunctionExitLayoutCustody::BaselineNearLayoutV1
        )
    {
        return Err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::RootMismatch,
        );
    }
    let unavailable = FunctionRelativeOptimizationUnavailableData::Unavailable;
    let record = FunctionRelativeOptimizationRealizationManifest {
        identity: FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
            b"pending",
        ),
        stage:
            FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        selections: selections.identity(),
        selected_lowering_selections: empty,
        selected_lowering_completion: None,
        post_allocation_machine_selections: empty,
        function_relative_layout_selections: empty,
        pre_physical_manifest: source_custody.manifest(),
        post_allocation_manifest: post.identity,
        selected: post.selected,
        pre_allocation_machine_effects: artifacts
            .machine
            .effects()
            .effects()
            .receipt()
            .identity(),
        post_allocation_machine: machine.identity(),
        baseline_pre_layout: artifacts.encoding.identity(),
        pre_layout: artifacts.encoding.identity(),
        baseline_resolved_layout: artifacts.layout.identity(),
        resolved_layout: artifacts.layout.identity(),
        x86_branch_relaxation: None,
        aarch64_cbnz_fusion: None,
        whole_function_exit_contract: exit_contract.identity(),
        target: artifacts.layout.target(),
        layout_policy: artifacts.layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        statistics: function_relative_statistics(artifacts.layout).map_err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::Manifest,
        )?,
        frame: unavailable,
        machine_emission: unavailable,
        section_placement: unavailable,
        symbols: unavailable,
        object_relocations: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    Ok(seal_function_relative_manifest(record))
}

fn custody_receipt(
    source: StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt,
    exit_contract: &ValidatedTerminalWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt {
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt {
        source,
        exit_contract: exit_contract.identity(),
        realization: manifest.record().identity,
    }
}

#[cfg(test)]
pub(crate) fn corrupt_active_resident_function_relative_source_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
) {
    crate::active_resident_resolved_selected_form_layout::corrupt_active_resident_resolved_layout_byte_for_test(
        &mut staged.source,
    );
}

#[cfg(test)]
pub(crate) fn corrupt_active_resident_function_relative_exit_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
) {
    staged.exit_contract.contract_mut().result_view =
        omega_register_model::RegisterViewId(u16::MAX);
}

#[cfg(test)]
pub(crate) fn corrupt_active_resident_function_relative_manifest_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
) {
    staged.manifest.record_mut().selected =
        omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity::from_bytes(
            [0x91; 32],
        );
}

#[cfg(test)]
pub(crate) fn corrupt_active_resident_function_relative_receipt_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
) {
    staged.custody.realization =
        FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes([0x92; 32]);
}

#[cfg(test)]
pub(crate) fn replace_active_resident_function_relative_exit_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
    foreign: &StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
) {
    staged.exit_contract = foreign.exit_contract.clone();
}
