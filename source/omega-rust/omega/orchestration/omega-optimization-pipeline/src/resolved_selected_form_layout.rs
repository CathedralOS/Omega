use std::collections::BTreeMap;

use omega_calling_conventions::MachineRegister;
use omega_machine_optimizer::{
    TerminalAarch64CbnzFusionAction, TerminalAarch64CbnzInstructionDisposition,
    TerminalPostAllocationMachineInstruction, TerminalQualifiedPhysicalRead,
};
use omega_regalloc::ValidatedTerminalSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_isa_aarch64::{
    Aarch64SelectedFormEncodingError,
    encode_aarch64_terminal_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    encode_aarch64_terminal_selected_nonzero_branch_form,
};
use omega_terminal_isa_x86_64::{
    X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET, X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET,
    X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET, X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH,
    X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT, X86_64SelectedFormEncodingError,
    X86_64SelectedStructuralUnitCallFootprint, X86_64StructuralUnitInternalControlFixup,
    X86_64StructuralUnitInternalControlFixupKind, X86_64StructuralUnitInternalControlFixupState,
    encode_x86_64_terminal_selected_nonzero_branch_form,
};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternativeKey, TerminalMachineEncodedControlEffect,
    TerminalMachineEncodedEffects, TerminalMachineEncodedMemoryEffect,
    TerminalMachineEncodedStackEffect, TerminalMachineEncodedTrapBehavior,
    TerminalMachineSizeKnowledge, TerminalSelectedBlock, TerminalSelectedBlockId,
    TerminalSelectedFunction, TerminalSelectedInstruction, TerminalSelectedInstructionId,
    TerminalSelectedTerminator,
};
use psi_core::{EdgeId, MachineId, OperationId};
use sha2::{Digest, Sha256};

use crate::{
    DeferredTerminalControlEncodingReason, OptimizedSelectedFormEncodingError,
    StagedOptimizedAarch64CbnzFusion, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedSelectedFormEncoding, TerminalSelectedFormEncodingRow,
    TerminalSelectedFormEncodingState, TerminalSelectedFormMachineOptimizationCustody,
    TerminalSelectedStructuralUnitFunctionEncoding,
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion,
};

const LAYOUT_SCHEMA: &[u8] = b"omega.terminal.resolved-selected-form-layout.v5";

/// Required-stage baseline layout for the currently admitted three-block
/// conditional. This is a visible policy identity, not an optimization level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalSelectedFunctionLayoutPolicy {
    EntryThenZeroFallthroughThenNonzeroV1,
    SingleEntryBlockV1,
    /// A separate zero-VReg structural roster. Every function has one entry
    /// block containing either `ReturnUnit`, or one unresolved whole-root
    /// `CallUnit` template followed by `ReturnUnit`.
    StructuralUnitCallThenReturnSingleEntryBlockV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalResolvedSelectedFormLayoutIdentity([u8; 32]);

impl TerminalResolvedSelectedFormLayoutIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalResolvedConditionalBranchEvidence {
    pub source_block: TerminalSelectedBlockId,
    pub when_nonzero_edge: EdgeId,
    pub when_nonzero_block: TerminalSelectedBlockId,
    pub when_nonzero_offset: u64,
    pub when_zero_edge: EdgeId,
    pub when_zero_block: TerminalSelectedBlockId,
    pub when_zero_offset: u64,
    /// x86-64 measures from instruction end; AArch64 measures from the branch
    /// word address. The target decoder independently checks this convention.
    pub byte_displacement: i64,
    pub decoded_register_reads: Vec<omega_register_model::RegisterViewId>,
    pub decoded_effects: TerminalMachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalResolvedSelectedFormRow {
    pub instruction: TerminalSelectedInstructionId,
    pub alternative: TerminalMachineAlternativeKey,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub branch: Option<Box<TerminalResolvedConditionalBranchEvidence>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalResolvedSelectedBlockLayout {
    pub block: TerminalSelectedBlockId,
    pub offset: u64,
    pub byte_count: u64,
    pub instructions: Vec<TerminalResolvedSelectedFormRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalResolvedSelectedFunctionLayout {
    pub machine: MachineId,
    pub byte_count: u64,
    pub blocks: Vec<TerminalResolvedSelectedBlockLayout>,
}

/// Function-relative custody for the canonical structural Unit call template.
/// The bytes deliberately retain their zero rel32 placeholder; `fixup` remains
/// unresolved until whole-text placement knows both MachineId coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalResolvedStructuralUnitCallLayout {
    pub instruction: TerminalSelectedInstructionId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub footprint: Box<X86_64SelectedStructuralUnitCallFootprint>,
    pub fixup: X86_64StructuralUnitInternalControlFixup,
}

/// Exact one-block function-relative span for the bounded structural Unit
/// route. A caller is 89 unresolved call bytes plus one `C3`; a leaf is the
/// single `C3` byte. This carrier grants neither section placement nor
/// executable-byte authority while `call.fixup` remains unresolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalResolvedStructuralUnitFunctionLayout {
    pub machine: MachineId,
    pub block: TerminalSelectedBlockId,
    pub offset: u64,
    pub byte_count: u64,
    pub call: Option<TerminalResolvedStructuralUnitCallLayout>,
    pub return_instruction: TerminalResolvedSelectedFormRow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedResolvedSelectedFormLayout {
    selected: omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity,
    machine: omega_machine_optimizer::TerminalPostAllocationMachineIdentity,
    pre_layout: crate::TerminalSelectedFormEncodingIdentity,
    machine_optimization: Option<TerminalSelectedFormMachineOptimizationCustody>,
    target: NativeTarget,
    policy: TerminalSelectedFunctionLayoutPolicy,
    identity: TerminalResolvedSelectedFormLayoutIdentity,
    functions: Vec<TerminalResolvedSelectedFunctionLayout>,
    structural_unit_functions: Vec<TerminalResolvedStructuralUnitFunctionLayout>,
}

impl StagedOptimizedResolvedSelectedFormLayout {
    pub const fn selected(
        &self,
    ) -> omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn machine(&self) -> omega_machine_optimizer::TerminalPostAllocationMachineIdentity {
        self.machine
    }

    pub const fn pre_layout(&self) -> crate::TerminalSelectedFormEncodingIdentity {
        self.pre_layout
    }

    pub const fn machine_optimization(
        &self,
    ) -> Option<TerminalSelectedFormMachineOptimizationCustody> {
        self.machine_optimization
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn policy(&self) -> TerminalSelectedFunctionLayoutPolicy {
        self.policy
    }

    pub const fn identity(&self) -> TerminalResolvedSelectedFormLayoutIdentity {
        self.identity
    }

    pub fn functions(&self) -> &[TerminalResolvedSelectedFunctionLayout] {
        &self.functions
    }

    pub fn structural_unit_functions(&self) -> &[TerminalResolvedStructuralUnitFunctionLayout] {
        &self.structural_unit_functions
    }

    /// Rebuild this same resolved-layout representation after a separately
    /// validated, function-relative byte-layout transformation. This helper
    /// recomputes content identity but grants no authority to perform or
    /// validate the transformation itself.
    pub(crate) fn with_replayed_functions(
        &self,
        functions: Vec<TerminalResolvedSelectedFunctionLayout>,
    ) -> Self {
        let identity = layout_identity(
            self.selected,
            self.machine,
            self.pre_layout,
            self.machine_optimization,
            self.target,
            self.policy,
            &functions,
            &self.structural_unit_functions,
        );
        Self {
            selected: self.selected,
            machine: self.machine,
            pre_layout: self.pre_layout,
            machine_optimization: self.machine_optimization,
            target: self.target,
            policy: self.policy,
            identity,
            functions,
            structural_unit_functions: self.structural_unit_functions.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn functions_mut(&mut self) -> &mut [TerminalResolvedSelectedFunctionLayout] {
        &mut self.functions
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn structural_unit_functions_mut(
        &mut self,
    ) -> &mut [TerminalResolvedStructuralUnitFunctionLayout] {
        &mut self.structural_unit_functions
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedResolvedSelectedFormLayoutError {
    PreLayout(OptimizedSelectedFormEncodingError),
    RootMismatch,
    MixedOrdinaryAndStructuralFunctions,
    StructuralFunctionRosterMismatch(MachineId),
    StructuralCallRosterMismatch(TerminalSelectedInstructionId),
    StructuralReturnRosterMismatch(TerminalSelectedInstructionId),
    StructuralEncodingMismatch(TerminalSelectedInstructionId),
    UnsupportedFunctionShape(MachineId),
    DuplicateInstruction(TerminalSelectedInstructionId),
    MissingInstruction(TerminalSelectedInstructionId),
    AlternativeMismatch(TerminalSelectedInstructionId),
    UnexpectedEncodingState(TerminalSelectedInstructionId),
    OffsetOverflow,
    BranchFallthroughMismatch(TerminalSelectedInstructionId),
    BranchEffectsMismatch(TerminalSelectedInstructionId),
    BranchSizeMismatch(TerminalSelectedInstructionId),
    X86_64(X86_64SelectedFormEncodingError),
    Aarch64(Aarch64SelectedFormEncodingError),
    ArtifactMismatch,
}

impl std::fmt::Display for OptimizedResolvedSelectedFormLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized resolved selected-form layout failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedResolvedSelectedFormLayoutError {}

pub fn stage_optimized_resolved_selected_form_layout<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let artifact = compute(selected, machine, physical, pre_layout, None)?;
    validate_optimized_resolved_selected_form_layout(
        selected, machine, physical, pre_layout, &artifact,
    )?;
    Ok(artifact)
}

pub fn validate_optimized_resolved_selected_form_layout<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let replayed = compute(selected, machine, physical, pre_layout, None)?;
    if artifact != &replayed {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    Ok(())
}

/// Resolve the validated symbolic CBNZ disposition after function-relative
/// offsets exist. The compare retains a zero-byte roster row and the branch is
/// independently target-decoded as CBNZ. The result remains separate
/// fragments with no emission, relocation, image, or publication authority.
pub fn stage_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion<
    S: ValidatedTerminalSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    fusion: &StagedOptimizedAarch64CbnzFusion,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let artifact = compute(selected, machine, physical, pre_layout, Some(fusion))?;
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
        selected, machine, physical, pre_layout, fusion, &artifact,
    )?;
    Ok(artifact)
}

/// Independently reconstruct every offset, byte string, target footprint, and
/// symbolic-fusion custody field.
pub fn validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion<
    S: ValidatedTerminalSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let replayed = compute(selected, machine, physical, pre_layout, Some(fusion))?;
    if artifact != &replayed {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    Ok(())
}

fn compute<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    fusion: Option<&StagedOptimizedAarch64CbnzFusion>,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    match fusion {
        None => validate_optimized_layout_independent_selected_form_encoding(
            selected, machine, physical, pre_layout,
        ),
        Some(fusion) => {
            validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion(
                selected, machine, physical, fusion, pre_layout,
            )
        }
    }
    .map_err(OptimizedResolvedSelectedFormLayoutError::PreLayout)?;
    let selected_plan = selected.selected_plan();
    let machine_plan = machine.machine().plan();
    if pre_layout.selected() != selected.selected_identity()
        || pre_layout.machine() != machine.machine().receipt().identity()
        || selected_plan.target != machine_plan.target
        || selected_plan.target.architecture != physical.model().architecture
        || selected_plan.functions.len() != machine_plan.functions.len()
        || selected_plan.structural_unit_functions.len()
            != machine_plan.structural_unit_functions.len()
        || selected_plan.structural_unit_functions.len()
            != pre_layout.structural_unit_functions().len()
        || pre_layout.machine_optimization()
            != fusion
                .map(crate::post_allocation_selected_form_encoding::machine_optimization_custody)
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }

    let has_ordinary = !selected_plan.functions.is_empty();
    let has_structural = !selected_plan.structural_unit_functions.is_empty();
    if has_ordinary && has_structural {
        return Err(OptimizedResolvedSelectedFormLayoutError::MixedOrdinaryAndStructuralFunctions);
    }
    if has_structural && fusion.is_some() {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }
    let policy = if has_structural {
        TerminalSelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1
    } else {
        selected_layout_policy(selected_plan)?
    };
    let mut pre_rows = pre_layout.rows().iter();
    let mut functions = Vec::with_capacity(selected_plan.functions.len());
    for (function, machine_function) in selected_plan.functions.iter().zip(&machine_plan.functions)
    {
        let mut function_pre_rows = BTreeMap::new();
        for block in &function.blocks {
            for instruction in block_instructions(block) {
                let row = pre_rows.next().ok_or(
                    OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
                )?;
                if row.instruction != instruction.id {
                    return Err(
                        OptimizedResolvedSelectedFormLayoutError::MissingInstruction(
                            instruction.id,
                        ),
                    );
                }
                if function_pre_rows.insert(instruction.id, row).is_some() {
                    return Err(
                        OptimizedResolvedSelectedFormLayoutError::DuplicateInstruction(
                            instruction.id,
                        ),
                    );
                }
            }
        }
        let machine_rows = machine_function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .map(|instruction| (instruction.instruction, instruction))
            .collect::<BTreeMap<_, _>>();
        functions.push(layout_function(
            selected_plan.target.architecture,
            function,
            &function_pre_rows,
            &machine_rows,
            physical,
            fusion,
        )?);
    }
    if pre_rows.next().is_some() {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }

    let mut structural_unit_functions =
        Vec::with_capacity(selected_plan.structural_unit_functions.len());
    for ((selected_function, machine_function), pre_function) in selected_plan
        .structural_unit_functions
        .iter()
        .zip(&machine_plan.structural_unit_functions)
        .zip(pre_layout.structural_unit_functions())
    {
        if selected_function.machine != machine_function.machine
            || selected_function.machine != pre_function.machine
            || selected_function.entry_block != machine_function.block
            || selected_function.entry_block != pre_function.block
        {
            return Err(
                OptimizedResolvedSelectedFormLayoutError::StructuralFunctionRosterMismatch(
                    selected_function.machine,
                ),
            );
        }
        match (
            &selected_function.call,
            &machine_function.call,
            &pre_function.call,
        ) {
            (None, None, None) => {}
            (Some(selected_call), Some(machine_call), Some(pre_call))
                if selected_call.id == machine_call.instruction
                    && selected_call.id == pre_call.instruction
                    && selected_call.operation == machine_call.operation
                    && selected_call.operation == pre_call.operation
                    && selected_call.callee == machine_call.callee
                    && selected_call.callee == pre_call.callee => {}
            (Some(selected_call), _, _) => {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::StructuralCallRosterMismatch(
                        selected_call.id,
                    ),
                );
            }
            (None, Some(machine_call), _) => {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::StructuralCallRosterMismatch(
                        machine_call.instruction,
                    ),
                );
            }
            (None, None, Some(pre_call)) => {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::StructuralCallRosterMismatch(
                        pre_call.instruction,
                    ),
                );
            }
        }
        let selected_return = &selected_function.terminator.instruction;
        if selected_return.id != machine_function.return_instruction.instruction
            || selected_return.id != pre_function.return_instruction.instruction
            || machine_function.return_instruction.alternative.key
                != pre_function.return_instruction.alternative
        {
            return Err(
                OptimizedResolvedSelectedFormLayoutError::StructuralReturnRosterMismatch(
                    selected_return.id,
                ),
            );
        }
        structural_unit_functions.push(layout_structural_unit_function(pre_function)?);
    }

