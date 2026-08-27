use omega_machine_optimizer::{
    TerminalPostAllocationMachineIdentity, TerminalPostAllocationMachineInstruction,
};
use omega_regalloc::ValidatedTerminalSelectedAnalysis;
use omega_register_model::{RegisterUnitId, RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_target::Architecture;
use omega_terminal_isa_aarch64::{
    Aarch64SelectedFormEncodingError, encode_aarch64_terminal_selected_form,
};
use omega_terminal_isa_x86_64::{
    X86_64SelectedFormEncodingError, encode_x86_64_terminal_selected_form,
};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternativeFamily, TerminalMachineAlternativeKey, TerminalMachineEncodedEffects,
    TerminalMachineSizeKnowledge, TerminalSelectedInstruction, TerminalSelectedInstructionId,
    TerminalSelectedInstructionKind, TerminalSelectedTerminator,
};
use sha2::{Digest, Sha256};

use crate::StagedOptimizedPostAllocationMachinePlan;

const ENCODER_SCHEMA: &[u8] = b"omega.terminal.layout-independent-selected-form-encoding.v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalSelectedFormEncodingIdentity([u8; 32]);

impl TerminalSelectedFormEncodingIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredTerminalControlEncodingReason {
    RequiresResolvedBranchLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSelectedFormDecodedFootprint {
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub implicit_clobbers: Vec<RegisterUnitId>,
    pub encoded: TerminalMachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalSelectedFormEncodingState {
    Encoded {
        bytes: Vec<u8>,
        footprint: Box<TerminalSelectedFormDecodedFootprint>,
    },
    DeferredControl {
        reason: DeferredTerminalControlEncodingReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSelectedFormEncodingRow {
    pub instruction: TerminalSelectedInstructionId,
    pub alternative: TerminalMachineAlternativeKey,
    pub state: TerminalSelectedFormEncodingState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedSelectedFormEncoding {
    selected: omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity,
    machine: TerminalPostAllocationMachineIdentity,
    identity: TerminalSelectedFormEncodingIdentity,
    rows: Vec<TerminalSelectedFormEncodingRow>,
}

impl StagedOptimizedSelectedFormEncoding {
    pub const fn selected(
        &self,
    ) -> omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn machine(&self) -> TerminalPostAllocationMachineIdentity {
        self.machine
    }

    pub const fn identity(&self) -> TerminalSelectedFormEncodingIdentity {
        self.identity
    }

    pub fn rows(&self) -> &[TerminalSelectedFormEncodingRow] {
        &self.rows
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedSelectedFormEncodingError {
    SelectedRootMismatch,
    PhysicalModelMismatch,
    FunctionRosterMismatch,
    BlockRosterMismatch,
    InstructionRosterMismatch,
    OperandFootprintMismatch(TerminalSelectedInstructionId),
    ImplicitFootprintMismatch(TerminalSelectedInstructionId),
    SizeDeclarationMismatch(TerminalSelectedInstructionId),
    X86_64(X86_64SelectedFormEncodingError),
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

pub fn stage_optimized_layout_independent_selected_form_encoding<
    S: ValidatedTerminalSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let artifact = compute(selected, machine, physical)?;
    validate_optimized_layout_independent_selected_form_encoding(
        selected, machine, physical, &artifact,
    )?;
    Ok(artifact)
}

pub fn validate_optimized_layout_independent_selected_form_encoding<
    S: ValidatedTerminalSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    artifact: &StagedOptimizedSelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let replayed = compute(selected, machine, physical)?;
    if artifact != &replayed {
        return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
    }
    Ok(())
}

fn compute<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    staged: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let machine = staged.machine().plan();
    if machine.selected != selected.selected_identity() {
        return Err(OptimizedSelectedFormEncodingError::SelectedRootMismatch);
    }
    if machine.physical_register_model != physical.identity() {
        return Err(OptimizedSelectedFormEncodingError::PhysicalModelMismatch);
    }
    let selected_plan = selected.selected_plan();
    if selected_plan.functions.len() != machine.functions.len() {
        return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
    }
    let mut rows = Vec::new();
    for (selected_function, machine_function) in
        selected_plan.functions.iter().zip(&machine.functions)
    {
        if selected_function.machine != machine_function.machine
            || selected_function.blocks.len() != machine_function.blocks.len()
        {
            return Err(OptimizedSelectedFormEncodingError::FunctionRosterMismatch);
        }
        for (selected_block, machine_block) in selected_function
            .blocks
            .iter()
            .zip(&machine_function.blocks)
        {
            if selected_block.id != machine_block.block
                || selected_block.instructions.len() + 1 != machine_block.instructions.len()
            {
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
                rows.push(encode_row(
                    selected_plan.target.architecture,
                    selected_instruction,
                    machine_instruction,
                    physical,
                )?);
            }
        }
    }
    let selected_root = selected.selected_identity();
    let machine_root = staged.machine().receipt().identity();
    let identity = encoding_identity(selected_root, machine_root, &rows);
    Ok(StagedOptimizedSelectedFormEncoding {
        selected: selected_root,
        machine: machine_root,
        identity,
        rows,
    })
}

fn terminator_instruction(terminator: &TerminalSelectedTerminator) -> &TerminalSelectedInstruction {
    match terminator {
        TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
        | TerminalSelectedTerminator::Return { instruction, .. } => instruction,
    }
}

fn encode_row(
    architecture: Architecture,
    selected: &TerminalSelectedInstruction,
    machine: &TerminalPostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<TerminalSelectedFormEncodingRow, OptimizedSelectedFormEncodingError> {
    let alternative = machine.alternative.key;
    let state = match selected.kind {
        TerminalSelectedInstructionKind::ConditionalBranchNonZero => {
            TerminalSelectedFormEncodingState::DeferredControl {
                reason: DeferredTerminalControlEncodingReason::RequiresResolvedBranchLayout,
            }
        }
        kind => encode_scalar(
            architecture,
            selected.id,
            kind,
            alternative,
            machine,
            physical,
        )?,
    };
    Ok(TerminalSelectedFormEncodingRow {
        instruction: selected.id,
        alternative,
        state,
    })
}

fn encode_scalar(
    architecture: Architecture,
    instruction: TerminalSelectedInstructionId,
    kind: TerminalSelectedInstructionKind,
    alternative: TerminalMachineAlternativeKey,
    machine: &TerminalPostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<TerminalSelectedFormEncodingState, OptimizedSelectedFormEncodingError> {
    let views = machine
        .operands
        .iter()
        .map(|operand| operand.view)
        .collect::<Vec<_>>();
    let (bytes, reads, writes, encoded_effects) = match architecture {
        Architecture::X86_64 => {
            let encoded = encode_x86_64_terminal_selected_form(physical, kind, alternative, &views)
                .map_err(OptimizedSelectedFormEncodingError::X86_64)?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().register_writes.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
        Architecture::Aarch64 => {
            let encoded =
                encode_aarch64_terminal_selected_form(physical, kind, alternative, &views)
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
    Ok(TerminalSelectedFormEncodingState::Encoded {
        bytes,
        footprint: Box::new(TerminalSelectedFormDecodedFootprint {
            register_reads: reads,
            register_writes: writes,
            implicit_defs: encoded_effects.implicit_unit_defs.clone(),
            implicit_clobbers: encoded_effects.implicit_unit_clobbers.clone(),
            encoded: encoded_effects,
        }),
    })
}

fn validate_operand_footprint(
    instruction: TerminalSelectedInstructionId,
    machine: &TerminalPostAllocationMachineInstruction,
    encoded: &TerminalMachineEncodedEffects,
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
    instruction: TerminalSelectedInstructionId,
    knowledge: TerminalMachineSizeKnowledge,
    actual: usize,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let actual = u16::try_from(actual)
        .map_err(|_| OptimizedSelectedFormEncodingError::SizeDeclarationMismatch(instruction))?;
    let matches = match knowledge {
        TerminalMachineSizeKnowledge::ExactBytes(expected) => actual == expected,
        TerminalMachineSizeKnowledge::EncoderResolved {
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
    selected: omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity,
    machine: TerminalPostAllocationMachineIdentity,
    rows: &[TerminalSelectedFormEncodingRow],
) -> TerminalSelectedFormEncodingIdentity {
    let mut hasher = Sha256::new();
    hasher.update(ENCODER_SCHEMA);
    hasher.update(selected.bytes());
    hasher.update(machine.bytes());
    hasher.update((rows.len() as u64).to_le_bytes());
    for row in rows {
        hasher.update(row.instruction.0.to_le_bytes());
        encode_alternative(&mut hasher, row.alternative);
        match &row.state {
            TerminalSelectedFormEncodingState::Encoded { bytes, footprint } => {
                hasher.update([0]);
                hasher.update((bytes.len() as u64).to_le_bytes());
                hasher.update(bytes);
                encode_views(&mut hasher, &footprint.register_reads);
                encode_views(&mut hasher, &footprint.register_writes);
                encode_units(&mut hasher, &footprint.implicit_defs);
                encode_units(&mut hasher, &footprint.implicit_clobbers);
                encode_effects(&mut hasher, &footprint.encoded);
            }
            TerminalSelectedFormEncodingState::DeferredControl { reason } => {
                hasher.update([1]);
                hasher.update([match reason {
                    DeferredTerminalControlEncodingReason::RequiresResolvedBranchLayout => 0,
                }]);
            }
        }
    }
    TerminalSelectedFormEncodingIdentity(hasher.finalize().into())
}

fn encode_effects(hasher: &mut Sha256, effects: &TerminalMachineEncodedEffects) {
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
    use omega_terminal_selected_instructions::{
        TerminalMachineEncodedControlEffect as Control,
        TerminalMachineEncodedMemoryEffect as Memory, TerminalMachineEncodedStackEffect as Stack,
        TerminalMachineEncodedTrapBehavior as Trap,
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

fn encode_alternative(hasher: &mut Sha256, alternative: TerminalMachineAlternativeKey) {
    hasher.update([match alternative.family {
        TerminalMachineAlternativeFamily::CompareI64Zero => 0,
        TerminalMachineAlternativeFamily::MaterializeI64 => 1,
        TerminalMachineAlternativeFamily::CopyI64 => 2,
        TerminalMachineAlternativeFamily::ExactAddI64 => 3,
        TerminalMachineAlternativeFamily::ExactAddI64Immediate => 4,
        TerminalMachineAlternativeFamily::ExactSubtractI64 => 5,
        TerminalMachineAlternativeFamily::ConditionalBranchNonZero => 6,
        TerminalMachineAlternativeFamily::ReturnI64 => 7,
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
