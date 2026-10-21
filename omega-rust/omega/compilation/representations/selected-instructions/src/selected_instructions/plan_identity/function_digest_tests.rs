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
        blocks: (Vec::new()).into(),
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

// Raw block storage and identity checks, not semantic admission fixtures.
fn block(identifier: u32) -> crate::SelectedBlock {
    crate::SelectedBlock {
        id: SelectedBlockId(identifier),
        origin: crate::SelectedBlockOrigin::Source(
            semantic_vocabulary::BlockId::new(u64::from(identifier) + 1).unwrap(),
        ),
        instructions: Vec::new(),
        terminator: crate::SelectedTerminator::Return {
            instruction: crate::SelectedInstruction {
                id: crate::SelectedInstructionId(identifier),
                kind: crate::SelectedInstructionKind::ReturnScalar,
                constraint: register_model::RegisterConstraintKey {
                    family: register_model::RegisterConstraintFamily::Instruction,
                    variant: identifier,
                },
                operands: Vec::new(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: Default::default(),
            },
            psi_return_edge: semantic_vocabulary::EdgeId::new(u64::from(identifier) + 1).unwrap(),
        },
    }
}
fn blocked() -> SelectedInstructionPlan {
    let mut function = function(1);
    function.blocks = vec![block(0), block(4)].into();
    plan(vec![function])
}
fn fresh_identity(source: &SelectedInstructionPlan) -> crate::SelectedInstructionPlanIdentity {
    let functions = source
        .functions
        .iter()
        .map(|function| {
            let mut fresh = function.clone();
            fresh.blocks = function.blocks.iter().cloned().collect();
            fresh
        })
        .collect();
    selected_instruction_plan_identity(&plan(functions))
}
#[test]
fn a_block_edit_detaches_only_its_contents_and_rehashes_the_plan() {
    let source = blocked();
    let original = selected_instruction_plan_identity(&source);
    let mut changed = source.clone();
    changed.functions[0].blocks[1].id = SelectedBlockId(9);
    let changed_identity = selected_instruction_plan_identity(&changed);
    assert_ne!(original, changed_identity);
    assert_eq!(changed_identity, fresh_identity(&changed));
    assert!(
        changed.functions[0]
            .blocks
            .shares_block_storage(&source.functions[0].blocks, 0)
    );
    assert!(
        !changed.functions[0]
            .blocks
            .shares_block_storage(&source.functions[0].blocks, 1)
    );
    assert_eq!(source.functions[0].blocks[1].id, SelectedBlockId(4));
    assert_eq!(selected_instruction_plan_identity(&source), original);

    let mut by_identity = source.clone();
    by_identity.functions[0]
        .block_mut(SelectedBlockId(4))
        .unwrap()
        .id = SelectedBlockId(9);
    assert_eq!(
        selected_instruction_plan_identity(&by_identity),
        changed_identity
    );
    assert!(
        by_identity.functions[0]
            .blocks
            .shares_block_storage(&source.functions[0].blocks, 0)
    );
}
#[test]
fn every_mutable_block_route_invalidates_shared_and_unique_digest_memos() {
    let source = blocked();
    let original = selected_instruction_plan_identity(&source);
    for route in 0..4 {
        let mut changed = source.clone();
        let blocks = &mut changed.functions[0].blocks;
        match route {
            0 => blocks[1].id = SelectedBlockId(9),
            1 => blocks.get_mut(1).unwrap().id = SelectedBlockId(9),
            2 => blocks.iter_mut().nth(1).unwrap().id = SelectedBlockId(9),
            _ => {
                for block in blocks {
                    if block.id == SelectedBlockId(4) {
                        block.id = SelectedBlockId(9);
                    }
                }
            }
        }
        assert_ne!(selected_instruction_plan_identity(&changed), original);
        assert_eq!(
            selected_instruction_plan_identity(&changed),
            fresh_identity(&changed)
        );
        // This second edit has no shared block payload left to force a detach.
        changed.functions[0].blocks[1].id = SelectedBlockId(4);
        assert_eq!(selected_instruction_plan_identity(&changed), original);
    }
}
#[test]
fn rebuilding_a_block_roster_preserves_content_identity_and_unchanged_pages() {
    let source = blocked();
    let original = selected_instruction_plan_identity(&source);
    let mut changed = source.clone();
    changed.functions[0].blocks.truncate(1);
    changed.functions[0].blocks.push(block(4));
    assert_eq!(changed.functions[0].blocks, source.functions[0].blocks);
    assert!(
        changed.functions[0]
            .blocks
            .shares_block_storage(&source.functions[0].blocks, 0)
    );
    assert_eq!(selected_instruction_plan_identity(&changed), original);
    changed.functions[0].blocks.push(block(8));
    assert_ne!(selected_instruction_plan_identity(&changed), original);
    assert_eq!(
        selected_instruction_plan_identity(&changed),
        fresh_identity(&changed)
    );
}

#[test]
fn mechanical_block_lookup_tracks_mutation_without_changing_content_identity() {
    let mut source = blocked();
    let mut instruction =
        crate::terminator_instruction(&source.functions[0].blocks[0].terminator).clone();
    instruction.id = crate::SelectedInstructionId(19);
    instruction.operands.push(crate::SelectedOperand {
        operand: 0,
        virtual_register: crate::VirtualRegisterId(7),
        class: register_model::RegisterClassId(0),
        access: register_model::RegisterOperandAccess::Def,
        fixed_view: Some(register_model::RegisterViewId(1)),
        tied_to: None,
        early_clobber: false,
    });
    source.functions[0].blocks[0].instructions.push(instruction);
    let identity = selected_instruction_plan_identity(&source);
    assert!(
        source.functions[0]
            .blocks
            .has_fixed_definition(crate::VirtualRegisterId(7))
    );
    assert_eq!(
        source.functions[0].blocks.maximum_instruction_id(),
        Some(crate::SelectedInstructionId(19))
    );
    assert_eq!(selected_instruction_plan_identity(&source), identity);
    let mut changed = source.clone();
    let instruction = &mut changed.functions[0].blocks[0].instructions[0];
    instruction.id = crate::SelectedInstructionId(3);
    instruction.operands[0].fixed_view = None;
    assert!(
        !changed.functions[0]
            .blocks
            .has_fixed_definition(crate::VirtualRegisterId(7))
    );
    assert_eq!(
        changed.functions[0].blocks.maximum_instruction_id(),
        Some(crate::SelectedInstructionId(4))
    );
    assert!(
        source.functions[0]
            .blocks
            .has_fixed_definition(crate::VirtualRegisterId(7))
    );
    assert_eq!(
        selected_instruction_plan_identity(&changed),
        fresh_identity(&changed)
    );
}
