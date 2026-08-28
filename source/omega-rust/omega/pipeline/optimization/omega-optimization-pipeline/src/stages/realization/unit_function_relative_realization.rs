use omega_optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationExecutionPhase,
    OptimizationSelectionIdentity,
};
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_selected_instructions::{SelectedInstructionKind, SelectedTerminator};

use crate::function_relative_realization::{
    function_relative_statistics, seal_function_relative_manifest,
};
use crate::{
    FunctionRelativeOptimizationRealizationError, FunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationUnavailableData, OptimizedPostAllocationMachinePipelineError,
    OptimizedRegisterHomeCustodyError, StagedOptimizedPostAllocationMachineCustodyReceipt,
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedRegisterHomeCustodyReceipt,
    StagedOptimizedRegisterHomes, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedSelectedFormEncoding, ValidatedFunctionRelativeOptimizationRealizationManifest,
    ValidatedWholeFunctionExitContract, WholeFunctionExitContractIdentity,
    stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_post_allocation_machine_plan, stage_optimized_resolved_selected_form_layout,
    stage_whole_function_exit_contract,
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_post_allocation_machine_plan_custody,
    validate_optimized_register_home_custody, validate_optimized_resolved_selected_form_layout,
    validate_whole_function_exit_contract,
};

/// Exact baseline realization for the currently admitted receiver-free Unit
/// semantic entry. This carrier proves function-relative bytes and exit
/// behavior only; it owns no source ProgramEntry signature, wrapper, process
/// entry, image, installation, or publication authority.
#[derive(Debug)]
pub struct StagedOptimizedUnitFunctionRelativeRealization {
    homes: StagedOptimizedRegisterHomes,
    machine: StagedOptimizedPostAllocationMachinePlan,
    encoding: StagedOptimizedSelectedFormEncoding,
    layout: StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: ValidatedWholeFunctionExitContract,
    manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    custody: StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt,
}

