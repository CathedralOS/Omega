use std::collections::BTreeMap;

use omega_machine_optimizer::TerminalPostAllocationMachineInstruction;
use omega_regalloc::ValidatedTerminalSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_isa_aarch64::{
    Aarch64SelectedFormEncodingError, encode_aarch64_terminal_selected_nonzero_branch_form,
};
use omega_terminal_isa_x86_64::{
    X86_64SelectedFormEncodingError, encode_x86_64_terminal_selected_nonzero_branch_form,
};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternativeKey, TerminalMachineEncodedControlEffect,
    TerminalMachineEncodedEffects, TerminalMachineEncodedMemoryEffect,
    TerminalMachineEncodedStackEffect, TerminalMachineEncodedTrapBehavior,
    TerminalMachineSizeKnowledge, TerminalSelectedBlock, TerminalSelectedBlockId,
    TerminalSelectedFunction, TerminalSelectedInstruction, TerminalSelectedInstructionId,
    TerminalSelectedTerminator,
};
use psi_core::{EdgeId, MachineId};
use sha2::{Digest, Sha256};

use crate::{
    DeferredTerminalControlEncodingReason, OptimizedSelectedFormEncodingError,
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedSelectedFormEncoding,
    TerminalSelectedFormEncodingRow, TerminalSelectedFormEncodingState,
    validate_optimized_layout_independent_selected_form_encoding,
};

const LAYOUT_SCHEMA: &[u8] = b"omega.terminal.resolved-selected-form-layout.v1";

/// Required-stage baseline layout for the currently admitted three-block
/// conditional. This is a visible policy identity, not an optimization level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalSelectedFunctionLayoutPolicy {
    EntryThenZeroFallthroughThenNonzeroV1,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedResolvedSelectedFormLayout {
    selected: omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity,
    machine: omega_machine_optimizer::TerminalPostAllocationMachineIdentity,
    pre_layout: crate::TerminalSelectedFormEncodingIdentity,
    target: NativeTarget,
    policy: TerminalSelectedFunctionLayoutPolicy,
    identity: TerminalResolvedSelectedFormLayoutIdentity,
    functions: Vec<TerminalResolvedSelectedFunctionLayout>,
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
            self.target,
            self.policy,
            &functions,
        );
        Self {
            selected: self.selected,
            machine: self.machine,
            pre_layout: self.pre_layout,
            target: self.target,
            policy: self.policy,
            identity,
            functions,
        }
    }

    #[cfg(test)]
    pub(crate) fn functions_mut(&mut self) -> &mut [TerminalResolvedSelectedFunctionLayout] {
        &mut self.functions
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedResolvedSelectedFormLayoutError {
    PreLayout(OptimizedSelectedFormEncodingError),
    RootMismatch,
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
    let artifact = compute(selected, machine, physical, pre_layout)?;
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
    let replayed = compute(selected, machine, physical, pre_layout)?;
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
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    validate_optimized_layout_independent_selected_form_encoding(
        selected, machine, physical, pre_layout,
    )
    .map_err(OptimizedResolvedSelectedFormLayoutError::PreLayout)?;
    let selected_plan = selected.selected_plan();
    let machine_plan = machine.machine().plan();
    if pre_layout.selected() != selected.selected_identity()
        || pre_layout.machine() != machine.machine().receipt().identity()
        || selected_plan.target != machine_plan.target
        || selected_plan.target.architecture != physical.model().architecture
        || selected_plan.functions.len() != machine_plan.functions.len()
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }

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
        )?);
    }
    if pre_rows.next().is_some() {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }

    let selected_root = selected.selected_identity();
    let machine_root = machine.machine().receipt().identity();
    let pre_layout_root = pre_layout.identity();
    let target = selected_plan.target;
    let policy = TerminalSelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1;
    let identity = layout_identity(
        selected_root,
        machine_root,
        pre_layout_root,
        target,
        policy,
        &functions,
    );
    Ok(StagedOptimizedResolvedSelectedFormLayout {
        selected: selected_root,
        machine: machine_root,
        pre_layout: pre_layout_root,
        target,
        policy,
        identity,
        functions,
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
) -> Result<TerminalResolvedSelectedFunctionLayout, OptimizedResolvedSelectedFormLayoutError> {
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
                .checked_add(planned_size(architecture, instruction.id, pre)?)
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
            let (bytes, branch) = match &pre.state {
                TerminalSelectedFormEncodingState::Encoded { bytes, .. } => (bytes.clone(), None),
                TerminalSelectedFormEncodingState::DeferredControl {
                    reason: DeferredTerminalControlEncodingReason::RequiresResolvedBranchLayout,
                } => resolve_branch(
                    architecture,
                    block,
                    instruction,
                    instruction_offset,
                    &block_offsets,
                    machine,
                    physical,
                )?,
            };
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

fn resolve_branch(
    architecture: Architecture,
    block: &TerminalSelectedBlock,
    instruction: &TerminalSelectedInstruction,
    instruction_offset: u64,
    block_offsets: &BTreeMap<TerminalSelectedBlockId, u64>,
    machine: &TerminalPostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
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
    let (bytes, effects) = match architecture {
        Architecture::X86_64 => {
            let encoded = encode_x86_64_terminal_selected_nonzero_branch_form(
                physical,
                machine.alternative.key,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::X86_64)?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().encoded.clone(),
            )
        }
        Architecture::Aarch64 => {
            let encoded = encode_aarch64_terminal_selected_nonzero_branch_form(
                physical,
                machine.alternative.key,
                displacement,
            )
            .map_err(OptimizedResolvedSelectedFormLayoutError::Aarch64)?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().encoded.clone(),
            )
        }
    };
    if effects != machine.alternative.encoded {
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
    instruction: TerminalSelectedInstructionId,
    row: &TerminalSelectedFormEncodingRow,
) -> Result<u64, OptimizedResolvedSelectedFormLayoutError> {
    match &row.state {
        TerminalSelectedFormEncodingState::Encoded { bytes, .. } => u64::try_from(bytes.len())
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow),
        TerminalSelectedFormEncodingState::DeferredControl {
            reason: DeferredTerminalControlEncodingReason::RequiresResolvedBranchLayout,
        } => {
            if row.instruction != instruction {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction),
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

fn layout_identity(
    selected: omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity,
    machine: omega_machine_optimizer::TerminalPostAllocationMachineIdentity,
    pre_layout: crate::TerminalSelectedFormEncodingIdentity,
    target: NativeTarget,
    policy: TerminalSelectedFunctionLayoutPolicy,
    functions: &[TerminalResolvedSelectedFunctionLayout],
) -> TerminalResolvedSelectedFormLayoutIdentity {
    let mut hasher = Sha256::new();
    hasher.update(LAYOUT_SCHEMA);
    hasher.update(selected.bytes());
    hasher.update(machine.bytes());
    hasher.update(pre_layout.bytes());
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
                        encode_effects(&mut hasher, &branch.decoded_effects);
                    }
                }
            }
        }
    }
    TerminalResolvedSelectedFormLayoutIdentity(hasher.finalize().into())
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