    let selected_root = selected.selected_identity();
    let machine_root = machine.machine().receipt().identity();
    let pre_layout_root = pre_layout.identity();
    let target = selected_plan.target;
    let identity = layout_identity(
        selected_root,
        machine_root,
        pre_layout_root,
        pre_layout.machine_optimization(),
        target,
        policy,
        &functions,
        &structural_unit_functions,
    );
    Ok(StagedOptimizedResolvedSelectedFormLayout {
        selected: selected_root,
        machine: machine_root,
        pre_layout: pre_layout_root,
        machine_optimization: pre_layout.machine_optimization(),
        target,
        policy,
        identity,
        functions,
        structural_unit_functions,
    })
}

fn layout_structural_unit_function(
    pre: &TerminalSelectedStructuralUnitFunctionEncoding,
) -> Result<TerminalResolvedStructuralUnitFunctionLayout, OptimizedResolvedSelectedFormLayoutError>
{
    let call = pre
        .call
        .as_ref()
        .map(layout_structural_unit_call)
        .transpose()?;
    let return_offset = match &call {
        None => 0,
        Some(call) => u64::try_from(call.bytes.len())
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?,
    };
    let TerminalSelectedFormEncodingState::Encoded { bytes, .. } = &pre.return_instruction.state
    else {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::StructuralReturnRosterMismatch(
                pre.return_instruction.instruction,
            ),
        );
    };
    if pre.return_instruction.machine_disposition
        != TerminalAarch64CbnzInstructionDisposition::RetainedV1
        || pre.return_instruction.alternative.family
            != omega_terminal_selected_instructions::TerminalMachineAlternativeFamily::ReturnUnit
        || bytes.as_slice() != [0xc3]
    {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::StructuralReturnRosterMismatch(
                pre.return_instruction.instruction,
            ),
        );
    }
    let byte_count = return_offset
        .checked_add(1)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    if byte_count
        != if call.is_some() {
            u64::try_from(X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT + 1)
                .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?
        } else {
            1
        }
    {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::StructuralEncodingMismatch(
                pre.return_instruction.instruction,
            ),
        );
    }
    Ok(TerminalResolvedStructuralUnitFunctionLayout {
        machine: pre.machine,
        block: pre.block,
        offset: 0,
        byte_count,
        call,
        return_instruction: TerminalResolvedSelectedFormRow {
            instruction: pre.return_instruction.instruction,
            alternative: pre.return_instruction.alternative,
            offset: return_offset,
            bytes: bytes.clone(),
            branch: None,
        },
    })
}

fn layout_structural_unit_call(
    pre: &crate::TerminalSelectedStructuralUnitCallEncodingRow,
) -> Result<TerminalResolvedStructuralUnitCallLayout, OptimizedResolvedSelectedFormLayoutError> {
    let fixup = pre.fixup;
    if pre.bytes.len() != X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT
        || pre.bytes.get(usize::from(X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET)) != Some(&0xe8)
        || pre
            .bytes
            .get(
                usize::from(X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET)
                    ..usize::from(X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET)
                        + usize::from(X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH),
            )
            != Some(&[0, 0, 0, 0][..])
        || fixup.kind
            != X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1
        || fixup.state
            != X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1
        || fixup.callee != pre.callee
        || fixup.opcode_byte_offset != X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET
        || fixup.field_byte_offset != X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET
        || fixup.next_instruction_byte_offset
            != X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET
        || fixup.field_byte_width != X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH
        || fixup.addend != 0
    {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::StructuralEncodingMismatch(pre.instruction),
        );
    }
    Ok(TerminalResolvedStructuralUnitCallLayout {
        instruction: pre.instruction,
        operation: pre.operation,
        callee: pre.callee,
        offset: 0,
        bytes: pre.bytes.clone(),
        footprint: pre.footprint.clone(),
        fixup,
    })
}

