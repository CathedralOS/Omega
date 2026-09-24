//! AArch64 conditional branches whose taken edge leaves the `B.cond` imm19
//! range must emit the widened `B.<invcond> +8; B target` row, and independent
//! admission must replay the same widened layout. These fixtures build the
//! selected program and pre-layout rows directly; they do not manufacture
//! selection authority.

use isa_aarch64::{
    aarch64_physical_register_model, encode_aarch64_selected_jump_form,
    encode_aarch64_selected_nonzero_branch_form,
};
use machine_code::{
    DeferredControlEncodingReason, SelectedFormDecodedFootprint, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormMachineDisposition,
};
use physical_instructions::{
    PostAllocationMachineBlock, PostAllocationMachineFunction, PostAllocationMachineInstruction,
};
use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineLatencyKnowledge, MachineSizeKnowledge, SelectedBlock,
    SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedSuccessor, SelectedTerminator,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId};
use target::Architecture;

use super::validate;
use crate::resolved_selected_form_layout::ordinary::layout;
use crate::resolved_selected_form_layout::{ResolvedBranchEvidence, SelectedFunctionLayoutPolicy};

/// `B.cond` rejects word displacements outside `-(1<<18)..1<<18`; 262,144
/// four-byte padding rows put the taken edge just past that bound.
const PAD_BEYOND_IMM19: usize = (1 << 18) + 4;

fn instruction(block: u32, kind: SelectedInstructionKind) -> SelectedInstruction {
    SelectedInstruction {
        id: SelectedInstructionId(block),
        kind,
        constraint: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 0,
        },
        operands: Vec::new(),
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: Default::default(),
    }
}

fn successor(target: u32) -> SelectedSuccessor {
    SelectedSuccessor {
        structural_case: None,
        role: selected_instructions::SelectedSuccessorRole::Semantic,
        structural_bindings: Vec::new(),
        psi_edge: EdgeId::new(u64::from(target) + 1).unwrap(),
        block: SelectedBlockId(target),
        source_target: BlockId::new(u64::from(target) + 1).unwrap(),
        bindings: Vec::new(),
        fuel: Vec::new(),
    }
}

fn branch(source: u32, taken: u32, otherwise: u32) -> SelectedTerminator {
    SelectedTerminator::ConditionalBranch {
        instruction: instruction(source, SelectedInstructionKind::ConditionalBranchNonZero),
        when_nonzero: successor(taken),
        when_zero: successor(otherwise),
    }
}

fn jump(source: u32, destination: u32) -> SelectedTerminator {
    SelectedTerminator::Jump {
        instruction: instruction(source, SelectedInstructionKind::Jump),
        successor: successor(destination),
    }
}

fn returned(source: u32) -> SelectedTerminator {
    SelectedTerminator::Return {
        instruction: instruction(source, SelectedInstructionKind::ReturnUnit),
        psi_return_edge: EdgeId::new(u64::from(source) + 1).unwrap(),
    }
}

fn block(id: u32, terminator: SelectedTerminator) -> SelectedBlock {
    block_with(id, Vec::new(), terminator)
}

fn block_with(
    id: u32,
    instructions: Vec<SelectedInstruction>,
    terminator: SelectedTerminator,
) -> SelectedBlock {
    SelectedBlock {
        id: SelectedBlockId(id),
        origin: selected_instructions::SelectedBlockOrigin::Source(
            BlockId::new(u64::from(id) + 1).unwrap(),
        ),
        instructions,
        terminator,
    }
}

fn function(blocks: Vec<SelectedBlock>) -> SelectedFunction {
    SelectedFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        provenance: Default::default(),
        structural: None,
        local_storage_slots: Vec::new(),
        outgoing_arguments: Vec::new(),
        calls: Vec::new(),
        normalized_foreign_calls: Vec::new(),
        memory_accesses: Vec::new(),
        boundary_settlements: Vec::new(),
        entry_block: SelectedBlockId(0),
        virtual_registers: Vec::new(),
        blocks,
    }
}

fn physical() -> register_model::ValidatedPhysicalRegisterModel {
    register_model::validate_physical_register_model(aarch64_physical_register_model()).unwrap()
}

fn branch_alternative() -> MachineAlternative {
    let physical = physical();
    let key = MachineAlternativeKey {
        family: MachineAlternativeFamily::ConditionalBranchNonZero,
        variant: 0,
    };
    let encoded = encode_aarch64_selected_nonzero_branch_form(&physical, key, 8).unwrap();
    MachineAlternative {
        key,
        applicability: MachineAlternativeApplicability::Always,
        size: MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 4,
            maximum_bytes: Some(8),
        },
        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
        encoded: encoded.footprint().encoded.clone(),
    }
}

fn jump_alternative() -> MachineAlternative {
    let physical = physical();
    let key = MachineAlternativeKey {
        family: MachineAlternativeFamily::Jump,
        variant: 0,
    };
    let encoded = encode_aarch64_selected_jump_form(&physical, key, 4).unwrap();
    MachineAlternative {
        key,
        applicability: MachineAlternativeApplicability::Always,
        size: MachineSizeKnowledge::ExactBytes(4),
        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
        encoded: encoded.footprint().encoded.clone(),
    }
}