impl StagedOptimizedUnitFunctionRelativeRealization {
    pub const fn homes(&self) -> &StagedOptimizedRegisterHomes {
        &self.homes
    }

    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }

    pub const fn encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.encoding
    }

    pub const fn layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.layout
    }

    pub const fn exit_contract(&self) -> &ValidatedWholeFunctionExitContract {
        &self.exit_contract
    }

    pub const fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> &StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt {
        &self.custody
    }

    #[cfg(test)]
    pub(crate) fn exit_contract_mut(&mut self) -> &mut ValidatedWholeFunctionExitContract {
        &mut self.exit_contract
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt {
    source: StagedOptimizedRegisterHomeCustodyReceipt,
    machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    exit_contract: WholeFunctionExitContractIdentity,
    realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt {
    pub const fn source(&self) -> StagedOptimizedRegisterHomeCustodyReceipt {
        self.source
    }

    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }

    pub const fn exit_contract(&self) -> WholeFunctionExitContractIdentity {
        self.exit_contract
    }

    pub const fn realization(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        self.realization
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedUnitFunctionRelativeRealizationError {
    Homes(OptimizedRegisterHomeCustodyError),
    Machine(OptimizedPostAllocationMachinePipelineError),
    Encoding(crate::OptimizedSelectedFormEncodingError),
    Layout(crate::OptimizedResolvedSelectedFormLayoutError),
    Exit(crate::WholeFunctionExitContractError),
    UnsupportedSelectionPhase,
    UnsupportedUnitShape,
    RootMismatch,
    ReceiptMismatch,
    Manifest(FunctionRelativeOptimizationRealizationError),
}

impl std::fmt::Display for OptimizedUnitFunctionRelativeRealizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized Unit function-relative realization failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedUnitFunctionRelativeRealizationError {}

pub fn stage_optimized_unit_function_relative_realization(
    homes: StagedOptimizedRegisterHomes,
) -> Result<
    StagedOptimizedUnitFunctionRelativeRealization,
    OptimizedUnitFunctionRelativeRealizationError,
> {
    let source = validate_source(&homes)?;
    let selected_stage = selected_stage(&homes);
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    let machine = stage_optimized_post_allocation_machine_plan(&homes)
        .map_err(OptimizedUnitFunctionRelativeRealizationError::Machine)?;
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, &machine, physical)
            .map_err(OptimizedUnitFunctionRelativeRealizationError::Encoding)?;
    let layout =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .map_err(OptimizedUnitFunctionRelativeRealizationError::Layout)?;
    let exit_contract =
        stage_whole_function_exit_contract(selected, &machine, physical, &encoding, &layout)
            .map_err(OptimizedUnitFunctionRelativeRealizationError::Exit)?;
    let manifest = expected_manifest(&homes, &machine, &encoding, &layout, &exit_contract)?;
    let custody = receipt(source, &machine, &exit_contract, &manifest);
    Ok(StagedOptimizedUnitFunctionRelativeRealization {
        homes,
        machine,
        encoding,
        layout,
        exit_contract,
        manifest,
        custody,
    })
}

pub fn validate_optimized_unit_function_relative_realization(
    staged: &StagedOptimizedUnitFunctionRelativeRealization,
) -> Result<
    StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt,
    OptimizedUnitFunctionRelativeRealizationError,
> {
    let source = validate_source(&staged.homes)?;
    let machine =
        validate_optimized_post_allocation_machine_plan_custody(&staged.homes, &staged.machine)
            .map_err(OptimizedUnitFunctionRelativeRealizationError::Machine)?;
    if &machine != staged.machine.custody() {
        return Err(OptimizedUnitFunctionRelativeRealizationError::ReceiptMismatch);
    }
    let selected_stage = selected_stage(&staged.homes);
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    validate_optimized_layout_independent_selected_form_encoding(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
    )
    .map_err(OptimizedUnitFunctionRelativeRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.layout,
    )
    .map_err(OptimizedUnitFunctionRelativeRealizationError::Layout)?;
    validate_whole_function_exit_contract(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.layout,
        &staged.exit_contract,
    )
    .map_err(OptimizedUnitFunctionRelativeRealizationError::Exit)?;
    let manifest = expected_manifest(
        &staged.homes,
        &staged.machine,
        &staged.encoding,
        &staged.layout,
        &staged.exit_contract,
    )?;
    if manifest.record() != staged.manifest.record() {
        return Err(OptimizedUnitFunctionRelativeRealizationError::RootMismatch);
    }
    let custody = receipt(source, &staged.machine, &staged.exit_contract, &manifest);
    if custody != staged.custody {
        return Err(OptimizedUnitFunctionRelativeRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

fn selected_stage(
    homes: &StagedOptimizedRegisterHomes,
) -> &crate::StagedOptimizedSelectedInstructions {
    homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
}

fn validate_source(
    homes: &StagedOptimizedRegisterHomes,
) -> Result<StagedOptimizedRegisterHomeCustodyReceipt, OptimizedUnitFunctionRelativeRealizationError>
{
    let source = validate_optimized_register_home_custody(
        homes.legality_stage(),
        homes.homes(),
        homes.post_allocation_manifest(),
    )
    .map_err(OptimizedUnitFunctionRelativeRealizationError::Homes)?;
    if source != homes.custody() {
        return Err(OptimizedUnitFunctionRelativeRealizationError::ReceiptMismatch);
    }
    let selected_stage = selected_stage(homes);
    let selections = selected_stage.optimized_target().optimized().selections();
    if [
        OptimizationExecutionPhase::SelectedLowering,
        OptimizationExecutionPhase::AllocationRecovery,
        OptimizationExecutionPhase::PostAllocationMachine,
        OptimizationExecutionPhase::FunctionRelativeLayout,
    ]
    .into_iter()
    .any(|phase| !selections.for_phase(phase).is_empty())
    {
        return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedSelectionPhase);
    }
    validate_unit_shape(selected_stage.selected().selected_plan())?;
    Ok(source)
}

fn validate_unit_shape(
    selected: &omega_selected_instructions::SelectedInstructionPlan,
) -> Result<(), OptimizedUnitFunctionRelativeRealizationError> {
    let [function] = selected.functions.as_slice() else {
        return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedUnitShape);
    };
    let [block] = function.blocks.as_slice() else {
        return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedUnitShape);
    };
    let SelectedTerminator::Return { instruction, .. } = &block.terminator else {
        return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedUnitShape);
    };
    if selected.entry != function.machine
        || function.attachment.is_some()
        || function.entry_block != block.id
        || !function.virtual_registers.is_empty()
        || !block.instructions.is_empty()
        || instruction.kind != SelectedInstructionKind::ReturnUnit
        || !instruction.operands.is_empty()
    {
        return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedUnitShape);
    }
    Ok(())
}