fn layout_function(
    architecture: Architecture,
    function: &TerminalSelectedFunction,
    pre_rows: &BTreeMap<TerminalSelectedInstructionId, &TerminalSelectedFormEncodingRow>,
    machine_rows: &BTreeMap<
        TerminalSelectedInstructionId,
        &TerminalPostAllocationMachineInstruction,
    >,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: Option<&StagedOptimizedAarch64CbnzFusion>,
) -> Result<TerminalResolvedSelectedFunctionLayout, OptimizedResolvedSelectedFormLayoutError> {
    if function.blocks.len() == 1 {
        return layout_single_block(
            architecture,
            function,
            pre_rows,
            machine_rows,
            physical,
            fusion,
        );
    }
    if function.blocks.len() != 3 {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine),
        );
    }
    let entry = find_block(function, function.entry_block)?;
    let TerminalSelectedTerminator::ConditionalBranch {
        when_nonzero,
        when_zero,
        ..
    } = &entry.terminator
    else {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine),
        );
    };
    if when_nonzero.block == when_zero.block
        || entry.id == when_nonzero.block
        || entry.id == when_zero.block
    {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine),
        );
    }
    let zero = find_block(function, when_zero.block)?;
    let nonzero = find_block(function, when_nonzero.block)?;
    if !matches!(zero.terminator, TerminalSelectedTerminator::Return { .. })
        || !matches!(
            nonzero.terminator,
            TerminalSelectedTerminator::Return { .. }
        )
    {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine),
        );
    }
    let ordered = [entry, zero, nonzero];
    let mut block_offsets = BTreeMap::new();
    let mut block_sizes = BTreeMap::new();
    let mut offset = 0_u64;
    for block in ordered {
        block_offsets.insert(block.id, offset);
        let start = offset;
        for instruction in block_instructions(block) {
            let pre = pre_rows.get(&instruction.id).ok_or(
                OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
            )?;
            offset = offset
                .checked_add(planned_size(architecture, instruction, pre)?)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
        }
        block_sizes.insert(block.id, offset - start);
    }
    let function_size = offset;

    let mut blocks = Vec::with_capacity(3);
    for block in ordered {
        let block_offset = block_offsets[&block.id];
        let mut instruction_offset = block_offset;
        let mut instructions = Vec::new();
        for instruction in block_instructions(block) {
            let pre = pre_rows[&instruction.id];
            let machine = machine_rows.get(&instruction.id).ok_or(
                OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
            )?;
            if machine.alternative.key != pre.alternative {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::AlternativeMismatch(instruction.id),
                );
            }
            let (bytes, branch) = resolve_instruction(
                architecture,
                function.machine,
                block,
                instruction,
                instruction_offset,
                &block_offsets,
                machine,
                pre,
                physical,
                fusion,
            )?;
            let byte_count = u64::try_from(bytes.len())
                .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
            instructions.push(TerminalResolvedSelectedFormRow {
                instruction: instruction.id,
                alternative: pre.alternative,
                offset: instruction_offset,
                bytes,
                branch,
            });
            instruction_offset = instruction_offset
                .checked_add(byte_count)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
        }
        let byte_count = block_sizes[&block.id];
        if instruction_offset != block_offset + byte_count {
            return Err(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow);
        }
        blocks.push(TerminalResolvedSelectedBlockLayout {
            block: block.id,
            offset: block_offset,
            byte_count,
            instructions,
        });
    }
    Ok(TerminalResolvedSelectedFunctionLayout {
        machine: function.machine,
        byte_count: function_size,
        blocks,
    })
}

fn selected_layout_policy(
    selected: &omega_terminal_selected_instructions::TerminalSelectedInstructionPlan,
) -> Result<TerminalSelectedFunctionLayoutPolicy, OptimizedResolvedSelectedFormLayoutError> {
    let is_single_entry = |function: &TerminalSelectedFunction| {
        let [block] = function.blocks.as_slice() else {
            return false;
        };
        function.entry_block == block.id
            && matches!(block.terminator, TerminalSelectedTerminator::Return { .. })
    };
    let single_entry_count = selected
        .functions
        .iter()
        .filter(|function| is_single_entry(function))
        .count();
    if single_entry_count == selected.functions.len() {
        Ok(TerminalSelectedFunctionLayoutPolicy::SingleEntryBlockV1)
    } else if single_entry_count == 0 {
        Ok(TerminalSelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1)
    } else {
        Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(
                selected.functions[single_entry_count].machine,
            ),
        )
    }
}

fn layout_single_block(
    architecture: Architecture,
    function: &TerminalSelectedFunction,
    pre_rows: &BTreeMap<TerminalSelectedInstructionId, &TerminalSelectedFormEncodingRow>,
    machine_rows: &BTreeMap<
        TerminalSelectedInstructionId,
        &TerminalPostAllocationMachineInstruction,
    >,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: Option<&StagedOptimizedAarch64CbnzFusion>,
) -> Result<TerminalResolvedSelectedFunctionLayout, OptimizedResolvedSelectedFormLayoutError> {
    let [block] = function.blocks.as_slice() else {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine),
        );
    };
    if function.entry_block != block.id
        || !matches!(block.terminator, TerminalSelectedTerminator::Return { .. })
        || fusion.is_some()
    {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine),
        );
    }
    let mut offset = 0_u64;
    let mut instructions = Vec::new();
    let block_offsets = BTreeMap::from([(block.id, 0)]);
    for instruction in block_instructions(block) {
        let pre = pre_rows
            .get(&instruction.id)
            .ok_or(OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id))?;
        let machine = machine_rows
            .get(&instruction.id)
            .ok_or(OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id))?;
        if machine.alternative.key != pre.alternative {
            return Err(
                OptimizedResolvedSelectedFormLayoutError::AlternativeMismatch(instruction.id),
            );
        }
        let (bytes, branch) = resolve_instruction(
            architecture,
            function.machine,
            block,
            instruction,
            offset,
            &block_offsets,
            machine,
            pre,
            physical,
            None,
        )?;
        if branch.is_some() {
            return Err(
                OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(
                    function.machine,
                ),
            );
        }
        let byte_count = u64::try_from(bytes.len())
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
        instructions.push(TerminalResolvedSelectedFormRow {
            instruction: instruction.id,
            alternative: pre.alternative,
            offset,
            bytes,
            branch: None,
        });
        offset = offset
            .checked_add(byte_count)
            .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    }
    Ok(TerminalResolvedSelectedFunctionLayout {
        machine: function.machine,
        byte_count: offset,
        blocks: vec![TerminalResolvedSelectedBlockLayout {
            block: block.id,
            offset: 0,
            byte_count: offset,
            instructions,
        }],
    })
}

#[allow(clippy::too_many_arguments)]
fn resolve_instruction(
    architecture: Architecture,
    function: MachineId,
    block: &TerminalSelectedBlock,
    instruction: &TerminalSelectedInstruction,
    instruction_offset: u64,
    block_offsets: &BTreeMap<TerminalSelectedBlockId, u64>,
    machine: &TerminalPostAllocationMachineInstruction,
    pre: &TerminalSelectedFormEncodingRow,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: Option<&StagedOptimizedAarch64CbnzFusion>,
) -> Result<
    (
        Vec<u8>,
        Option<Box<TerminalResolvedConditionalBranchEvidence>>,
    ),
    OptimizedResolvedSelectedFormLayoutError,