fn plain_alternative() -> MachineAlternative {
    MachineAlternative {
        key: MachineAlternativeKey {
            family: MachineAlternativeFamily::CopyI64,
            variant: 0,
        },
        applicability: MachineAlternativeApplicability::Always,
        size: MachineSizeKnowledge::ExactBytes(4),
        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
        encoded: branch_alternative().encoded,
    }
}

fn row_alternative(
    instruction: &SelectedInstruction,
    alternatives: &Alternatives,
) -> MachineAlternativeKey {
    alternatives.of(instruction).key
}

fn footprint(alternatives: &Alternatives) -> SelectedFormDecodedFootprint {
    SelectedFormDecodedFootprint {
        register_reads: Vec::new(),
        register_writes: Vec::new(),
        implicit_defs: Vec::new(),
        implicit_clobbers: Vec::new(),
        encoded: alternatives.plain.encoded.clone(),
    }
}

fn encoded_row(
    instruction: &SelectedInstruction,
    footprint: &SelectedFormDecodedFootprint,
) -> SelectedFormEncodingRow {
    SelectedFormEncodingRow {
        instruction: instruction.id,
        alternative: MachineAlternativeKey {
            family: MachineAlternativeFamily::CopyI64,
            variant: 0,
        },
        machine_disposition: SelectedFormMachineDisposition::RetainedV1,
        state: SelectedFormEncodingState::Encoded {
            bytes: vec![0; 4],
            footprint: Box::new(footprint.clone()),
        },
        address: None,
    }
}

fn deferred_row(
    instruction: &SelectedInstruction,
    alternatives: &Alternatives,
) -> SelectedFormEncodingRow {
    SelectedFormEncodingRow {
        instruction: instruction.id,
        alternative: row_alternative(instruction, alternatives),
        machine_disposition: SelectedFormMachineDisposition::RetainedV1,
        state: SelectedFormEncodingState::DeferredControl {
            reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
        },
        address: None,
    }
}

fn terminator_row(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::Return { instruction, .. }
        | SelectedTerminator::Crash { instruction, .. }
        | SelectedTerminator::HostedExitProcess { instruction, .. } => instruction,
    }
}

fn machine_row(
    instruction: &SelectedInstruction,
    alternative: &MachineAlternative,
) -> PostAllocationMachineInstruction {
    PostAllocationMachineInstruction {
        instruction: instruction.id,
        alternative: alternative.clone(),
        operands: Vec::new(),
        address: None,
        implicit_unit_uses: Vec::new(),
        implicit_unit_defs: Vec::new(),
        implicit_unit_clobbers: Vec::new(),
        unit_uses: Vec::new(),
        unit_defs: Vec::new(),
        unit_clobbers: Vec::new(),
    }
}

