use omega_optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationExecutionPhase,
    OptimizationSelectionIdentity,
};
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_selected_instructions::{SelectedInstructionKind, SelectedInstructionPlan};
use omega_target::{Architecture, ObjectFormat};

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

/// Owning function-relative custody for the bounded structural-signature Unit
/// route. The internal call remains a typed unresolved MachineId fixup; this
/// carrier grants no section placement, object relocation, or executable-byte
/// authority.
#[derive(Debug)]
pub struct StagedOptimizedStructuralUnitFunctionRelativeRealization {
    homes: StagedOptimizedRegisterHomes,
    machine: StagedOptimizedPostAllocationMachinePlan,
    encoding: StagedOptimizedSelectedFormEncoding,
    layout: StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: ValidatedWholeFunctionExitContract,
    manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
    custody: StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt,
}

impl StagedOptimizedStructuralUnitFunctionRelativeRealization {
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

    pub const fn custody(
        &self,
    ) -> &StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt {
        &self.custody
    }

    #[cfg(test)]
    pub(crate) fn layout_mut(&mut self) -> &mut StagedOptimizedResolvedSelectedFormLayout {
        &mut self.layout
    }

    #[cfg(test)]
    pub(crate) fn exit_contract_mut(&mut self) -> &mut ValidatedWholeFunctionExitContract {
        &mut self.exit_contract
    }

    #[cfg(test)]
    pub(crate) fn manifest_mut(
        &mut self,
    ) -> &mut ValidatedFunctionRelativeOptimizationRealizationManifest {
        &mut self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt {
    source: StagedOptimizedRegisterHomeCustodyReceipt,
    machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    exit_contract: WholeFunctionExitContractIdentity,
    realization: FunctionRelativeOptimizationRealizationManifestIdentity,
}

impl StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt {
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
pub enum OptimizedStructuralUnitFunctionRelativeRealizationError {
    Homes(OptimizedRegisterHomeCustodyError),
    Machine(OptimizedPostAllocationMachinePipelineError),
    Encoding(crate::OptimizedSelectedFormEncodingError),
    Layout(crate::OptimizedResolvedSelectedFormLayoutError),
    Exit(crate::WholeFunctionExitContractError),
    UnsupportedSelectionPhase,
    UnsupportedStructuralUnitShape,
    RootMismatch,
    ReceiptMismatch,
    Manifest(FunctionRelativeOptimizationRealizationError),
}

impl std::fmt::Display for OptimizedStructuralUnitFunctionRelativeRealizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized structural Unit function-relative realization failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedStructuralUnitFunctionRelativeRealizationError {}

pub fn stage_optimized_structural_unit_function_relative_realization(
    homes: StagedOptimizedRegisterHomes,
) -> Result<
    StagedOptimizedStructuralUnitFunctionRelativeRealization,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
> {
    let source = validate_source(&homes)?;
    let selected_stage = selected_stage(&homes);
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    let machine = stage_optimized_post_allocation_machine_plan(&homes)
        .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Machine)?;
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, &machine, physical)
            .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Encoding)?;
    let layout =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Layout)?;
    let exit_contract =
        stage_whole_function_exit_contract(selected, &machine, physical, &encoding, &layout)
            .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Exit)?;
    let manifest = expected_manifest(&homes, &machine, &encoding, &layout, &exit_contract)?;
    let custody = receipt(source, &machine, &exit_contract, &manifest);
    Ok(StagedOptimizedStructuralUnitFunctionRelativeRealization {
        homes,
        machine,
        encoding,
        layout,
        exit_contract,
        manifest,
        custody,
    })
}

pub fn validate_optimized_structural_unit_function_relative_realization(
    staged: &StagedOptimizedStructuralUnitFunctionRelativeRealization,
) -> Result<
    StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
