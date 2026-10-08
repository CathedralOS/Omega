//! The plan identity reads each function's digest from its storage entry.
//! These tests pin that a memoized digest never outlives the content it was
//! computed for: every mutable route clears it, shared or unique, and the
//! memoized identity always equals the identity of an identical plan built
//! afresh.

use crate::{
    SelectedBlockId, SelectedFunction, SelectedFunctions, SelectedInstructionPlan,
    selected_instruction_plan_identity,
};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::NativeTarget;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

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
        blocks: Vec::new(),
    }
}

fn plan(functions: Vec<SelectedFunction>) -> SelectedInstructionPlan {
    SelectedInstructionPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        target: NativeTarget::linux_x64(),
        entry: MachineId::new(1).unwrap(),
        functions: SelectedFunctions::from(functions),
    }
}

fn edited(block: u32) -> SelectedFunction {
    let mut function = function(2);
    function.entry_block = SelectedBlockId(block);
    function
}

#[test]
fn a_memoized_digest_equals_the_digest_of_an_identical_fresh_plan() {
    let source = plan(vec![function(1), function(2)]);
    let identity = selected_instruction_plan_identity(&source);
    assert_eq!(identity, selected_instruction_plan_identity(&source));
    assert_eq!(
        identity,
        selected_instruction_plan_identity(&plan(vec![function(1), function(2)]))
    );
    assert_eq!(
        identity,
        selected_instruction_plan_identity(&source.clone()),
        "a clone shares the memoized digests of identical content"
    );
}

#[test]
fn every_mutable_route_rehashes_a_shared_function_and_spares_the_source() {
    let source = plan(vec![function(1), function(2)]);
    let identity = selected_instruction_plan_identity(&source);
    let expected = selected_instruction_plan_identity(&plan(vec![function(1), edited(7)]));
    assert_ne!(identity, expected);

    let mut indexed = source.clone();
    indexed.functions[1].entry_block = SelectedBlockId(7);
    assert_eq!(selected_instruction_plan_identity(&indexed), expected);
    assert!(
        indexed
            .functions
            .shares_function_storage(&source.functions, 0)
    );
    assert!(
        !indexed
            .functions
            .shares_function_storage(&source.functions, 1)
    );

    let mut got = source.clone();
    got.functions.get_mut(1).unwrap().entry_block = SelectedBlockId(7);
    assert_eq!(selected_instruction_plan_identity(&got), expected);

    let mut iterated = source.clone();
    iterated.functions.iter_mut().nth(1).unwrap().entry_block = SelectedBlockId(7);
    assert_eq!(selected_instruction_plan_identity(&iterated), expected);

    let mut looped = source.clone();
    for function in &mut looped.functions {
        if function.machine.get() == 2 {
            function.entry_block = SelectedBlockId(7);
        }
    }
    assert_eq!(selected_instruction_plan_identity(&looped), expected);

    assert_eq!(selected_instruction_plan_identity(&source), identity);
}

#[test]
fn a_uniquely_owned_function_edited_in_place_drops_its_digest() {
    let mut unique = plan(vec![function(1), function(2)]);
    let identity = selected_instruction_plan_identity(&unique);
    unique.functions[1].entry_block = SelectedBlockId(7);
    let expected = selected_instruction_plan_identity(&plan(vec![function(1), edited(7)]));
    assert_ne!(identity, expected);
    assert_eq!(selected_instruction_plan_identity(&unique), expected);

    unique.functions[1].entry_block = SelectedBlockId(0);
    assert_eq!(selected_instruction_plan_identity(&unique), identity);
}

#[test]
fn the_identity_binds_roster_order_count_and_roots() {
    let identity = selected_instruction_plan_identity(&plan(vec![function(1), function(2)]));
    assert_ne!(
        identity,
        selected_instruction_plan_identity(&plan(vec![function(2), function(1)]))
    );
    assert_ne!(
        identity,
        selected_instruction_plan_identity(&plan(vec![function(1)]))
    );
    let mut pushed = plan(vec![function(1)]);
    pushed.functions.push(function(2));
    assert_eq!(identity, selected_instruction_plan_identity(&pushed));
    let mut retargeted = plan(vec![function(1), function(2)]);
    retargeted.target = NativeTarget::linux_arm64();
    assert_ne!(identity, selected_instruction_plan_identity(&retargeted));
    let mut reentered = plan(vec![function(1), function(2)]);
    reentered.entry = MachineId::new(2).unwrap();
    assert_ne!(identity, selected_instruction_plan_identity(&reentered));
}
