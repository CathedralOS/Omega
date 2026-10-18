use machine_code::{
    FunctionFragmentControlProvenance, ResolvedSelectedBlockLayout, ResolvedSelectedFormRow,
    ResolvedSelectedFunctionLayout,
};
use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, SelectedBlock, SelectedBlockId,
    SelectedBlockOrigin, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedTerminator,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId};

use super::{ResolvedFragmentEmissionError, emit};

// These are projection inputs, not an ISA encoding test: bytes have already
// been chosen by layout. Descending, sparse identities rule out an accidental
// dependency on instruction IDs being dense offsets or sorted keys.
fn fixture(instruction_count: u32) -> (SelectedFunction, ResolvedSelectedFunctionLayout) {
    let mut selected = Vec::new();
    let mut rows = Vec::new();
    for ordinal in 0..=instruction_count {
        let terminal = ordinal == instruction_count;
        let id = SelectedInstructionId(u32::MAX - ordinal * 3);
        selected.push(SelectedInstruction {
            id,
            kind: if terminal {
                SelectedInstructionKind::ReturnUnit
            } else {
                SelectedInstructionKind::CopyI64
            },
            constraint: RegisterConstraintKey {
                family: if terminal {
                    RegisterConstraintFamily::Return
                } else {
                    RegisterConstraintFamily::Instruction
                },
                variant: 0,
            },
            operands: Vec::new(),
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: SelectedInstructionProvenance {
                operations: vec![OperationId::new(u64::from(ordinal) + 1).unwrap()],
                ..Default::default()
            },
        });
        rows.push(ResolvedSelectedFormRow {
            instruction: id,
            alternative: MachineAlternativeKey {
                family: if terminal {
                    MachineAlternativeFamily::ReturnUnit
                } else {
                    MachineAlternativeFamily::CopyI64
                },
                variant: 0,
            },
            offset: u64::from(ordinal) * 4,
            bytes: ordinal.to_le_bytes().to_vec(),
            branch: None,
            internal_machine_fixup: None,
            normalized_foreign_call_fixup: None,
            image_address_fixup: None,
        });
    }
    let machine = MachineId::new(1).unwrap();
    let block = SelectedBlockId(7);
    let terminal = selected.pop().unwrap();
    let byte_count = u64::from(instruction_count + 1) * 4;
    (
        SelectedFunction {
            machine,
            attachment: None,
            nominal_cleanup_subject: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: block,
            virtual_registers: Vec::new(),
            blocks: vec![SelectedBlock {
                id: block,
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: selected,
                terminator: SelectedTerminator::Return {
                    instruction: terminal,
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }],
        },
        ResolvedSelectedFunctionLayout {
            machine,
            byte_count,
            blocks: vec![ResolvedSelectedBlockLayout {
                block,
                offset: 0,
                byte_count,
                instructions: rows,
            }],
        },
    )
}

#[test]
fn ordered_projection_preserves_bytes_identity_and_provenance() {
    for instruction_count in [0, 3, 257] {
        let (selected, layout) = fixture(instruction_count);
        let fragment = emit(&selected, &layout).unwrap();
        assert_eq!(fragment.byte_count, layout.byte_count);
        assert_eq!(fragment.blocks.len(), 1);
        assert_eq!(
            fragment.blocks[0].instructions.len(),
            instruction_count as usize + 1
        );
        let expected_bytes: Vec<_> = (0..=instruction_count).flat_map(u32::to_le_bytes).collect();
        assert_eq!(fragment.bytes, expected_bytes);
        for (ordinal, span) in fragment.blocks[0].instructions.iter().enumerate() {
            let row = &layout.blocks[0].instructions[ordinal];
            assert_eq!(span.instruction, row.instruction);
            assert_eq!(span.alternative, row.alternative);
            assert_eq!(span.offset, row.offset);
            assert_eq!(span.bytes, row.bytes);
            assert_eq!(
                span.provenance.operations,
                vec![OperationId::new(ordinal as u64 + 1).unwrap()]
            );
            assert_eq!(
                span.control,
                if ordinal == instruction_count as usize {
                    FunctionFragmentControlProvenance::Return {
                        psi_return_edge: EdgeId::new(1).unwrap(),
                    }
                } else {
                    FunctionFragmentControlProvenance::None
                }
            );
        }
    }
}

#[test]
fn ordered_projection_rejects_missing_instruction() {
    let (selected, mut layout) = fixture(3);
    let missing = SelectedInstructionId(17);
    layout.blocks[0].instructions[1].instruction = missing;
    assert_eq!(
        emit(&selected, &layout),
        Err(ResolvedFragmentEmissionError::MissingInstruction(missing))
    );
}

#[test]
fn ordered_projection_preserves_zero_byte_rows() {
    let (selected, mut layout) = fixture(3);
    layout.blocks[0].instructions[0].bytes.clear();
    for row in &mut layout.blocks[0].instructions[1..] {
        row.offset -= 4;
    }
    layout.blocks[0].byte_count -= 4;
    layout.byte_count -= 4;
    let fragment = emit(&selected, &layout).unwrap();
    assert_eq!(fragment.blocks[0].instructions.len(), 4);
    assert!(fragment.blocks[0].instructions[0].bytes.is_empty());
    assert_eq!(
        fragment.blocks[0].instructions[0].provenance,
        selected.blocks[0].instructions[0].provenance
    );
    assert_eq!(
        fragment.bytes,
        (1_u32..=3).flat_map(u32::to_le_bytes).collect::<Vec<_>>()
    );
}

// Run on the same host/profile, without another build, both before and after
// changing the traversal. Fixture construction and result validation are outside
// the timer; timings include allocation and copying of the actual fragment.
#[test]
#[ignore = "manual fragment-emission scaling measurement"]
fn ordered_projection_scaling() {
    for instruction_count in [4096, 8192, 16384] {
        let (selected, layout) = fixture(instruction_count);
        let expected = emit(&selected, &layout).unwrap();
        let mut timings = Vec::new();
        for _ in 0..7 {
            let started = std::time::Instant::now();
            let actual = std::hint::black_box(emit(&selected, &layout).unwrap());
            timings.push(started.elapsed());
            assert_eq!(actual, expected);
        }
        timings.sort_unstable();
        eprintln!(
            "fragment rows={} median_ns={}",
            instruction_count + 1,
            timings[3].as_nanos()
        );
    }
}
