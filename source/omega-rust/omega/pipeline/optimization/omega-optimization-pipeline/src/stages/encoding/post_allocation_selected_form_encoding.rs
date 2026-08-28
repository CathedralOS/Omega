use omega_calling_conventions::MachineRegister;
use omega_isa_aarch64::{
    Aarch64SelectedFormEncodingError, aarch64_shortest_movn_materialization_recipe,
    encode_aarch64_selected_form, encode_aarch64_shortest_movn_materialization,
};
use omega_isa_x86_64::{
    ValidatedX86_64SelectedStructuralUnitCallTemplate, X86_64SelectedFormEncodingError,
    X86_64SelectedStructuralUnitCallFootprint, X86_64StructuralUnitCallTemplateError,
    X86_64StructuralUnitInternalControlFixup, encode_x86_64_selected_form,
    encode_x86_64_selected_structural_unit_call_template,
    validate_x86_64_register_constraint_catalog, x86_64_register_constraint_catalog,
};
use omega_machine_optimizer::{
    Aarch64CbnzFusionIdentity, Aarch64CbnzInstructionDisposition,
    Aarch64MovnInstructionDisposition, Aarch64MovnMaterializationIdentity,
    PostAllocationMachineIdentity, PostAllocationMachineInstruction,
    PostAllocationStructuralUnitFunction, StructuralUnitCallMachineEffects,
    StructuralUnitFunctionMachineEffects,
};
use omega_optimization_core::OptimizationSelectionIdentity;
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::{
    RegisterUnitId, RegisterViewId, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog,
};
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, MachineSizeKnowledge,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedStructuralUnitCallInstruction, SelectedStructuralUnitFunction, SelectedTerminator,
};
use omega_target::{Architecture, NativeTarget};
use psi_core::{MachineId, OperationId};
use sha2::{Digest, Sha256};

use crate::{
    StagedOptimizedAarch64CbnzFusion, StagedOptimizedAarch64MovnMaterialization,
    StagedOptimizedPostAllocationMachinePlan,
};

const ENCODER_SCHEMA: &[u8] = b"omega.terminal.layout-independent-selected-form-encoding.v6";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectedFormEncodingIdentity([u8; 32]);

