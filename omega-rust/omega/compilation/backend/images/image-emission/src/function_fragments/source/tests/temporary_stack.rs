//! Instruction IDs are local to each function; temporary peaks are not global.
use super::super::temporary_stack_bytes_for_function;
use machine_code::{
    SelectedFormDecodedFootprint, SelectedFormEncodedRowStore, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormMachineDisposition,
};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
    MachineEncodedStackEffect, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedFunction, SelectedFunctions, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedTerminator,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId};

fn instruction(id: u32, kind: SelectedInstructionKind) -> SelectedInstruction {
    SelectedInstruction {
        id: SelectedInstructionId(id),
        kind,
        constraint: register_model::RegisterConstraintKey {
            family: register_model::RegisterConstraintFamily::Instruction,
            variant: 0,
        },
        operands: Vec::new(),
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: Default::default(),
    }
}

fn function(machine: u64) -> SelectedFunction {
    SelectedFunction {
        machine: MachineId::new(machine).unwrap(),
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
        entry_block: SelectedBlockId(0),
        virtual_registers: Vec::new(),
        blocks: vec![SelectedBlock {
            id: SelectedBlockId(0),
            origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
            instructions: vec![instruction(0, SelectedInstructionKind::CopyI64)],
            terminator: SelectedTerminator::Return {
                instruction: instruction(1, SelectedInstructionKind::ReturnUnit),
                psi_return_edge: EdgeId::new(1).unwrap(),
            },
        }],
    }
}

fn alternative() -> MachineAlternativeKey {
    MachineAlternativeKey {
        family: MachineAlternativeFamily::CopyI64,
        variant: 0,
    }
}

fn encoding(peaks: &[u16]) -> (Vec<SelectedFormEncodingRow>, SelectedFormEncodedRowStore) {
    let mut store = SelectedFormEncodedRowStore::new();
    let mut rows = Vec::new();
    for &peak in peaks {
        // Typed footprint inputs to accounting, not a claim of ISA admission:
        // ordinary, balanced PUSHFQ and directed-FP envelopes require 0/8/16.
        for id in 0..2 {
            let mut effects = MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new());
            if id == 0 && peak != 0 {
                effects.stack = MachineEncodedStackEffect::TemporaryBytesV1 {
                    stack_pointer: register_model::RegisterViewId(4),
                    byte_count: peak,
                };
            }
            let (code, footprint) = store.intern(
                &[if id == 0 { 0x90 } else { 0xc3 }],
                SelectedFormDecodedFootprint {
                    register_reads: Vec::new(),
                    register_writes: Vec::new(),
                    implicit_defs: Vec::new(),
                    implicit_clobbers: Vec::new(),
                    encoded: effects,
                },
            );
            rows.push(SelectedFormEncodingRow {
                instruction: SelectedInstructionId(id),
                alternative: alternative(),
                machine_disposition: SelectedFormMachineDisposition::RetainedV1,
                state: SelectedFormEncodingState::Encoded,
                address: None,
                code,
                footprint,
            });
        }
    }
    (rows, store)
}

fn assert_function_peaks(order: &[(u64, u16)]) {
    let mut functions = SelectedFunctions::default();
    for &(machine, _) in order {
        functions.push(function(machine));
    }
    let (rows, store) = encoding(&order.iter().map(|&(_, peak)| peak).collect::<Vec<_>>());
    let actual = functions
        .iter()
        .map(|function| {
            temporary_stack_bytes_for_function(function.machine, &functions, &rows, &store).unwrap()
        })
        .collect::<Vec<_>>();
    let expected = order
        .iter()
        .map(|&(_, peak)| u32::from(peak))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn temporary_stack_keeps_ordinary_and_flags_functions_separate() {
    assert_function_peaks(&[(1, 0), (2, 8)]);
}

#[test]
fn temporary_stack_does_not_import_another_functions_directed_fp_peak() {
    assert_function_peaks(&[(1, 0), (2, 8), (3, 16)]);
}

#[test]
fn temporary_stack_identity_survives_reverse_function_order() {
    assert_function_peaks(&[(3, 16), (2, 8), (1, 0)]);
}

#[test]
fn temporary_stack_rejects_changed_missing_extra_and_duplicate_roster_rows() {
    let mut functions = SelectedFunctions::default();
    functions.push(function(1));
    functions.push(function(2));
    let (rows, store) = encoding(&[0, 8]);
    let check = |rows: &[SelectedFormEncodingRow]| {
        temporary_stack_bytes_for_function(MachineId::new(1).unwrap(), &functions, rows, &store)
    };
    check(&rows).unwrap();
    let mut changed = rows.clone();
    changed[1].instruction = SelectedInstructionId(7);
    assert!(check(&changed).is_err());
    assert!(check(&rows[..3]).is_err());
    changed = rows.clone();
    changed.push(rows[0].clone());
    assert!(check(&changed).is_err());
    let mut duplicate = functions.clone();
    duplicate[0].blocks[0].instructions[0].id = SelectedInstructionId(1);
    changed = rows.clone();
    changed[0].instruction = SelectedInstructionId(1);
    assert!(
        temporary_stack_bytes_for_function(
            MachineId::new(1).unwrap(),
            &duplicate,
            &changed,
            &store
        )
        .is_err()
    );
    assert!(
        temporary_stack_bytes_for_function(MachineId::new(3).unwrap(), &functions, &rows, &store)
            .is_err()
    );
}

#[test]
fn temporary_stack_keeps_tight_under_budget_numeric_controls() {
    let mut functions = SelectedFunctions::default();
    for machine in 1..=3 {
        functions.push(function(machine));
    }
    let (rows, store) = encoding(&[0, 8, 16]);
    for (machine, exact, insufficient) in [(2, 8, 7), (3, 16, 15)] {
        let required = temporary_stack_bytes_for_function(
            MachineId::new(machine).unwrap(),
            &functions,
            &rows,
            &store,
        )
        .unwrap();
        assert!(required <= exact);
        assert!(required > insufficient);
        // These are numerical controls on the accounting algorithm's output,
        // not authored-source provisioning or forged replay/admission tests.
    }
}