> {
    match (&pre.machine_disposition, &pre.state) {
        (
            TerminalAarch64CbnzInstructionDisposition::RetainedV1,
            TerminalSelectedFormEncodingState::Encoded { bytes, .. },
        ) => Ok((bytes.clone(), None)),
        (
            TerminalAarch64CbnzInstructionDisposition::RetainedV1,
            TerminalSelectedFormEncodingState::DeferredControl {
                reason: DeferredTerminalControlEncodingReason::RequiresResolvedBranchLayout,
            },
        ) => resolve_branch(
            architecture,
            block,
            instruction,
            instruction_offset,
            block_offsets,
            machine,
            physical,
            None,
        ),
        (
            TerminalAarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { consumer },
            TerminalSelectedFormEncodingState::Encoded { .. },
        ) => {
            let action = fusion_action(fusion, function, block.id, instruction.id, *consumer)?;
            if architecture != Architecture::Aarch64
                || !matches!(
                    instruction.kind,
                    omega_terminal_selected_instructions::TerminalSelectedInstructionKind::CompareI64Zero
                )
                || action.compare != instruction.id
                || action.branch != *consumer
            {
                return Err(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(
                    instruction.id,
                ));
            }
            Ok((Vec::new(), None))
        }
        (
            TerminalAarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
                compare,
                source_read,
            },
            TerminalSelectedFormEncodingState::DeferredControl {
                reason: DeferredTerminalControlEncodingReason::RequiresResolvedBranchLayout,
            },
        ) => {
            let action = fusion_action(fusion, function, block.id, *compare, instruction.id)?;
            if architecture != Architecture::Aarch64 || &action.source_read != source_read {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(
                        instruction.id,
                    ),
                );
            }
            resolve_branch(
                architecture,
                block,
                instruction,
                instruction_offset,
                block_offsets,
                machine,
                physical,
                Some((source_read, action)),
            )
        }
        _ => Err(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction.id)),
    }
}

fn resolve_branch(
    architecture: Architecture,
    block: &TerminalSelectedBlock,
    instruction: &TerminalSelectedInstruction,
    instruction_offset: u64,
    block_offsets: &BTreeMap<TerminalSelectedBlockId, u64>,
    machine: &TerminalPostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    fused: Option<(
        &TerminalQualifiedPhysicalRead,
        &TerminalAarch64CbnzFusionAction,
    )>,
) -> Result<
    (
        Vec<u8>,
        Option<Box<TerminalResolvedConditionalBranchEvidence>>,
    ),
    OptimizedResolvedSelectedFormLayoutError,
> {
    let TerminalSelectedTerminator::ConditionalBranch {
        instruction: terminator,
        when_nonzero,
        when_zero,
    } = &block.terminator
    else {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction.id),
        );
    };
    if terminator.id != instruction.id {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction.id),
        );
    }
    let nonzero_offset = *block_offsets.get(&when_nonzero.block).ok_or(
        OptimizedResolvedSelectedFormLayoutError::BranchFallthroughMismatch(instruction.id),
    )?;
    let zero_offset = *block_offsets.get(&when_zero.block).ok_or(
        OptimizedResolvedSelectedFormLayoutError::BranchFallthroughMismatch(instruction.id),
    )?;
    let branch_size = branch_size(architecture);
    let branch_end = instruction_offset
        .checked_add(branch_size)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    if zero_offset != branch_end {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::BranchFallthroughMismatch(instruction.id),
        );
    }
    let displacement = match architecture {
        Architecture::X86_64 => checked_delta(nonzero_offset, branch_end)?,
        Architecture::Aarch64 => checked_delta(nonzero_offset, instruction_offset)?,
    };
    let (bytes, register_reads, effects) = match (architecture, fused) {
        (Architecture::Aarch64, Some((source_read, _))) => {
            let encoded =
                encode_aarch64_terminal_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                    physical,
                    source_read.view,
                    displacement,
                )
                .map_err(OptimizedResolvedSelectedFormLayoutError::Aarch64)?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
        (Architecture::X86_64, Some(_)) => {
            return Err(
                OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction.id),
            );
        }
        (Architecture::X86_64, None) => {
            let encoded = encode_x86_64_terminal_selected_nonzero_branch_form(
                physical,
                machine.alternative.key,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::X86_64)?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
        (Architecture::Aarch64, None) => {
            let encoded = encode_aarch64_terminal_selected_nonzero_branch_form(
                physical,
                machine.alternative.key,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::Aarch64)?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
    };
    if let Some((source_read, action)) = fused {
        validate_fused_branch_footprint(
            instruction.id,
            block,
            source_read,
            action,
            physical,
            &register_reads,
            &effects,
            &machine.alternative.encoded,
        )?;
    } else if effects != machine.alternative.encoded {
        return Err(
            OptimizedResolvedSelectedFormLayoutError::BranchEffectsMismatch(instruction.id),
        );
    }
    if u64::try_from(bytes.len()).ok() != Some(branch_size) {
        return Err(OptimizedResolvedSelectedFormLayoutError::BranchSizeMismatch(instruction.id));
    }
    let declared_size_matches = match machine.alternative.size {
        TerminalMachineSizeKnowledge::ExactBytes(expected) => u64::from(expected) == branch_size,
        TerminalMachineSizeKnowledge::EncoderResolved {
            minimum_bytes,
            maximum_bytes,
        } => {
            branch_size >= u64::from(minimum_bytes)
                && maximum_bytes.is_none_or(|maximum| branch_size <= u64::from(maximum))
        }
    };
    if !declared_size_matches {
        return Err(OptimizedResolvedSelectedFormLayoutError::BranchSizeMismatch(instruction.id));
    }
    Ok((
        bytes,
        Some(Box::new(TerminalResolvedConditionalBranchEvidence {
            source_block: block.id,
            when_nonzero_edge: when_nonzero.psi_edge,
            when_nonzero_block: when_nonzero.block,
            when_nonzero_offset: nonzero_offset,
            when_zero_edge: when_zero.psi_edge,
            when_zero_block: when_zero.block,
            when_zero_offset: zero_offset,
            byte_displacement: displacement,
            decoded_register_reads: register_reads,
            decoded_effects: effects,
        })),
    ))
}

fn find_block(
    function: &TerminalSelectedFunction,
    id: TerminalSelectedBlockId,
) -> Result<&TerminalSelectedBlock, OptimizedResolvedSelectedFormLayoutError> {
    function
        .blocks
        .iter()
        .find(|block| block.id == id)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine))
}

fn block_instructions(block: &TerminalSelectedBlock) -> Vec<&TerminalSelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
            | TerminalSelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .collect()
}