fn expected_manifest(
    homes: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    OptimizedUnitFunctionRelativeRealizationError,
> {
    let selected_stage = selected_stage(homes);
    let optimized = selected_stage.optimized_target().optimized();
    let selections = optimized.selections();
    let source = homes.custody();
    let post = homes.post_allocation_manifest().record();
    if post.selected_lowering_completion.is_some()
        || post.selected != source.selected()
        || post.target != layout.target()
        || machine.machine().receipt().post_allocation_manifest() != post.identity
        || machine.machine().receipt().selected() != source.selected()
        || encoding.selected() != source.selected()
        || encoding.machine() != machine.machine().receipt().identity()
        || layout.selected() != source.selected()
        || layout.machine() != machine.machine().receipt().identity()
        || layout.pre_layout() != encoding.identity()
        || exit_contract.contract().selected != source.selected()
        || exit_contract.contract().post_allocation_manifest != post.identity
        || exit_contract.contract().post_allocation_machine
            != machine.machine().receipt().identity()
        || exit_contract.contract().pre_layout != encoding.identity()
        || exit_contract.contract().resolved_layout != layout.identity()
    {
        return Err(OptimizedUnitFunctionRelativeRealizationError::RootMismatch);
    }
    let unavailable = FunctionRelativeOptimizationUnavailableData::Unavailable;
    let record = FunctionRelativeOptimizationRealizationManifest {
        identity: FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
            b"pending",
        ),
        stage: FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        selections: selections.identity(),
        selected_lowering_selections: empty_phase_identity(selections, OptimizationExecutionPhase::SelectedLowering),
        selected_lowering_completion: None,
        allocation_recovery_selections: empty_phase_identity(selections, OptimizationExecutionPhase::AllocationRecovery),
        post_allocation_machine_selections: empty_phase_identity(selections, OptimizationExecutionPhase::PostAllocationMachine),
        function_relative_layout_selections: empty_phase_identity(selections, OptimizationExecutionPhase::FunctionRelativeLayout),
        pre_physical_manifest: source.manifest(),
        post_allocation_manifest: post.identity,
        selected: source.selected(),
        pre_allocation_machine_effects: machine.effects().effects().receipt().identity(),
        post_allocation_machine: machine.machine().receipt().identity(),
        baseline_pre_layout: encoding.identity(),
        pre_layout: encoding.identity(),
        baseline_resolved_layout: layout.identity(),
        resolved_layout: layout.identity(),
        x86_branch_relaxation: None,
        aarch64_cbnz_fusion: None,
        aarch64_movn_materialization: None,
        whole_function_exit_contract: exit_contract.identity(),
        target: layout.target(),
        layout_policy: layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        statistics: function_relative_statistics(layout)
            .map_err(OptimizedUnitFunctionRelativeRealizationError::Manifest)?,
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

fn empty_phase_identity(
    selections: &omega_optimization_core::OptimizationSelections,
    phase: OptimizationExecutionPhase,
) -> OptimizationSelectionIdentity {
    selections.for_phase(phase).identity()
}

fn receipt(
    source: StagedOptimizedRegisterHomeCustodyReceipt,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    exit_contract: &ValidatedWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt {
    StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt {
        source,
        machine: machine.custody().clone(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record().identity,
    }
}