impl SelectedFormEncodingIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredControlEncodingReason {
    RequiresResolvedBranchLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFormDecodedFootprint {
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub implicit_clobbers: Vec<RegisterUnitId>,
    pub encoded: MachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedFormEncodingState {
    Encoded {
        bytes: Vec<u8>,
        footprint: Box<SelectedFormDecodedFootprint>,
    },
    DeferredControl {
        reason: DeferredControlEncodingReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFormEncodingRow {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternativeKey,
    pub machine_disposition: Aarch64CbnzInstructionDisposition,
    pub state: SelectedFormEncodingState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralUnitCallEncodingRow {
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub bytes: Vec<u8>,
    pub footprint: Box<X86_64SelectedStructuralUnitCallFootprint>,
    pub fixup: X86_64StructuralUnitInternalControlFixup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralUnitFunctionEncoding {
    pub machine: MachineId,
    pub block: omega_selected_instructions::SelectedBlockId,
    pub call: Option<SelectedStructuralUnitCallEncodingRow>,
    pub return_instruction: SelectedFormEncodingRow,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelectedFormEncodingCounts {
    pub ordinary_encoded: u64,
    pub ordinary_deferred_control: u64,
    pub structural_encoded_call_templates: u64,
    pub structural_encoded_returns: u64,
    pub structural_deferred_internal_control: u64,
    pub structural_internal_fixups: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectedFormMachineOptimizationCustody {
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    fusion: Aarch64CbnzFusionIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectedFormMovnOptimizationCustody {
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    materialization: Aarch64MovnMaterializationIdentity,
}

impl SelectedFormMovnOptimizationCustody {
    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }

    pub const fn materialization(self) -> Aarch64MovnMaterializationIdentity {
        self.materialization
    }
}

impl SelectedFormMachineOptimizationCustody {
    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }

    pub const fn fusion(self) -> Aarch64CbnzFusionIdentity {
        self.fusion
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedSelectedFormEncoding {
    selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    machine: PostAllocationMachineIdentity,
    machine_optimization: Option<SelectedFormMachineOptimizationCustody>,
    movn_optimization: Option<SelectedFormMovnOptimizationCustody>,
    identity: SelectedFormEncodingIdentity,
    rows: Vec<SelectedFormEncodingRow>,
    structural_unit_functions: Vec<SelectedStructuralUnitFunctionEncoding>,
    counts: SelectedFormEncodingCounts,
}

impl StagedOptimizedSelectedFormEncoding {
    pub const fn selected(&self) -> omega_selected_instructions::SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn machine(&self) -> PostAllocationMachineIdentity {
        self.machine
    }

    pub const fn machine_optimization(&self) -> Option<SelectedFormMachineOptimizationCustody> {
        self.machine_optimization
    }

    pub const fn movn_optimization(&self) -> Option<SelectedFormMovnOptimizationCustody> {
        self.movn_optimization
    }

    pub const fn identity(&self) -> SelectedFormEncodingIdentity {
        self.identity
    }

    pub fn rows(&self) -> &[SelectedFormEncodingRow] {
        &self.rows
    }

    pub fn structural_unit_functions(&self) -> &[SelectedStructuralUnitFunctionEncoding] {
        &self.structural_unit_functions
    }

    pub const fn counts(&self) -> SelectedFormEncodingCounts {
        self.counts
    }

    #[cfg(test)]
    pub(crate) fn rows_mut(&mut self) -> &mut [SelectedFormEncodingRow] {
        &mut self.rows
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn structural_unit_functions_mut(
        &mut self,
    ) -> &mut [SelectedStructuralUnitFunctionEncoding] {
        &mut self.structural_unit_functions
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn counts_mut(&mut self) -> &mut SelectedFormEncodingCounts {
        &mut self.counts
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedSelectedFormEncodingError {
    SelectedRootMismatch,
    PhysicalModelMismatch,
    FunctionRosterMismatch,
    BlockRosterMismatch,
    InstructionRosterMismatch,
    StructuralFunctionRosterMismatch,
    StructuralConstraintCatalogMismatch,
    StructuralCallRosterMismatch(SelectedInstructionId),
    StructuralReturnRosterMismatch(SelectedInstructionId),
    OperandFootprintMismatch(SelectedInstructionId),
    ImplicitFootprintMismatch(SelectedInstructionId),
    SizeDeclarationMismatch(SelectedInstructionId),
    CountOverflow,
    X86_64(X86_64SelectedFormEncodingError),
    X86_64Structural(X86_64StructuralUnitCallTemplateError),
    Aarch64(Aarch64SelectedFormEncodingError),
    ArtifactMismatch,
}

impl std::fmt::Display for OptimizedSelectedFormEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized selected-form encoding failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedSelectedFormEncodingError {}

pub fn stage_optimized_layout_independent_selected_form_encoding<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let artifact = compute(selected, machine, physical, None, None)?;
    validate_optimized_layout_independent_selected_form_encoding(
        selected, machine, physical, &artifact,
    )?;
    Ok(artifact)
}

pub fn validate_optimized_layout_independent_selected_form_encoding<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    artifact: &StagedOptimizedSelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let replayed = compute(selected, machine, physical, None, None)?;
    if artifact != &replayed {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
}

/// Bind an independently validated symbolic CBNZ disposition into pre-layout
/// custody. Scalar bytes still validate the source forms; the disposition, not
/// this artifact, authorizes the resolved layout to omit or replace them.
/// This grants no layout, emission, section, or publication authority.
pub fn stage_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: &StagedOptimizedAarch64CbnzFusion,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let artifact = compute(selected, machine, physical, Some(fusion), None)?;
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion(
        selected, machine, physical, fusion, &artifact,
    )?;
    Ok(artifact)
}

/// Replay the complete pre-layout roster and machine-optimization custody.
pub fn validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    artifact: &StagedOptimizedSelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let replayed = compute(selected, machine, physical, Some(fusion), None)?;
    if artifact != &replayed {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
}

/// Apply an independently validated shortest-MOVN recipe to pre-layout scalar
/// bytes. The artifact still grants no layout, emission, or publication
/// authority.
pub fn stage_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    materialization: &StagedOptimizedAarch64MovnMaterialization,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let artifact = compute(selected, machine, physical, None, Some(materialization))?;
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization(
        selected,
        machine,
        physical,
        materialization,
        &artifact,
    )?;
    Ok(artifact)
}

pub fn validate_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    materialization: &StagedOptimizedAarch64MovnMaterialization,
    artifact: &StagedOptimizedSelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let replayed = compute(selected, machine, physical, None, Some(materialization))?;
    if artifact != &replayed {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
}

fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: Option<&StagedOptimizedAarch64CbnzFusion>,
    movn: Option<&StagedOptimizedAarch64MovnMaterialization>,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    if fusion.is_some() && movn.is_some() {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    let machine = staged.machine().plan();
    if machine.selected != selected.selected_identity() {
        return Err(OptimizedSelectedFormEncodingError::SelectedRootMismatch);
    }
    if machine.physical_register_model != physical.identity() {
        return Err(OptimizedSelectedFormEncodingError::PhysicalModelMismatch);
    }
    let machine_optimization = fusion
        .map(|fusion| validate_fusion_roots(selected, staged, physical, fusion))
        .transpose()?;
    let movn_optimization = movn
        .map(|materialization| validate_movn_roots(selected, staged, physical, materialization))
        .transpose()?;
    let selected_plan = selected.selected_plan();
    if selected_plan.functions.len() != machine.functions.len() {
        return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
    }
    let mut rows = Vec::new();
    for (function_index, (selected_function, machine_function)) in selected_plan
        .functions
        .iter()
        .zip(&machine.functions)
        .enumerate()
    {
        if selected_function.machine != machine_function.machine
            || selected_function.blocks.len() != machine_function.blocks.len()
        {
            return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
        }
        let fusion_function = fusion
            .map(|fusion| {
                fusion
                    .fusion()
                    .plan()
                    .functions
                    .get(function_index)
                    .ok_or(OptimizedSelectedFormEncodingError::FunctionRosterMismatch)
            })
            .transpose()?;
        let movn_function = movn
            .map(|materialization| {
                materialization
                    .materialization()
                    .plan()
                    .functions
                    .get(function_index)
                    .ok_or(OptimizedSelectedFormEncodingError::FunctionRosterMismatch)
            })
            .transpose()?;
        if fusion_function.is_some_and(|row| row.machine != selected_function.machine) {
            return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
        }
        if movn_function.is_some_and(|row| row.machine != selected_function.machine) {
            return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
        }
        for (block_index, (selected_block, machine_block)) in selected_function
            .blocks
            .iter()
            .zip(&machine_function.blocks)
            .enumerate()
        {
            if selected_block.id != machine_block.block
                || selected_block.instructions.len() + 1 != machine_block.instructions.len()
            {
                return Err(OptimizedSelectedFormEncodingError::BlockRosterMismatch);
            }
            let fusion_block = fusion_function
                .map(|function| {
                    function
                        .blocks
                        .get(block_index)
                        .ok_or(OptimizedSelectedFormEncodingError::BlockRosterMismatch)
                })
                .transpose()?;
            let movn_block = movn_function
                .map(|function| {
                    function
                        .blocks
                        .get(block_index)
                        .ok_or(OptimizedSelectedFormEncodingError::BlockRosterMismatch)
                })
                .transpose()?;
            if fusion_block.is_some_and(|row| {
                row.block != selected_block.id
                    || row.instructions.len() != machine_block.instructions.len()
            }) {
                return Err(OptimizedSelectedFormEncodingError::BlockRosterMismatch);
            }
            if movn_block.is_some_and(|row| {
                row.block != selected_block.id
                    || row.instructions.len() != machine_block.instructions.len()
            }) {
                return Err(OptimizedSelectedFormEncodingError::BlockRosterMismatch);
            }
            for (index, machine_instruction) in machine_block.instructions.iter().enumerate() {
                let selected_instruction = if index < selected_block.instructions.len() {
                    &selected_block.instructions[index]
                } else {
                    terminator_instruction(&selected_block.terminator)
                };
                if selected_instruction.id != machine_instruction.instruction {
                    return Err(OptimizedSelectedFormEncodingError::InstructionRosterMismatch);
                }
                let disposition = fusion_block
                    .map(|block| {
                        block
                            .instructions
                            .get(index)
                            .ok_or(OptimizedSelectedFormEncodingError::InstructionRosterMismatch)
                    })
                    .transpose()?;
                if disposition.is_some_and(|row| row.instruction != selected_instruction.id) {
                    return Err(OptimizedSelectedFormEncodingError::InstructionRosterMismatch);
                }
                let movn_disposition = movn_block
                    .map(|block| {
                        block
                            .instructions
                            .get(index)
                            .ok_or(OptimizedSelectedFormEncodingError::InstructionRosterMismatch)
                    })
                    .transpose()?;
                if movn_disposition.is_some_and(|row| row.instruction != selected_instruction.id) {
                    return Err(OptimizedSelectedFormEncodingError::InstructionRosterMismatch);
                }
                rows.push(encode_row(
                    selected_plan.target.architecture,
                    selected_instruction,
                    machine_instruction,
                    physical,
                    disposition
                        .map(|row| row.disposition.clone())
                        .unwrap_or(Aarch64CbnzInstructionDisposition::RetainedV1),
                    movn_disposition.map(|row| &row.disposition),
                )?);
            }
        }
    }
    let effect_plan = staged.effects().effects().plan();
    if selected_plan.structural_unit_functions.len() != machine.structural_unit_functions.len()
        || selected_plan.structural_unit_functions.len()
            != effect_plan.structural_unit_functions.len()
    {
        return Err(OptimizedSelectedFormEncodingError::StructuralFunctionRosterMismatch);
    }
    let structural_constraints = if selected_plan.structural_unit_functions.is_empty() {
        None
    } else {
        if selected_plan.target.architecture != Architecture::X86_64 {
            return Err(OptimizedSelectedFormEncodingError::StructuralFunctionRosterMismatch);
        }
        let constraints = validate_x86_64_register_constraint_catalog(
            x86_64_register_constraint_catalog(physical),
            physical,
        )
        .map_err(|_| OptimizedSelectedFormEncodingError::StructuralConstraintCatalogMismatch)?;
        if constraints.identity() != machine.register_constraints
            || constraints.identity() != effect_plan.register_constraints
        {
            return Err(OptimizedSelectedFormEncodingError::StructuralConstraintCatalogMismatch);
        }
        Some(constraints)
    };
    let mut structural_unit_functions =
        Vec::with_capacity(selected_plan.structural_unit_functions.len());
    for ((selected_function, machine_function), effect_function) in selected_plan
        .structural_unit_functions
        .iter()
        .zip(&machine.structural_unit_functions)
        .zip(&effect_plan.structural_unit_functions)
    {
        structural_unit_functions.push(encode_structural_function(
            selected_plan.target,
            selected_plan,
            selected_function,
            machine_function,
            effect_function,
            physical,
            structural_constraints
                .as_ref()
                .ok_or(OptimizedSelectedFormEncodingError::StructuralConstraintCatalogMismatch)?,
        )?);
    }
    let counts = encoding_counts(&rows, &structural_unit_functions)?;
    let selected_root = selected.selected_identity();
    let machine_root = staged.machine().receipt().identity();
    let identity = encoding_identity(
        selected_root,
        machine_root,
        machine_optimization,
        movn_optimization,
        &rows,
        &structural_unit_functions,
        counts,
    );
    Ok(StagedOptimizedSelectedFormEncoding {
        selected: selected_root,
        machine: machine_root,
        machine_optimization,
        movn_optimization,
        identity,
        rows,
        structural_unit_functions,
        counts,
    })
}

fn encode_structural_function(
    target: NativeTarget,
    selected_plan: &omega_selected_instructions::SelectedInstructionPlan,
    selected: &SelectedStructuralUnitFunction,
    machine: &PostAllocationStructuralUnitFunction,
    effects: &StructuralUnitFunctionMachineEffects,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedStructuralUnitFunctionEncoding, OptimizedSelectedFormEncodingError> {
    if selected.machine != machine.machine
        || selected.machine != effects.machine
        || selected.entry_block != machine.block
        || selected.entry_block != effects.block
    {
        return Err(OptimizedSelectedFormEncodingError::StructuralFunctionRosterMismatch);
    }
    if machine.call != effects.call
        || machine.return_effect != effects.return_effect
        || machine.return_ownership != effects.return_ownership
    {
        return Err(OptimizedSelectedFormEncodingError::StructuralFunctionRosterMismatch);
    }
    let call = match (&selected.call, &machine.call) {
        (None, None) => None,
        (Some(selected_call), Some(machine_call)) => Some(encode_structural_call(
            target,
            selected_plan,
            selected_call,
            machine_call,
            physical,
            constraints,
        )?),
        (Some(selected_call), None) => {
            return Err(
                OptimizedSelectedFormEncodingError::StructuralCallRosterMismatch(selected_call.id),
            );
        }
        (None, Some(machine_call)) => {
            return Err(
                OptimizedSelectedFormEncodingError::StructuralCallRosterMismatch(
                    machine_call.instruction,
                ),
            );
        }
    };
    let selected_return = &selected.terminator.instruction;
    if selected_return.id != machine.return_instruction.instruction
        || selected_return.id != effects.return_instruction.instruction
        || selected_return.kind != effects.return_instruction.kind
        || selected_return.provenance != machine.return_provenance
        || selected_return.provenance != effects.return_instruction.provenance
        || selected.terminator.effect != machine.return_effect
        || selected.terminator.ownership != machine.return_ownership
        || selected.terminator.effect != effects.return_effect
        || selected.terminator.ownership != effects.return_ownership
        || !effects
            .return_instruction
            .alternatives
            .contains(&machine.return_instruction.alternative)
    {
        return Err(
            OptimizedSelectedFormEncodingError::StructuralReturnRosterMismatch(selected_return.id),
        );
    }
    let return_instruction = encode_row(
        target.architecture,
        selected_return,
        &machine.return_instruction,
        physical,
        Aarch64CbnzInstructionDisposition::RetainedV1,
        None,
    )?;
    if !matches!(
        return_instruction.state,
        SelectedFormEncodingState::Encoded { ref bytes, .. } if bytes.as_slice() == [0xc3]
    ) {
        return Err(
            OptimizedSelectedFormEncodingError::StructuralReturnRosterMismatch(selected_return.id),
        );
    }
    Ok(SelectedStructuralUnitFunctionEncoding {
        machine: selected.machine,
        block: selected.entry_block,
        call,
        return_instruction,
    })
}

fn encode_structural_call(
    target: NativeTarget,
    selected_plan: &omega_selected_instructions::SelectedInstructionPlan,
    selected: &SelectedStructuralUnitCallInstruction,
    machine: &StructuralUnitCallMachineEffects,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedStructuralUnitCallEncodingRow, OptimizedSelectedFormEncodingError> {
    if machine.instruction != selected.id
        || machine.operation != selected.operation
        || machine.callee != selected.callee
        || machine.constraint != selected.constraint
        || machine.unit_uses != selected.implicit_uses
        || machine.unit_defs != selected.implicit_defs
        || machine.unit_clobbers != selected.clobbers
        || machine.layout != selected.layout
        || machine.effect != selected.effect
        || machine.ownership != selected.ownership
        || machine.claim_transfers != selected.claim_transfers
        || machine.provenance != selected.provenance
        || selected_plan
            .structural_unit_functions
            .iter()
            .filter(|function| function.machine == selected.callee)
            .count()
            != 1
    {
        return Err(OptimizedSelectedFormEncodingError::StructuralCallRosterMismatch(selected.id));
    }
    let encoded = encode_x86_64_selected_structural_unit_call_template(
        target,
        physical,
        constraints,
        selected,
        machine.declaration,
    )
    .map_err(OptimizedSelectedFormEncodingError::X86_64Structural)?;
    Ok(structural_call_encoding_row(selected, encoded))
}

fn structural_call_encoding_row(
    selected: &SelectedStructuralUnitCallInstruction,
    encoded: ValidatedX86_64SelectedStructuralUnitCallTemplate,
) -> SelectedStructuralUnitCallEncodingRow {
    SelectedStructuralUnitCallEncodingRow {
        instruction: selected.id,
        operation: selected.operation,
        callee: selected.callee,
        bytes: encoded.bytes().to_vec(),
        footprint: Box::new(encoded.footprint().clone()),
        fixup: encoded.fixup(),
    }
}

fn encoding_counts(
    rows: &[SelectedFormEncodingRow],
    structural: &[SelectedStructuralUnitFunctionEncoding],
) -> Result<SelectedFormEncodingCounts, OptimizedSelectedFormEncodingError> {
    let mut counts = SelectedFormEncodingCounts::default();
    for row in rows {
        let count = match row.state {
            SelectedFormEncodingState::Encoded { .. } => &mut counts.ordinary_encoded,
            SelectedFormEncodingState::DeferredControl { .. } => {
                &mut counts.ordinary_deferred_control
            }
        };
        *count = count
            .checked_add(1)
            .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
    }
    for function in structural {
        counts.structural_encoded_returns = counts
            .structural_encoded_returns
            .checked_add(1)
            .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
        if function.call.is_some() {
            counts.structural_encoded_call_templates = counts
                .structural_encoded_call_templates
                .checked_add(1)
                .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
            counts.structural_deferred_internal_control = counts
                .structural_deferred_internal_control
                .checked_add(1)
                .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
            counts.structural_internal_fixups = counts
                .structural_internal_fixups
                .checked_add(1)
                .ok_or(OptimizedSelectedFormEncodingError::CountOverflow)?;
        }
    }
    Ok(counts)
}

fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}

fn encode_row(
    architecture: Architecture,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    machine_disposition: Aarch64CbnzInstructionDisposition,
    movn_disposition: Option<&Aarch64MovnInstructionDisposition>,
) -> Result<SelectedFormEncodingRow, OptimizedSelectedFormEncodingError> {
    validate_machine_disposition(
        architecture,
        selected,
        machine,
        physical,
        &machine_disposition,
    )?;
    let alternative = machine.alternative.key;
    let state = match (selected.kind, movn_disposition) {
        (kind @ SelectedInstructionKind::MaterializeI64 { .. }, Some(disposition)) => {
            encode_aarch64_movn_row(architecture, selected, kind, machine, physical, disposition)?
        }
        (SelectedInstructionKind::ConditionalBranchNonZero, _) => {
            SelectedFormEncodingState::DeferredControl {
                reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
            }
        }
        (kind, Some(Aarch64MovnInstructionDisposition::RetainedV1)) | (kind, None) => {
            encode_scalar(
                architecture,
                selected.id,
                kind,
                alternative,
                machine,
                physical,
            )?
        }
        (_, Some(Aarch64MovnInstructionDisposition::MovnSeededMaterializationV1 { .. })) => {
            return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
        }
    };
    Ok(SelectedFormEncodingRow {
        instruction: selected.id,
        alternative,
        machine_disposition,
        state,
    })
}

fn encode_aarch64_movn_row(
    architecture: Architecture,
    selected: &SelectedInstruction,
    kind: SelectedInstructionKind,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    disposition: &Aarch64MovnInstructionDisposition,
) -> Result<SelectedFormEncodingState, OptimizedSelectedFormEncodingError> {
    let Aarch64MovnInstructionDisposition::MovnSeededMaterializationV1 {
        literal_bits,
        destination,
        baseline_word_count,
        recipe,
    } = disposition
    else {
        return encode_scalar(
            architecture,
            selected.id,
            kind,
            machine.alternative.key,
            machine,
            physical,
        );
    };
    let SelectedInstructionKind::MaterializeI64 { value } = kind else {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
    };
    let operand = machine
        .operands
        .first()
        .filter(|_| machine.operands.len() == 1);
    let valid_destination = operand.is_some_and(|operand| {
        architecture == Architecture::Aarch64
            && destination.instruction == selected.id
            && destination.operand == operand.operand
            && destination.virtual_register == operand.virtual_register
            && destination.class == operand.class
            && destination.view == operand.view
            && destination.storage_units == operand.storage_units
            && destination.write_units == operand.write_units
            && Some(destination.write_semantics) == operand.write_semantics
    });
    if !valid_destination || integer_bits(value) != Some(*literal_bits) {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
    }
    let isa_recipe = aarch64_shortest_movn_materialization_recipe(value)
        .map_err(OptimizedSelectedFormEncodingError::Aarch64)?;
    let recipe_matches = usize::from(*baseline_word_count) * 4 == isa_recipe.baseline_byte_count()
        && recipe.seed_halfword == isa_recipe.seed().halfword()
        && recipe.seed_immediate == isa_recipe.seed().immediate()
        && recipe.patches.len() == isa_recipe.patches().len()
        && recipe
            .patches
            .iter()
            .zip(isa_recipe.patches())
            .all(|(left, right)| {
                left.halfword == right.halfword() && left.immediate == right.immediate()
            });
    if !recipe_matches {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    let encoded = encode_aarch64_shortest_movn_materialization(physical, destination.view, value)
        .map_err(OptimizedSelectedFormEncodingError::Aarch64)?;
    let footprint = encoded.footprint();
    validate_operand_footprint(
        selected.id,
        machine,
        &footprint.encoded,
        &footprint.register_reads,
        &footprint.register_writes,
    )?;
    if footprint.encoded != machine.alternative.encoded {
        return Err(OptimizedSelectedFormEncodingError::ImplicitFootprintMismatch(selected.id));
    }
    validate_size(selected.id, machine.alternative.size, encoded.bytes().len())?;
    Ok(SelectedFormEncodingState::Encoded {
        bytes: encoded.bytes().to_vec(),
        footprint: Box::new(SelectedFormDecodedFootprint {
            register_reads: footprint.register_reads.clone(),
            register_writes: footprint.register_writes.clone(),
            implicit_defs: footprint.encoded.implicit_unit_defs.clone(),
            implicit_clobbers: footprint.encoded.implicit_unit_clobbers.clone(),
            encoded: footprint.encoded.clone(),
        }),
    })
}

fn integer_bits(value: psi_core::IntegerValue) -> Option<u64> {
    match value {
        psi_core::IntegerValue::Signed(value) => {
            i64::try_from(value).ok().map(|value| value as u64)
        }
        psi_core::IntegerValue::Unsigned(value) => u64::try_from(value).ok(),
    }
}

fn validate_machine_disposition(
    architecture: Architecture,
    selected: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    disposition: &Aarch64CbnzInstructionDisposition,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let valid = match disposition {
        Aarch64CbnzInstructionDisposition::RetainedV1 => true,
        Aarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { consumer } => {
            architecture == Architecture::Aarch64
                && matches!(selected.kind, SelectedInstructionKind::CompareI64Zero)
                && *consumer != selected.id
        }
        Aarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
            compare,
            source_read,
        } => {
            let view = physical
                .model()
                .views
                .iter()
                .find(|view| view.id == source_read.view);
            architecture == Architecture::Aarch64
                && matches!(
                    selected.kind,
                    SelectedInstructionKind::ConditionalBranchNonZero
                )
                && machine.operands.is_empty()
                && *compare == source_read.source_instruction
                && *compare != selected.id
                && source_read.operand == 0
                && view.is_some_and(|view| {
                    view.class == source_read.class && view.units == source_read.units
                })
        }
    };
    if !valid {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(selected.id));
    }
    Ok(())
}

fn encode_scalar(
    architecture: Architecture,
    instruction: SelectedInstructionId,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<SelectedFormEncodingState, OptimizedSelectedFormEncodingError> {
    let views = machine
        .operands
        .iter()
        .map(|operand| operand.view)
        .collect::<Vec<_>>();
    let (bytes, reads, writes, encoded_effects) = match architecture {
        Architecture::X86_64 => {
            let encoded = encode_x86_64_selected_form(physical, kind, alternative, &views)
                .map_err(OptimizedSelectedFormEncodingError::X86_64)?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().register_writes.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
        Architecture::Aarch64 => {
            let encoded = encode_aarch64_selected_form(physical, kind, alternative, &views)
                .map_err(OptimizedSelectedFormEncodingError::Aarch64)?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().register_writes.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
    };
    validate_operand_footprint(instruction, machine, &encoded_effects, &reads, &writes)?;
    if encoded_effects != machine.alternative.encoded {
        return Err(OptimizedSelectedFormEncodingError::ImplicitFootprintMismatch(instruction));
    }
    validate_size(instruction, machine.alternative.size, bytes.len())?;
    Ok(SelectedFormEncodingState::Encoded {
        bytes,
        footprint: Box::new(SelectedFormDecodedFootprint {
            register_reads: reads,
            register_writes: writes,
            implicit_defs: encoded_effects.implicit_unit_defs.clone(),
            implicit_clobbers: encoded_effects.implicit_unit_clobbers.clone(),
            encoded: encoded_effects,
        }),
    })
}

fn validate_operand_footprint(
    instruction: SelectedInstructionId,
    machine: &PostAllocationMachineInstruction,
    encoded: &MachineEncodedEffects,
    reads: &[RegisterViewId],
    writes: &[RegisterViewId],
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let resolve = |operand: u16| {
        machine
            .operands
            .iter()
            .find(|row| row.operand == operand)
            .map(|row| row.view)
    };
    let expected_reads = encoded
        .external_operand_reads
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(instruction))?;
    let expected_writes = encoded
        .external_operand_writes
        .iter()
        .map(|operand| resolve(*operand))
        .collect::<Option<Vec<_>>>()
        .ok_or(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(instruction))?;
    if reads != expected_reads || writes != expected_writes {
        return Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(instruction));
    }
    Ok(())
}

fn validate_size(
    instruction: SelectedInstructionId,
    knowledge: MachineSizeKnowledge,
    actual: usize,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let actual = u16::try_from(actual)
        .map_err(|_| OptimizedSelectedFormEncodingError::SizeDeclarationMismatch(instruction))?;
    let matches = match knowledge {
        MachineSizeKnowledge::ExactBytes(expected) => actual == expected,
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes,
            maximum_bytes,
        } => actual >= minimum_bytes && maximum_bytes.is_none_or(|maximum| actual <= maximum),
    };
    if !matches {
        return Err(OptimizedSelectedFormEncodingError::SizeDeclarationMismatch(
            instruction,
        ));
    }
    Ok(())
}

fn encoding_identity(
    selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    machine: PostAllocationMachineIdentity,
    machine_optimization: Option<SelectedFormMachineOptimizationCustody>,
    movn_optimization: Option<SelectedFormMovnOptimizationCustody>,
    rows: &[SelectedFormEncodingRow],
    structural_unit_functions: &[SelectedStructuralUnitFunctionEncoding],
    counts: SelectedFormEncodingCounts,
) -> SelectedFormEncodingIdentity {
    let mut hasher = Sha256::new();
    hasher.update(ENCODER_SCHEMA);
    hasher.update(selected.bytes());
    hasher.update(machine.bytes());
    match machine_optimization {
        None => hasher.update([0]),
        Some(custody) => {
            hasher.update([1]);
            hasher.update(custody.selections.bytes());
            hasher.update(custody.post_allocation_machine_selections.bytes());
            hasher.update(custody.fusion.bytes());
        }
    }
    match movn_optimization {
        None => hasher.update([0]),
        Some(custody) => {
            hasher.update([1]);
            hasher.update(custody.selections.bytes());
            hasher.update(custody.post_allocation_machine_selections.bytes());
            hasher.update(custody.materialization.bytes());
        }
    }
    hasher.update((rows.len() as u64).to_le_bytes());
    for row in rows {
        encode_encoding_row(&mut hasher, row);
    }
    hasher.update((structural_unit_functions.len() as u64).to_le_bytes());
    for function in structural_unit_functions {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.block.0.to_le_bytes());
        match &function.call {
            None => hasher.update([0]),
            Some(call) => {
                hasher.update([1]);
                hasher.update(call.instruction.0.to_le_bytes());
                hasher.update(call.operation.get().to_le_bytes());
                hasher.update(call.callee.get().to_le_bytes());
                hasher.update((call.bytes.len() as u64).to_le_bytes());
                hasher.update(&call.bytes);
                encode_structural_footprint(&mut hasher, &call.footprint);
                encode_structural_fixup(&mut hasher, call.fixup);
            }
        }
        encode_encoding_row(&mut hasher, &function.return_instruction);
    }
    encode_counts(&mut hasher, counts);
    SelectedFormEncodingIdentity(hasher.finalize().into())
}

fn encode_encoding_row(hasher: &mut Sha256, row: &SelectedFormEncodingRow) {
    hasher.update(row.instruction.0.to_le_bytes());
    encode_alternative(hasher, row.alternative);
    encode_machine_disposition(hasher, &row.machine_disposition);
    match &row.state {
        SelectedFormEncodingState::Encoded { bytes, footprint } => {
            hasher.update([0]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
            encode_views(hasher, &footprint.register_reads);
            encode_views(hasher, &footprint.register_writes);
            encode_units(hasher, &footprint.implicit_defs);
            encode_units(hasher, &footprint.implicit_clobbers);
            encode_effects(hasher, &footprint.encoded);
        }
        SelectedFormEncodingState::DeferredControl { reason } => {
            hasher.update([1]);
            hasher.update([match reason {
                DeferredControlEncodingReason::RequiresResolvedBranchLayout => 0,
            }]);
        }
    }
}

fn encode_structural_footprint(
    hasher: &mut Sha256,
    footprint: &X86_64SelectedStructuralUnitCallFootprint,
) {
    encode_units(hasher, &footprint.implicit_unit_uses);
    encode_units(hasher, &footprint.implicit_unit_defs);
    encode_units(hasher, &footprint.implicit_unit_clobbers);
    for read in footprint.root_reads {
        encode_machine_register(hasher, read.root);
        hasher.update(read.byte_offset.to_le_bytes());
        hasher.update(read.byte_count.to_le_bytes());
    }
    for write in footprint.caller_copy_writes {
        hasher.update(write.stack_byte_offset.to_le_bytes());
        hasher.update(write.byte_count.to_le_bytes());
    }
    for register in footprint.scratch_register_writes {
        encode_machine_register(hasher, register);
    }
    for write in footprint.argument_pointer_writes {
        encode_machine_register(hasher, write.register);
        hasher.update(write.stack_byte_offset.to_le_bytes());
    }
    hasher.update([u8::from(footprint.writes_rflags)]);
    hasher.update(footprint.frame_byte_count.to_le_bytes());
    hasher.update(footprint.shadow_byte_count.to_le_bytes());
    hasher.update(footprint.pre_call_stack_alignment.to_le_bytes());
    hasher.update([u8::from(footprint.frame_is_balanced)]);
    hasher.update([match footprint.trap {
        omega_selected_instructions::MachineTrapBehavior::NeverV1 => 0,
        omega_selected_instructions::MachineTrapBehavior::MayArchitecturalFaultV1 => 1,
    }]);
    hasher.update([match footprint.barrier {
        omega_selected_instructions::StructuralUnitCallBarrier::CallV1 => 0,
    }]);
    hasher.update([match footprint.call {
        omega_selected_instructions::StructuralUnitCallEffect::DirectInternalUnitV1 => 0,
    }]);
    hasher.update([match footprint.cleanup {
        omega_selected_instructions::MachineCleanupEffect::NoneV1 => 0,
    }]);
}

fn encode_structural_fixup(hasher: &mut Sha256, fixup: X86_64StructuralUnitInternalControlFixup) {
    hasher.update([match fixup.kind {
        omega_isa_x86_64::X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1 => 0,
    }]);
    hasher.update([match fixup.state {
        omega_isa_x86_64::X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1 => 0,
    }]);
    hasher.update(fixup.callee.get().to_le_bytes());
    hasher.update(fixup.opcode_byte_offset.to_le_bytes());
    hasher.update(fixup.field_byte_offset.to_le_bytes());
    hasher.update(fixup.next_instruction_byte_offset.to_le_bytes());
    hasher.update([fixup.field_byte_width]);
    hasher.update(fixup.addend.to_le_bytes());
}

fn encode_counts(hasher: &mut Sha256, counts: SelectedFormEncodingCounts) {
    for count in [
        counts.ordinary_encoded,
        counts.ordinary_deferred_control,
        counts.structural_encoded_call_templates,
        counts.structural_encoded_returns,
        counts.structural_deferred_internal_control,
        counts.structural_internal_fixups,
    ] {
        hasher.update(count.to_le_bytes());
    }
}

fn validate_fusion_roots<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: &StagedOptimizedAarch64CbnzFusion,
) -> Result<SelectedFormMachineOptimizationCustody, OptimizedSelectedFormEncodingError> {
    let receipt = fusion.fusion().receipt();
    let plan = fusion.fusion().plan();
    let custody = fusion.custody();
    if selected.selected_plan().target.architecture != Architecture::Aarch64
        || receipt.selected() != selected.selected_identity()
        || receipt.source() != machine.machine().receipt().identity()
        || receipt.identity() != custody.fusion()
        || receipt.action_count() != custody.action_count()
        || plan.target != selected.selected_plan().target
        || plan.physical_register_model != physical.identity()
    {
        return Err(OptimizedSelectedFormEncodingError::SelectedRootMismatch);
    }
    Ok(SelectedFormMachineOptimizationCustody {
        selections: custody.selections(),
        post_allocation_machine_selections: custody.post_allocation_machine_selections(),
        fusion: custody.fusion(),
    })
}

fn validate_movn_roots<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    materialization: &StagedOptimizedAarch64MovnMaterialization,
) -> Result<SelectedFormMovnOptimizationCustody, OptimizedSelectedFormEncodingError> {
    let receipt = materialization.materialization().receipt();
    let plan = materialization.materialization().plan();
    let custody = materialization.custody();
    if selected.selected_plan().target.architecture != Architecture::Aarch64
        || receipt.selected() != selected.selected_identity()
        || receipt.source() != machine.machine().receipt().identity()
        || receipt.identity() != custody.materialization()
        || receipt.action_count() != custody.action_count()
        || receipt.baseline_words() != custody.baseline_words()
        || receipt.selected_words() != custody.selected_words()
        || plan.target != selected.selected_plan().target
        || plan.physical_register_model != physical.identity()
    {
        return Err(OptimizedSelectedFormEncodingError::SelectedRootMismatch);
    }
    Ok(SelectedFormMovnOptimizationCustody {
        selections: custody.selections(),
        post_allocation_machine_selections: custody.post_allocation_machine_selections(),
        materialization: custody.materialization(),
    })
}

pub(crate) fn machine_optimization_custody(
    fusion: &StagedOptimizedAarch64CbnzFusion,
) -> SelectedFormMachineOptimizationCustody {
    SelectedFormMachineOptimizationCustody {
        selections: fusion.custody().selections(),
        post_allocation_machine_selections: fusion.custody().post_allocation_machine_selections(),
        fusion: fusion.custody().fusion(),
    }
}

fn encode_machine_disposition(
    hasher: &mut Sha256,
    disposition: &Aarch64CbnzInstructionDisposition,
) {
    match disposition {
        Aarch64CbnzInstructionDisposition::RetainedV1 => hasher.update([0]),
        Aarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { consumer } => {
            hasher.update([1]);
            hasher.update(consumer.0.to_le_bytes());
        }
        Aarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
            compare,
            source_read,
        } => {
            hasher.update([2]);
            hasher.update(compare.0.to_le_bytes());
            hasher.update(source_read.source_instruction.0.to_le_bytes());
            hasher.update(source_read.operand.to_le_bytes());
            hasher.update(source_read.virtual_register.0.to_le_bytes());
            hasher.update(source_read.class.0.to_le_bytes());
            hasher.update(source_read.view.0.to_le_bytes());
            encode_units(hasher, &source_read.units);
        }
    }
}

fn encode_effects(hasher: &mut Sha256, effects: &MachineEncodedEffects) {
    hasher.update((effects.external_operand_reads.len() as u64).to_le_bytes());
    for operand in &effects.external_operand_reads {
        hasher.update(operand.to_le_bytes());
    }
    hasher.update((effects.external_operand_writes.len() as u64).to_le_bytes());
    for operand in &effects.external_operand_writes {
        hasher.update(operand.to_le_bytes());
    }
    encode_units(hasher, &effects.implicit_unit_uses);
    encode_units(hasher, &effects.implicit_unit_defs);
    encode_units(hasher, &effects.implicit_unit_clobbers);
    use omega_selected_instructions::{
        MachineEncodedControlEffect as Control, MachineEncodedMemoryEffect as Memory,
        MachineEncodedStackEffect as Stack, MachineEncodedTrapBehavior as Trap,
    };
    match effects.memory {
        Memory::NoneV1 => hasher.update([0]),
        Memory::ReadActivationStackV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
    }
    match effects.stack {
        Stack::UnchangedV1 => hasher.update([0]),
        Stack::PopBytesV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
    }
    hasher.update([match effects.trap {
        Trap::NeverV1 => 0,
        Trap::MayArchitecturalFaultV1 => 1,
    }]);
    match effects.control {
        Control::FallThroughV1 => hasher.update([0]),
        Control::ConditionalRelativeBranchV1 => hasher.update([1]),
        Control::ReturnFromActivationStackV1 => hasher.update([2]),
        Control::ReturnIndirectRegisterV1 { target } => {
            hasher.update([3]);
            hasher.update(target.0.to_le_bytes());
        }
    }
}

fn encode_alternative(hasher: &mut Sha256, alternative: MachineAlternativeKey) {
    hasher.update([match alternative.family {
        MachineAlternativeFamily::CompareI64Zero => 0,
        MachineAlternativeFamily::MaterializeI64 => 1,
        MachineAlternativeFamily::CopyI64 => 2,
        MachineAlternativeFamily::ExactAddI64 => 3,
        MachineAlternativeFamily::ExactAddI64Immediate => 4,
        MachineAlternativeFamily::ExactSubtractI64 => 5,
        MachineAlternativeFamily::ConditionalBranchNonZero => 6,
        MachineAlternativeFamily::ReturnI64 => 7,
        MachineAlternativeFamily::ExactSubtractI64Immediate => 8,
        MachineAlternativeFamily::ReturnUnit => 9,
    }]);
    hasher.update(alternative.variant.to_le_bytes());
}

fn encode_views(hasher: &mut Sha256, views: &[RegisterViewId]) {
    hasher.update((views.len() as u64).to_le_bytes());
    for view in views {
        hasher.update(view.0.to_le_bytes());
    }
}

fn encode_units(hasher: &mut Sha256, units: &[RegisterUnitId]) {
    hasher.update((units.len() as u64).to_le_bytes());
    for unit in units {
        hasher.update(unit.0.to_le_bytes());
    }
}

fn encode_machine_register(hasher: &mut Sha256, register: MachineRegister) {
    let (tag, index) = match register {
        MachineRegister::X86Rax => (1, 0),
        MachineRegister::X86Rcx => (2, 0),
        MachineRegister::X86Rdx => (3, 0),
        MachineRegister::X86Rbx => (4, 0),
        MachineRegister::X86Rsp => (5, 0),
        MachineRegister::X86Rbp => (6, 0),
        MachineRegister::X86Rsi => (7, 0),
        MachineRegister::X86Rdi => (8, 0),
        MachineRegister::X86R8 => (9, 0),
        MachineRegister::X86R9 => (10, 0),
        MachineRegister::X86R10 => (11, 0),
        MachineRegister::X86R11 => (12, 0),
        MachineRegister::X86R12 => (13, 0),
        MachineRegister::X86R13 => (14, 0),
        MachineRegister::X86R14 => (15, 0),
        MachineRegister::X86R15 => (16, 0),
        MachineRegister::X86Xmm(index) => (17, index),
        MachineRegister::Aarch64X(index) => (18, index),
        MachineRegister::Aarch64V(index) => (19, index),
    };
    hasher.update([tag, index]);
}