fn planned_size(
    architecture: Architecture,
    instruction: &TerminalSelectedInstruction,
    row: &TerminalSelectedFormEncodingRow,
) -> Result<u64, OptimizedResolvedSelectedFormLayoutError> {
    match &row.machine_disposition {
        TerminalAarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { .. } => {
            if architecture == Architecture::Aarch64
                && matches!(
                    instruction.kind,
                    omega_terminal_selected_instructions::TerminalSelectedInstructionKind::CompareI64Zero
                )
                && matches!(row.state, TerminalSelectedFormEncodingState::Encoded { .. })
            {
                return Ok(0);
            }
            return Err(
                OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction.id),
            );
        }
        TerminalAarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 { .. } => {
            if architecture == Architecture::Aarch64
                && matches!(
                    row.state,
                    TerminalSelectedFormEncodingState::DeferredControl { .. }
                )
            {
                return Ok(branch_size(architecture));
            }
            return Err(
                OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction.id),
            );
        }
        TerminalAarch64CbnzInstructionDisposition::RetainedV1 => {}
    }
    match &row.state {
        TerminalSelectedFormEncodingState::Encoded { bytes, .. } => u64::try_from(bytes.len())
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow),
        TerminalSelectedFormEncodingState::DeferredControl {
            reason: DeferredTerminalControlEncodingReason::RequiresResolvedBranchLayout,
        } => {
            if row.instruction != instruction.id {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
                );
            }
            Ok(branch_size(architecture))
        }
    }
}

const fn branch_size(architecture: Architecture) -> u64 {
    match architecture {
        Architecture::X86_64 => 6,
        Architecture::Aarch64 => 4,
    }
}

fn checked_delta(target: u64, base: u64) -> Result<i64, OptimizedResolvedSelectedFormLayoutError> {
    i64::try_from(i128::from(target) - i128::from(base))
        .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)
}

fn fusion_action<'fusion>(
    fusion: Option<&'fusion StagedOptimizedAarch64CbnzFusion>,
    machine: MachineId,
    block: TerminalSelectedBlockId,
    compare: TerminalSelectedInstructionId,
    branch: TerminalSelectedInstructionId,
) -> Result<&'fusion TerminalAarch64CbnzFusionAction, OptimizedResolvedSelectedFormLayoutError> {
    fusion
        .and_then(|fusion| {
            fusion.fusion().plan().actions.iter().find(|action| {
                action.machine == machine
                    && action.block == block
                    && action.compare == compare
                    && action.branch == branch
            })
        })
        .ok_or(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(branch))
}

#[allow(clippy::too_many_arguments)]
fn validate_fused_branch_footprint(
    instruction: TerminalSelectedInstructionId,
    block: &TerminalSelectedBlock,
    source_read: &TerminalQualifiedPhysicalRead,
    action: &TerminalAarch64CbnzFusionAction,
    physical: &ValidatedPhysicalRegisterModel,
    register_reads: &[omega_register_model::RegisterViewId],
    effects: &TerminalMachineEncodedEffects,
    original: &TerminalMachineEncodedEffects,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == source_read.view)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::BranchEffectsMismatch(instruction))?;
    let TerminalSelectedTerminator::ConditionalBranch {
        when_nonzero,
        when_zero,
        ..
    } = &block.terminator
    else {
        return Err(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction));
    };
    if register_reads != [source_read.view]
        || source_read.units != view.units
        || &action.source_read != source_read
        || action.when_nonzero_edge != when_nonzero.psi_edge
        || action.when_nonzero_block != when_nonzero.block
        || action.when_zero_edge != when_zero.psi_edge
        || action.when_zero_block != when_zero.block
        || effects.external_operand_reads != []
        || effects.external_operand_writes != []
        || effects.implicit_unit_uses != action.pc_units
        || effects.implicit_unit_defs != action.pc_units
        || !effects.implicit_unit_clobbers.is_empty()
        || effects
            .implicit_unit_uses
            .iter()
            .any(|unit| action.nzcv_units.contains(unit))
        || effects.memory != original.memory
        || effects.stack != original.stack
        || effects.trap != original.trap
        || effects.control != original.control
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::BranchEffectsMismatch(instruction));
    }
    Ok(())
}

fn layout_identity(
    selected: omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity,
    machine: omega_machine_optimizer::TerminalPostAllocationMachineIdentity,
    pre_layout: crate::TerminalSelectedFormEncodingIdentity,
    machine_optimization: Option<TerminalSelectedFormMachineOptimizationCustody>,
    target: NativeTarget,
    policy: TerminalSelectedFunctionLayoutPolicy,
    functions: &[TerminalResolvedSelectedFunctionLayout],
    structural_unit_functions: &[TerminalResolvedStructuralUnitFunctionLayout],
) -> TerminalResolvedSelectedFormLayoutIdentity {
    let mut hasher = Sha256::new();
    hasher.update(LAYOUT_SCHEMA);
    hasher.update(selected.bytes());
    hasher.update(machine.bytes());
    hasher.update(pre_layout.bytes());
    match machine_optimization {
        None => hasher.update([0]),
        Some(custody) => {
            hasher.update([1]);
            hasher.update(custody.selections().bytes());
            hasher.update(custody.post_allocation_machine_selections().bytes());
            hasher.update(custody.fusion().bytes());
        }
    }
    hasher.update([match target.architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => 1,
    }]);
    hasher.update([match target.object_format {
        ObjectFormat::Elf => 0,
        ObjectFormat::MachO => 1,
        ObjectFormat::Coff => 2,
    }]);
    hasher.update((target.pointer_size as u64).to_le_bytes());
    hasher.update((target.pointer_alignment as u64).to_le_bytes());
    hasher.update([match policy {
        TerminalSelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1 => 0,
        TerminalSelectedFunctionLayoutPolicy::SingleEntryBlockV1 => 1,
        TerminalSelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1 => 2,
    }]);
    hasher.update((functions.len() as u64).to_le_bytes());
    for function in functions {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.byte_count.to_le_bytes());
        hasher.update((function.blocks.len() as u64).to_le_bytes());
        for block in &function.blocks {
            hasher.update(block.block.0.to_le_bytes());
            hasher.update(block.offset.to_le_bytes());
            hasher.update(block.byte_count.to_le_bytes());
            hasher.update((block.instructions.len() as u64).to_le_bytes());
            for instruction in &block.instructions {
                hasher.update(instruction.instruction.0.to_le_bytes());
                encode_alternative(&mut hasher, instruction.alternative);
                hasher.update(instruction.offset.to_le_bytes());
                hasher.update((instruction.bytes.len() as u64).to_le_bytes());
                hasher.update(&instruction.bytes);
                match &instruction.branch {
                    None => hasher.update([0]),
                    Some(branch) => {
                        hasher.update([1]);
                        hasher.update(branch.source_block.0.to_le_bytes());
                        hasher.update(branch.when_nonzero_edge.get().to_le_bytes());
                        hasher.update(branch.when_nonzero_block.0.to_le_bytes());
                        hasher.update(branch.when_nonzero_offset.to_le_bytes());
                        hasher.update(branch.when_zero_edge.get().to_le_bytes());
                        hasher.update(branch.when_zero_block.0.to_le_bytes());
                        hasher.update(branch.when_zero_offset.to_le_bytes());
                        hasher.update(branch.byte_displacement.to_le_bytes());
                        encode_views(&mut hasher, &branch.decoded_register_reads);
                        encode_effects(&mut hasher, &branch.decoded_effects);
                    }
                }
            }
        }
    }
    hasher.update((structural_unit_functions.len() as u64).to_le_bytes());
    for function in structural_unit_functions {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.block.0.to_le_bytes());
        hasher.update(function.offset.to_le_bytes());
        hasher.update(function.byte_count.to_le_bytes());
        match &function.call {
            None => hasher.update([0]),
            Some(call) => {
                hasher.update([1]);
                hasher.update(call.instruction.0.to_le_bytes());
                hasher.update(call.operation.get().to_le_bytes());
                hasher.update(call.callee.get().to_le_bytes());
                hasher.update(call.offset.to_le_bytes());
                hasher.update((call.bytes.len() as u64).to_le_bytes());
                hasher.update(&call.bytes);
                encode_structural_footprint(&mut hasher, &call.footprint);
                encode_structural_fixup(&mut hasher, call.fixup);
            }
        }
        hasher.update(function.return_instruction.instruction.0.to_le_bytes());
        encode_alternative(&mut hasher, function.return_instruction.alternative);
        hasher.update(function.return_instruction.offset.to_le_bytes());
        hasher.update((function.return_instruction.bytes.len() as u64).to_le_bytes());
        hasher.update(&function.return_instruction.bytes);
        debug_assert!(function.return_instruction.branch.is_none());
        hasher.update([0]);
    }
    TerminalResolvedSelectedFormLayoutIdentity(hasher.finalize().into())
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
        omega_terminal_selected_instructions::TerminalMachineTrapBehavior::NeverV1 => 0,
        omega_terminal_selected_instructions::TerminalMachineTrapBehavior::MayArchitecturalFaultV1 => 1,
    }]);
    hasher.update([match footprint.barrier {
        omega_terminal_selected_instructions::TerminalStructuralUnitCallBarrier::CallV1 => 0,
    }]);
    hasher.update([match footprint.call {
        omega_terminal_selected_instructions::TerminalStructuralUnitCallEffect::DirectInternalUnitV1 => 0,
    }]);
    hasher.update([match footprint.cleanup {
        omega_terminal_selected_instructions::TerminalMachineCleanupEffect::NoneV1 => 0,
    }]);
}