/// Rows must arrive in declared block order, terminator last per block.
fn pre_layout_rows(
    function: &SelectedFunction,
    footprint: &SelectedFormDecodedFootprint,
    alternatives: &Alternatives,
) -> Vec<SelectedFormEncodingRow> {
    let mut rows = Vec::new();
    for block in &function.blocks {
        for instruction in &block.instructions {
            rows.push(encoded_row(instruction, footprint));
        }
        let terminator = terminator_row(&block.terminator);
        rows.push(match &block.terminator {
            SelectedTerminator::ConditionalBranch { .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { .. }
            | SelectedTerminator::Jump { .. } => deferred_row(terminator, alternatives),
            _ => encoded_row(terminator, footprint),
        });
    }
    rows
}

fn machine_function(
    function: &SelectedFunction,
    alternatives: &Alternatives,
) -> PostAllocationMachineFunction {
    PostAllocationMachineFunction {
        machine: function.machine,
        local_storage_slots: Vec::new(),
        outgoing_arguments: Vec::new(),
        blocks: function
            .blocks
            .iter()
            .map(|block| PostAllocationMachineBlock {
                block: block.id,
                instructions: block
                    .instructions
                    .iter()
                    .chain(std::iter::once(terminator_row(&block.terminator)))
                    .map(|instruction| machine_row(instruction, alternatives.of(instruction)))
                    .collect(),
            })
            .collect(),
    }
}

struct Alternatives {
    branch: MachineAlternative,
    jump: MachineAlternative,
    plain: MachineAlternative,
}

impl Alternatives {
    fn of(&self, instruction: &SelectedInstruction) -> &MachineAlternative {
        match instruction.kind {
            SelectedInstructionKind::ConditionalBranchNonZero => &self.branch,
            SelectedInstructionKind::Jump => &self.jump,
            _ => &self.plain,
        }
    }
}

fn produce_and_admit(
    selected: &SelectedFunction,
) -> crate::resolved_selected_form_layout::ResolvedSelectedFunctionLayout {
    let alternatives = Alternatives {
        branch: branch_alternative(),
        jump: jump_alternative(),
        plain: plain_alternative(),
    };
    let footprint = footprint(&alternatives);
    let rows = pre_layout_rows(selected, &footprint, &alternatives);
    let machine = machine_function(selected, &alternatives);
    let pre_rows: std::collections::BTreeMap<_, _> =
        rows.iter().map(|row| (row.instruction, row)).collect();
    let machine_rows: std::collections::BTreeMap<_, _> = machine
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .map(|instruction| (instruction.instruction, instruction))
        .collect();
    let produced = layout(
        Architecture::Aarch64,
        selected,
        &pre_rows,
        &machine_rows,
        &physical(),
        SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1,
    )
    .unwrap();
    let mut iter = rows.iter();
    validate(
        Architecture::Aarch64,
        selected,
        &machine,
        &physical(),
        SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1,
        &mut iter,
        &produced,
    )
    .unwrap();
    assert!(iter.next().is_none());
    produced
}

#[test]
fn taken_edge_beyond_imm19_widens_to_inverted_skip_plus_unconditional_branch() {
    let padding: Vec<SelectedInstruction> = (0..PAD_BEYOND_IMM19 as u32)
        .map(|index| instruction(1000 + index, SelectedInstructionKind::CopyI64))
        .collect();
    let selected = function(vec![
        block(0, branch(0, 2, 1)),
        block_with(1, padding, jump(1, 2)),
        block(2, returned(2)),
    ]);
    let layout = produce_and_admit(&selected);
    let row = &layout.blocks[0].instructions[0];
    assert_eq!(row.bytes.len(), 8);
    let skip = u32::from_le_bytes(row.bytes[0..4].try_into().unwrap());
    // `B.EQ +8` skips the unconditional branch when the predicate is false.
    assert_eq!(skip, 0x5400_0000 | (2 << 5));
    let target = u32::from_le_bytes(row.bytes[4..8].try_into().unwrap());
    assert_eq!(target & 0xfc00_0000, 0x1400_0000);
    let branch_offset = layout.blocks[0].instructions[0].offset;
    let taken_offset = layout.blocks[2].offset;
    let decoded = i64::from(((target & 0x03ff_ffff) << 6) as i32 >> 6) * 4;
    assert_eq!(
        decoded,
        i64::try_from(taken_offset).unwrap() - i64::try_from(branch_offset).unwrap() - 4
    );
    assert!(i64::try_from(taken_offset).unwrap() - i64::try_from(branch_offset).unwrap() > 1 << 20);
    let Some(ResolvedBranchEvidence::Conditional(evidence)) =
        layout.blocks[0].instructions[0].branch.as_deref()
    else {
        panic!("conditional branch must carry conditional evidence")
    };
    assert_eq!(
        evidence.byte_displacement,
        i64::try_from(taken_offset).unwrap() - i64::try_from(branch_offset).unwrap()
    );
    // The unconditional jump out of the padding block also binds its target.
    let jump_row = layout.blocks[1].instructions.last().unwrap();
    assert_eq!(jump_row.bytes.len(), 4);
    assert_eq!(
        u32::from_le_bytes(jump_row.bytes[..4].try_into().unwrap()) & 0xfc00_0000,
        0x1400_0000
    );
}

#[test]
fn backward_taken_edge_beyond_imm19_widens() {
    let padding: Vec<SelectedInstruction> = (0..PAD_BEYOND_IMM19 as u32)
        .map(|index| instruction(2000 + index, SelectedInstructionKind::CopyI64))
        .collect();
    // Taken edge jumps back to the entry block from past the imm19 range.
    let selected = function(vec![
        block(0, jump(0, 1)),
        block_with(1, padding, branch(1, 0, 2)),
        block(2, returned(2)),
    ]);
    let layout = produce_and_admit(&selected);
    let row = layout.blocks[1].instructions.last().unwrap();
    assert_eq!(row.bytes.len(), 8);
    let skip = u32::from_le_bytes(row.bytes[0..4].try_into().unwrap());
    assert_eq!(skip, 0x5400_0000 | (2 << 5));
    let target = u32::from_le_bytes(row.bytes[4..8].try_into().unwrap());
    let decoded = i64::from(((target & 0x03ff_ffff) << 6) as i32 >> 6) * 4;
    // The `B` sits one word into the row, so its displacement reaches back
    // from `offset + 4` to the entry block at offset zero.
    assert_eq!(decoded, -(i64::try_from(row.offset).unwrap()) - 4);
}

#[test]
fn in_range_branch_keeps_the_canonical_four_byte_form() {
    let padding: Vec<SelectedInstruction> = (0..4u32)
        .map(|index| instruction(3000 + index, SelectedInstructionKind::CopyI64))
        .collect();
    let selected = function(vec![
        block(0, branch(0, 2, 1)),
        block_with(1, padding, jump(1, 2)),
        block(2, returned(2)),
    ]);
    let layout = produce_and_admit(&selected);
    let row = &layout.blocks[0].instructions[0];
    assert_eq!(row.bytes.len(), 4);
    let word = u32::from_le_bytes(row.bytes[..4].try_into().unwrap());
    assert_eq!(word & 0xff00_001f, 0x5400_0001);
}