> {
    let source = validate_source(&staged.homes)?;
    let machine =
        validate_optimized_post_allocation_machine_plan_custody(&staged.homes, &staged.machine)
            .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Machine)?;
    if &machine != staged.machine.custody() {
        return Err(OptimizedStructuralUnitFunctionRelativeRealizationError::ReceiptMismatch);
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
    .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.layout,
    )
    .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Layout)?;
    validate_whole_function_exit_contract(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.layout,
        &staged.exit_contract,
    )
    .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Exit)?;
    let manifest = expected_manifest(
        &staged.homes,
        &staged.machine,
        &staged.encoding,
        &staged.layout,
        &staged.exit_contract,
    )?;
    if manifest.record() != staged.manifest.record() {
        return Err(OptimizedStructuralUnitFunctionRelativeRealizationError::RootMismatch);
    }
    let custody = receipt(source, &staged.machine, &staged.exit_contract, &manifest);
    if custody != staged.custody {
        return Err(OptimizedStructuralUnitFunctionRelativeRealizationError::ReceiptMismatch);
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
) -> Result<
    StagedOptimizedRegisterHomeCustodyReceipt,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
> {
    let source = validate_optimized_register_home_custody(
        homes.legality_stage(),
        homes.homes(),
        homes.post_allocation_manifest(),
    )
    .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Homes)?;
    if source != homes.custody() {
        return Err(OptimizedStructuralUnitFunctionRelativeRealizationError::ReceiptMismatch);
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
        return Err(
            OptimizedStructuralUnitFunctionRelativeRealizationError::UnsupportedSelectionPhase,
        );
    }
    validate_structural_unit_shape(selected_stage.selected().selected_plan())?;
    Ok(source)
}

fn validate_structural_unit_shape(
    selected: &SelectedInstructionPlan,
) -> Result<(), OptimizedStructuralUnitFunctionRelativeRealizationError> {
    if selected.target.architecture != Architecture::X86_64
        || selected.target.object_format != ObjectFormat::Coff
        || !selected.functions.is_empty()
        || selected.structural_unit_functions.is_empty()
        || !selected
            .structural_unit_functions
            .iter()
            .any(|function| function.machine == selected.entry)
    {
        return Err(
            OptimizedStructuralUnitFunctionRelativeRealizationError::UnsupportedStructuralUnitShape,
        );
    }
    for function in &selected.structural_unit_functions {
        if function.terminator.instruction.kind != SelectedInstructionKind::ReturnUnit
            || !function.terminator.instruction.operands.is_empty()
            || function.call.as_ref().is_some_and(|call| {
                call.id == function.terminator.instruction.id
                    || !selected
                        .structural_unit_functions
                        .iter()
                        .any(|callee| callee.machine == call.callee)
            })
        {
            return Err(
                OptimizedStructuralUnitFunctionRelativeRealizationError::UnsupportedStructuralUnitShape,
            );
        }
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
    OptimizedStructuralUnitFunctionRelativeRealizationError,
> {
    let selected_stage = selected_stage(homes);
    let optimized = selected_stage.optimized_target().optimized();
    let selections = optimized.selections();
    let source = homes.custody();
    let post = homes.post_allocation_manifest().record();
    let selected_plan = selected_stage.selected().selected_plan();
    let structural_function_count = u64::try_from(selected_plan.structural_unit_functions.len())
        .map_err(|_| {
            OptimizedStructuralUnitFunctionRelativeRealizationError::Manifest(
                FunctionRelativeOptimizationRealizationError::StatisticsOverflow,
            )
        })?;
    if post.selected_lowering_completion.is_some()
        || post.selected != source.selected()
        || post.statistics.functions != 0
        || post.statistics.structural_unit_functions != structural_function_count
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
        return Err(OptimizedStructuralUnitFunctionRelativeRealizationError::RootMismatch);
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
        whole_function_exit_contract: exit_contract.identity(),
        target: layout.target(),
        layout_policy: layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        statistics: function_relative_statistics(layout)
            .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Manifest)?,
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
) -> StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt {
    StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt {
        source,
        machine: machine.custody().clone(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record().identity,
    }
}
