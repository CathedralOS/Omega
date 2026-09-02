use omega_abstract_operations::ValueBinding;
use omega_optimization_core::FunctionFragmentEmissionIdentity;
use omega_optimization_unit::{FuelSettlement, PsiProvenance};
use omega_register_model::RegisterViewId;
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_target_operations::TerminalPsiProvenance;
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerCarrier, IntegerSign, MachineId, OperationId,
    ScalarType,
};
use psi_terminal::TerminalPsiIdentity;
use sha2::{Digest, Sha256};

const FRAGMENT_SCHEMA: &[u8] = b"omega.terminal.function-fragment-emission.v4";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentEmissionPlan {
    pub identity: FunctionFragmentEmissionIdentity,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<FunctionFragment>,
    pub structural_unit_functions: Vec<StructuralUnitFunctionFragment>,
}

/// Function fragment for the structural-ABI Unit lane. Its call bytes remain
/// non-executable until whole-text placement discharges every typed fixup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralUnitFunctionFragment {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub byte_count: u64,
    pub bytes: Vec<u8>,
    pub block: StructuralUnitFunctionFragmentBlockSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralUnitFunctionFragmentBlockSpan {
    pub block: SelectedBlockId,
    pub offset: u64,
    pub byte_count: u64,
    pub call: Option<StructuralUnitCallFragmentSpan>,
    pub return_instruction: FunctionFragmentInstructionSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralUnitCallFragmentSpan {
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub provenance: SelectedInstructionProvenance,
    pub fixup: FunctionFragmentInternalMachineFixup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentInternalMachineFixupKind {
    X86Relative32FromNextInstructionToInternalMachineV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentInternalMachineFixupState {
    UnresolvedZeroFieldV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionFragmentInternalMachineFixup {
    pub kind: FunctionFragmentInternalMachineFixupKind,
    pub state: FunctionFragmentInternalMachineFixupState,
    pub callee: MachineId,
    pub opcode_function_offset: u64,
    pub field_function_offset: u64,
    pub next_instruction_function_offset: u64,
    pub field_byte_width: u8,
    pub addend: i64,
}

impl FunctionFragmentEmissionPlan {
    pub fn recomputed_identity(&self) -> FunctionFragmentEmissionIdentity {
        function_fragment_emission_identity(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragment {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub byte_count: u64,
    pub bytes: Vec<u8>,
    pub blocks: Vec<FunctionFragmentBlockSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentBlockSpan {
    pub block: SelectedBlockId,
    pub offset: u64,
    pub byte_count: u64,
    pub instructions: Vec<FunctionFragmentInstructionSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentInstructionSpan {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternativeKey,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub branch: Option<Box<FunctionFragmentConditionalBranchEvidence>>,
    pub provenance: SelectedInstructionProvenance,
    pub control: FunctionFragmentControlProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionFragmentControlProvenance {
    None,
    ConditionalBranch {
        predicate: FunctionFragmentConditionalBranchPredicate,
        when_taken: FunctionFragmentSuccessorProvenance,
        when_fallthrough: FunctionFragmentSuccessorProvenance,
    },
    Return {
        psi_return_edge: EdgeId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentConditionalBranchPredicate {
    NonZeroV1,
    U64LessThanV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentSuccessorProvenance {
    pub psi_edge: EdgeId,
    pub block: SelectedBlockId,
    pub source_target: BlockId,
    pub bindings: Vec<ValueBinding>,
    pub fuel: Vec<FuelSettlement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentConditionalBranchEvidence {
    pub predicate: FunctionFragmentConditionalBranchPredicate,
    pub source_block: SelectedBlockId,
    pub when_taken_edge: EdgeId,
    pub when_taken_block: SelectedBlockId,
    pub when_taken_offset: u64,
    pub when_fallthrough_edge: EdgeId,
    pub when_fallthrough_block: SelectedBlockId,
    pub when_fallthrough_offset: u64,
    pub byte_displacement: i64,
    pub decoded_register_reads: Vec<RegisterViewId>,
    pub decoded_effects: MachineEncodedEffects,
}

pub fn function_fragment_emission_identity(
    plan: &FunctionFragmentEmissionPlan,
) -> FunctionFragmentEmissionIdentity {
    let mut hasher = Sha256::new();
    hasher.update(FRAGMENT_SCHEMA);
    hasher.update(plan.psi.vocabulary_marker.get().to_le_bytes());
    hasher.update(plan.psi.program_fingerprint.as_bytes());
    hasher.update(plan.fuel_schedule.marker().to_le_bytes());
    hasher.update(plan.selected.bytes());
    encode_target(&mut hasher, plan.target);
    hasher.update(plan.entry.get().to_le_bytes());
    hasher.update((plan.functions.len() as u64).to_le_bytes());
    for function in &plan.functions {
        hasher.update(function.machine.get().to_le_bytes());
        match function.attachment {
            None => hasher.update([0]),
            Some(attachment) => {
                hasher.update([1]);
                hasher.update(attachment.get().to_le_bytes());
            }
        }
        encode_function_provenance(&mut hasher, &function.provenance);
        hasher.update(function.byte_count.to_le_bytes());
        encode_bytes(&mut hasher, &function.bytes);
        hasher.update((function.blocks.len() as u64).to_le_bytes());
        for block in &function.blocks {
            hasher.update(block.block.0.to_le_bytes());
            hasher.update(block.offset.to_le_bytes());
            hasher.update(block.byte_count.to_le_bytes());
            hasher.update((block.instructions.len() as u64).to_le_bytes());
            for row in &block.instructions {
                hasher.update(row.instruction.0.to_le_bytes());
                encode_alternative(&mut hasher, row.alternative);
                hasher.update(row.offset.to_le_bytes());
                encode_bytes(&mut hasher, &row.bytes);
                encode_branch(&mut hasher, row.branch.as_deref());
                encode_instruction_provenance(&mut hasher, &row.provenance);
                encode_control(&mut hasher, &row.control);
            }
        }
    }
    hasher.update((plan.structural_unit_functions.len() as u64).to_le_bytes());
    for function in &plan.structural_unit_functions {
        hasher.update(function.machine.get().to_le_bytes());
        match function.attachment {
            None => hasher.update([0]),
            Some(attachment) => {
                hasher.update([1]);
                hasher.update(attachment.get().to_le_bytes());
            }
        }
        encode_function_provenance(&mut hasher, &function.provenance);
        hasher.update(function.byte_count.to_le_bytes());
        encode_bytes(&mut hasher, &function.bytes);
        let block = &function.block;
        hasher.update(block.block.0.to_le_bytes());
        hasher.update(block.offset.to_le_bytes());
        hasher.update(block.byte_count.to_le_bytes());
        match &block.call {
            None => hasher.update([0]),
            Some(call) => {
                hasher.update([1]);
                hasher.update(call.instruction.0.to_le_bytes());
                hasher.update(call.operation.get().to_le_bytes());
                hasher.update(call.callee.get().to_le_bytes());
                hasher.update(call.offset.to_le_bytes());
                encode_bytes(&mut hasher, &call.bytes);
                encode_instruction_provenance(&mut hasher, &call.provenance);
                encode_internal_machine_fixup(&mut hasher, call.fixup);
            }
        }
        encode_instruction_span(&mut hasher, &block.return_instruction);
    }
    FunctionFragmentEmissionIdentity::from_canonical_bytes(&hasher.finalize())
}

fn encode_instruction_span(hasher: &mut Sha256, row: &FunctionFragmentInstructionSpan) {
    hasher.update(row.instruction.0.to_le_bytes());
    encode_alternative(hasher, row.alternative);
    hasher.update(row.offset.to_le_bytes());
    encode_bytes(hasher, &row.bytes);
    encode_branch(hasher, row.branch.as_deref());
    encode_instruction_provenance(hasher, &row.provenance);
    encode_control(hasher, &row.control);
}

fn encode_internal_machine_fixup(hasher: &mut Sha256, fixup: FunctionFragmentInternalMachineFixup) {
    hasher.update([match fixup.kind {
        FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 => 1,
    }]);
    hasher.update([match fixup.state {
        FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1 => 1,
    }]);
    hasher.update(fixup.callee.get().to_le_bytes());
    hasher.update(fixup.opcode_function_offset.to_le_bytes());
    hasher.update(fixup.field_function_offset.to_le_bytes());
    hasher.update(fixup.next_instruction_function_offset.to_le_bytes());
    hasher.update([fixup.field_byte_width]);
    hasher.update(fixup.addend.to_le_bytes());
}

fn encode_target(hasher: &mut Sha256, target: NativeTarget) {
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
}

fn encode_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn encode_function_provenance(hasher: &mut Sha256, provenance: &TerminalPsiProvenance) {
    encode_semantic_ids(hasher, provenance.operations.iter().map(|id| id.get()));
    encode_semantic_ids(hasher, provenance.edges.iter().map(|id| id.get()));
}

fn encode_instruction_provenance(hasher: &mut Sha256, provenance: &SelectedInstructionProvenance) {
    encode_semantic_ids(hasher, provenance.operations.iter().map(|id| id.get()));
    encode_semantic_ids(hasher, provenance.values.iter().map(|id| id.get()));
    encode_semantic_ids(hasher, provenance.edges.iter().map(|id| id.get()));
    encode_semantic_ids(hasher, provenance.obligations.iter().map(|id| id.get()));
    encode_fuel(hasher, &provenance.fuel);
}

fn encode_semantic_ids(hasher: &mut Sha256, ids: impl ExactSizeIterator<Item = u64>) {
    hasher.update((ids.len() as u64).to_le_bytes());
    for id in ids {
        hasher.update(id.to_le_bytes());
    }
}

fn encode_fuel(hasher: &mut Sha256, fuel: &[FuelSettlement]) {
    hasher.update((fuel.len() as u64).to_le_bytes());
    for settlement in fuel {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                hasher.update([0]);
                hasher.update(operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                hasher.update([1]);
                hasher.update(edge.get().to_le_bytes());
            }
        }
        hasher.update(settlement.units.to_le_bytes());
    }
}

fn encode_control(hasher: &mut Sha256, control: &FunctionFragmentControlProvenance) {
    match control {
        FunctionFragmentControlProvenance::None => hasher.update([0]),
        FunctionFragmentControlProvenance::ConditionalBranch {
            predicate,
            when_taken,
            when_fallthrough,
        } => {
            hasher.update([1]);
            encode_branch_predicate(hasher, *predicate);
            encode_successor(hasher, when_taken);
            encode_successor(hasher, when_fallthrough);
        }
        FunctionFragmentControlProvenance::Return { psi_return_edge } => {
            hasher.update([2]);
            hasher.update(psi_return_edge.get().to_le_bytes());
        }
    }
}

fn encode_successor(hasher: &mut Sha256, successor: &FunctionFragmentSuccessorProvenance) {
    hasher.update(successor.psi_edge.get().to_le_bytes());
    hasher.update(successor.block.0.to_le_bytes());
    hasher.update(successor.source_target.get().to_le_bytes());
    hasher.update((successor.bindings.len() as u64).to_le_bytes());
    for binding in &successor.bindings {
        hasher.update(binding.parameter.get().to_le_bytes());
        hasher.update(binding.argument.get().to_le_bytes());
        match binding.scalar_type {
            ScalarType::Boolean => hasher.update([0]),
            ScalarType::Integer(integer) => {
                hasher.update([1]);
                hasher.update([match integer.carrier() {
                    IntegerCarrier::Fixed => 0,
                    IntegerCarrier::Address => 1,
                }]);
                hasher.update([match integer.sign() {
                    IntegerSign::Signed => 0,
                    IntegerSign::Unsigned => 1,
                }]);
                hasher.update(integer.bits().to_le_bytes());
            }
            ScalarType::IeeeFloat(format) => {
                hasher.update([2]);
                hasher.update([match format {
                    psi_core::IeeeFloatFormat::Binary32 => 0,
                    psi_core::IeeeFloatFormat::Binary64 => 1,
                }]);
            }
        }
    }
    encode_fuel(hasher, &successor.fuel);
}

fn encode_branch(hasher: &mut Sha256, branch: Option<&FunctionFragmentConditionalBranchEvidence>) {
    let Some(branch) = branch else {
        hasher.update([0]);
        return;
    };
    hasher.update([1]);
    encode_branch_predicate(hasher, branch.predicate);
    hasher.update(branch.source_block.0.to_le_bytes());
    hasher.update(branch.when_taken_edge.get().to_le_bytes());
    hasher.update(branch.when_taken_block.0.to_le_bytes());
    hasher.update(branch.when_taken_offset.to_le_bytes());
    hasher.update(branch.when_fallthrough_edge.get().to_le_bytes());
    hasher.update(branch.when_fallthrough_block.0.to_le_bytes());
    hasher.update(branch.when_fallthrough_offset.to_le_bytes());
    hasher.update(branch.byte_displacement.to_le_bytes());
    hasher.update((branch.decoded_register_reads.len() as u64).to_le_bytes());
    for read in &branch.decoded_register_reads {
        hasher.update(read.0.to_le_bytes());
    }
    encode_effects(hasher, &branch.decoded_effects);
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
        MachineAlternativeFamily::CompareI64 => 10,
        MachineAlternativeFamily::ConditionalBranchU64LessThan => 11,
    }]);
    hasher.update(alternative.variant.to_le_bytes());
}

fn encode_branch_predicate(
    hasher: &mut Sha256,
    predicate: FunctionFragmentConditionalBranchPredicate,
) {
    hasher.update([match predicate {
        FunctionFragmentConditionalBranchPredicate::NonZeroV1 => 0,
        FunctionFragmentConditionalBranchPredicate::U64LessThanV1 => 1,
    }]);
}

fn encode_effects(hasher: &mut Sha256, effects: &MachineEncodedEffects) {
    encode_u16s(hasher, &effects.external_operand_reads);
    encode_u16s(hasher, &effects.external_operand_writes);
    encode_u16s(
        hasher,
        &effects
            .implicit_unit_uses
            .iter()
            .map(|id| id.0)
            .collect::<Vec<_>>(),
    );
    encode_u16s(
        hasher,
        &effects
            .implicit_unit_defs
            .iter()
            .map(|id| id.0)
            .collect::<Vec<_>>(),
    );
    encode_u16s(
        hasher,
        &effects
            .implicit_unit_clobbers
            .iter()
            .map(|id| id.0)
            .collect::<Vec<_>>(),
    );
    match effects.memory {
        MachineEncodedMemoryEffect::NoneV1 => hasher.update([0]),
        MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
    }
    match effects.stack {
        MachineEncodedStackEffect::UnchangedV1 => hasher.update([0]),
        MachineEncodedStackEffect::PopBytesV1 {
            stack_pointer,
            byte_count,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(byte_count.to_le_bytes());
        }
    }
    hasher.update([match effects.trap {
        MachineEncodedTrapBehavior::NeverV1 => 0,
        MachineEncodedTrapBehavior::MayArchitecturalFaultV1 => 1,
    }]);
    match effects.control {
        MachineEncodedControlEffect::FallThroughV1 => hasher.update([0]),
        MachineEncodedControlEffect::ConditionalRelativeBranchV1 => hasher.update([1]),
        MachineEncodedControlEffect::ReturnFromActivationStackV1 => hasher.update([2]),
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target } => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use psi_core::ValueId;

    fn zero_span_plan() -> FunctionFragmentEmissionPlan {
        let mut plan = FunctionFragmentEmissionPlan {
            identity: FunctionFragmentEmissionIdentity::from_canonical_bytes(b"pending"),
            psi: TerminalPsiIdentity {
                vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
                program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([1; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            selected: SelectedInstructionPlanIdentity::from_bytes([2; 32]),
            target: NativeTarget::linux_x64(),
            entry: MachineId::new(1).unwrap(),
            functions: vec![FunctionFragment {
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                byte_count: 0,
                bytes: Vec::new(),
                blocks: vec![FunctionFragmentBlockSpan {
                    block: SelectedBlockId(0),
                    offset: 0,
                    byte_count: 0,
                    instructions: vec![FunctionFragmentInstructionSpan {
                        instruction: SelectedInstructionId(0),
                        alternative: MachineAlternativeKey {
                            family: MachineAlternativeFamily::CompareI64Zero,
                            variant: 0,
                        },
                        offset: 0,
                        bytes: Vec::new(),
                        branch: None,
                        provenance: SelectedInstructionProvenance::default(),
                        control: FunctionFragmentControlProvenance::None,
                    }],
                }],
            }],
            structural_unit_functions: Vec::new(),
        };
        plan.identity = plan.recomputed_identity();
        plan
    }

    #[test]
    fn fragment_identity_binds_zero_spans_aggregate_bytes_and_provenance() {
        let original = zero_span_plan();
        assert_eq!(original.identity, original.recomputed_identity());

        let mut changed = original.clone();
        changed.functions[0].bytes.push(0x90);
        assert_ne!(changed.recomputed_identity(), original.identity);

        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions[0]
            .provenance
            .values
            .push(ValueId::new(7).unwrap());
        assert_ne!(changed.recomputed_identity(), original.identity);

        let mut changed = original.clone();
        changed.functions[0].blocks[0].instructions.clear();
        assert_ne!(changed.recomputed_identity(), original.identity);
    }
}