fn encode_structural_fixup(hasher: &mut Sha256, fixup: X86_64StructuralUnitInternalControlFixup) {
    hasher.update([match fixup.kind {
        X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1 => 0,
    }]);
    hasher.update([match fixup.state {
        X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1 => 0,
    }]);
    hasher.update(fixup.callee.get().to_le_bytes());
    hasher.update(fixup.opcode_byte_offset.to_le_bytes());
    hasher.update(fixup.field_byte_offset.to_le_bytes());
    hasher.update(fixup.next_instruction_byte_offset.to_le_bytes());
    hasher.update([fixup.field_byte_width]);
    hasher.update(fixup.addend.to_le_bytes());
}

fn encode_views(hasher: &mut Sha256, values: &[omega_register_model::RegisterViewId]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.0.to_le_bytes());
    }
}

fn encode_alternative(hasher: &mut Sha256, alternative: TerminalMachineAlternativeKey) {
    use omega_terminal_selected_instructions::TerminalMachineAlternativeFamily as Family;
    hasher.update([match alternative.family {
        Family::CompareI64Zero => 0,
        Family::MaterializeI64 => 1,
        Family::CopyI64 => 2,
        Family::ExactAddI64 => 3,
        Family::ExactAddI64Immediate => 4,
        Family::ExactSubtractI64 => 5,
        Family::ConditionalBranchNonZero => 6,
        Family::ReturnI64 => 7,
        Family::ExactSubtractI64Immediate => 8,
        Family::ReturnUnit => 9,
    }]);
    hasher.update(alternative.variant.to_le_bytes());
}

fn encode_effects(hasher: &mut Sha256, effects: &TerminalMachineEncodedEffects) {
    encode_u16s(hasher, &effects.external_operand_reads);
    encode_u16s(hasher, &effects.external_operand_writes);
    encode_units(hasher, &effects.implicit_unit_uses);
    encode_units(hasher, &effects.implicit_unit_defs);
    encode_units(hasher, &effects.implicit_unit_clobbers);
    match effects.memory {
        TerminalMachineEncodedMemoryEffect::NoneV1 => hasher.update([0]),
        TerminalMachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
    }
    match effects.stack {
        TerminalMachineEncodedStackEffect::UnchangedV1 => hasher.update([0]),
        TerminalMachineEncodedStackEffect::PopBytesV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
    }
    hasher.update([match effects.trap {
        TerminalMachineEncodedTrapBehavior::NeverV1 => 0,
        TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1 => 1,
    }]);
    match effects.control {
        TerminalMachineEncodedControlEffect::FallThroughV1 => hasher.update([0]),
        TerminalMachineEncodedControlEffect::ConditionalRelativeBranchV1 => hasher.update([1]),
        TerminalMachineEncodedControlEffect::ReturnFromActivationStackV1 => hasher.update([2]),
        TerminalMachineEncodedControlEffect::ReturnIndirectRegisterV1 { target } => {
            hasher.update([3]);
            hasher.update(target.0.to_le_bytes());
        }
    }
}

fn encode_u16s(hasher: &mut Sha256, values: &[u16]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.to_le_bytes());
    }
}

fn encode_units(hasher: &mut Sha256, values: &[omega_register_model::RegisterUnitId]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for value in values {
        hasher.update(value.0.to_le_bytes());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        TerminalSelectedFormDecodedFootprint, TerminalSelectedStructuralUnitCallEncodingRow,
    };
    use omega_terminal_isa_x86_64::{
        X86_64StructuralUnitArgumentPointerWrite, X86_64StructuralUnitCallerCopyWrite,
        X86_64StructuralUnitRootRead,
    };
    use omega_terminal_selected_instructions::{
        TerminalMachineAlternativeFamily, TerminalMachineCleanupEffect,
        TerminalMachineTrapBehavior, TerminalStructuralUnitCallBarrier,
        TerminalStructuralUnitCallEffect,
    };

    fn structural_function(with_call: bool) -> TerminalSelectedStructuralUnitFunctionEncoding {
        let machine = MachineId::new(71).unwrap();
        let callee = MachineId::new(72).unwrap();
        let mut bytes = vec![0; X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT];
        bytes[usize::from(X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET)] = 0xe8;
        let call = with_call.then(|| TerminalSelectedStructuralUnitCallEncodingRow {
            instruction: TerminalSelectedInstructionId(0),
            operation: OperationId::new(81).unwrap(),
            callee,
            bytes,
            footprint: Box::new(X86_64SelectedStructuralUnitCallFootprint {
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: Vec::new(),
                root_reads: [
                    X86_64StructuralUnitRootRead {
                        root: MachineRegister::X86Rcx,
                        byte_offset: 0,
                        byte_count: 8,
                    },
                    X86_64StructuralUnitRootRead {
                        root: MachineRegister::X86Rcx,
                        byte_offset: 8,
                        byte_count: 8,
                    },
                    X86_64StructuralUnitRootRead {
                        root: MachineRegister::X86Rdx,
                        byte_offset: 0,
                        byte_count: 8,
                    },
                    X86_64StructuralUnitRootRead {
                        root: MachineRegister::X86Rdx,
                        byte_offset: 8,
                        byte_count: 8,
                    },
                ],
                caller_copy_writes: [
                    X86_64StructuralUnitCallerCopyWrite {
                        stack_byte_offset: 32,
                        byte_count: 8,
                    },
                    X86_64StructuralUnitCallerCopyWrite {
                        stack_byte_offset: 40,
                        byte_count: 8,
                    },
                    X86_64StructuralUnitCallerCopyWrite {
                        stack_byte_offset: 48,
                        byte_count: 8,
                    },
                    X86_64StructuralUnitCallerCopyWrite {
                        stack_byte_offset: 56,
                        byte_count: 8,
                    },
                ],
                scratch_register_writes: [MachineRegister::X86Rax],
                argument_pointer_writes: [
                    X86_64StructuralUnitArgumentPointerWrite {
                        register: MachineRegister::X86Rcx,
                        stack_byte_offset: 32,
                    },
                    X86_64StructuralUnitArgumentPointerWrite {
                        register: MachineRegister::X86Rdx,
                        stack_byte_offset: 48,
                    },
                ],
                writes_rflags: true,
                frame_byte_count: 72,
                shadow_byte_count: 32,
                pre_call_stack_alignment: 16,
                frame_is_balanced: true,
                trap: TerminalMachineTrapBehavior::MayArchitecturalFaultV1,
                barrier: TerminalStructuralUnitCallBarrier::CallV1,
                call: TerminalStructuralUnitCallEffect::DirectInternalUnitV1,
                cleanup: TerminalMachineCleanupEffect::NoneV1,
            }),
            fixup: X86_64StructuralUnitInternalControlFixup {
                kind: X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1,
                state: X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1,
                callee,
                opcode_byte_offset: X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET,
                field_byte_offset: X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET,
                next_instruction_byte_offset: X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET,
                field_byte_width: X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH,
                addend: 0,
            },
        });
        TerminalSelectedStructuralUnitFunctionEncoding {
            machine,
            block: TerminalSelectedBlockId(0),
            call,
            return_instruction: TerminalSelectedFormEncodingRow {
                instruction: TerminalSelectedInstructionId(u32::from(with_call)),
                alternative: TerminalMachineAlternativeKey {
                    family: TerminalMachineAlternativeFamily::ReturnUnit,
                    variant: 0,
                },
                machine_disposition: TerminalAarch64CbnzInstructionDisposition::RetainedV1,
                state: TerminalSelectedFormEncodingState::Encoded {
                    bytes: vec![0xc3],
                    footprint: Box::new(TerminalSelectedFormDecodedFootprint {
                        register_reads: Vec::new(),
                        register_writes: Vec::new(),
                        implicit_defs: Vec::new(),
                        implicit_clobbers: Vec::new(),
                        encoded: TerminalMachineEncodedEffects {
                            external_operand_reads: Vec::new(),
                            external_operand_writes: Vec::new(),
                            implicit_unit_uses: Vec::new(),
                            implicit_unit_defs: Vec::new(),
                            implicit_unit_clobbers: Vec::new(),
                            memory: TerminalMachineEncodedMemoryEffect::NoneV1,
                            stack: TerminalMachineEncodedStackEffect::UnchangedV1,
                            trap: TerminalMachineEncodedTrapBehavior::NeverV1,
                            control:
                                TerminalMachineEncodedControlEffect::ReturnFromActivationStackV1,
                        },
                    }),
                },
            },
        }
    }

    #[test]
    fn structural_layout_retains_unresolved_call_and_exact_function_spans() {
        let caller = layout_structural_unit_function(&structural_function(true)).unwrap();
        assert_eq!(caller.offset, 0);
        assert_eq!(caller.byte_count, 90);
        assert_eq!(caller.return_instruction.offset, 89);
        assert_eq!(caller.return_instruction.bytes, [0xc3]);
        let call = caller.call.unwrap();
        assert_eq!(call.offset, 0);
        assert_eq!(call.bytes.len(), 89);
        assert_eq!(&call.bytes[81..85], [0, 0, 0, 0]);
        assert_eq!(
            call.fixup.state,
            X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1
        );

        let leaf = layout_structural_unit_function(&structural_function(false)).unwrap();
        assert_eq!(leaf.offset, 0);
        assert_eq!(leaf.byte_count, 1);
        assert!(leaf.call.is_none());
        assert_eq!(leaf.return_instruction.offset, 0);
        assert_eq!(leaf.return_instruction.bytes, [0xc3]);
    }

    #[test]
    fn structural_layout_rejects_template_fixup_and_return_corruption() {
        let mut corrupted = structural_function(true);
        corrupted.call.as_mut().unwrap().bytes[81] = 1;
        assert!(matches!(
            layout_structural_unit_function(&corrupted),
            Err(
                OptimizedResolvedSelectedFormLayoutError::StructuralEncodingMismatch(
                    TerminalSelectedInstructionId(0)
                )
            )
        ));

        let mut corrupted = structural_function(true);
        corrupted
            .call
            .as_mut()
            .unwrap()
            .fixup
            .next_instruction_byte_offset = 84;
        assert!(matches!(
            layout_structural_unit_function(&corrupted),
            Err(
                OptimizedResolvedSelectedFormLayoutError::StructuralEncodingMismatch(
                    TerminalSelectedInstructionId(0)
                )
            )
        ));

        let mut corrupted = structural_function(false);
        let TerminalSelectedFormEncodingState::Encoded { bytes, .. } =
            &mut corrupted.return_instruction.state
        else {
            unreachable!()
        };
        bytes[0] = 0x90;
        assert!(matches!(
            layout_structural_unit_function(&corrupted),
            Err(
                OptimizedResolvedSelectedFormLayoutError::StructuralReturnRosterMismatch(
                    TerminalSelectedInstructionId(0)
                )
            )
        ));
    }
}
