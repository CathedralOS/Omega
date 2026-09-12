//! Raw ordering premises only; these fixtures do not manufacture selection authority.
use super::*;
use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
use selected_instructions::{
    SelectedBlockId, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedSuccessor,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId};

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
fn jump(source: u32, destination: u32) -> SelectedTerminator {
    SelectedTerminator::Jump {
        instruction: instruction(source, SelectedInstructionKind::Jump),
        successor: successor(destination),
    }
}
fn branch(source: u32, taken: u32, otherwise: u32) -> SelectedTerminator {
    SelectedTerminator::ConditionalBranch {
        instruction: instruction(source, SelectedInstructionKind::ConditionalBranchNonZero),
        when_nonzero: successor(taken),
        when_zero: successor(otherwise),
    }
}
fn returned(source: u32) -> SelectedTerminator {
    SelectedTerminator::Return {
        instruction: instruction(source, SelectedInstructionKind::ReturnUnit),
        psi_return_edge: EdgeId::new(u64::from(source) + 1).unwrap(),
    }
}
fn function(terminators: Vec<SelectedTerminator>) -> SelectedFunction {
    SelectedFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        provenance: Default::default(),
        structural: None,
        outgoing_arguments: Vec::new(),
        local_storage_slots: Vec::new(),
        calls: Vec::new(),
        memory_accesses: Vec::new(),
        boundary_settlements: Vec::new(),
        entry_block: SelectedBlockId(0),
        virtual_registers: Vec::new(),
        blocks: terminators
            .into_iter()
            .enumerate()
            .map(|(position, terminator)| SelectedBlock {
                id: SelectedBlockId(position as u32),
                origin: selected_instructions::SelectedBlockOrigin::Source(
                    BlockId::new(position as u64 + 1).unwrap(),
                ),
                instructions: Vec::new(),
                terminator,
            })
            .collect(),
    }
}
fn order(function: &SelectedFunction) -> Vec<u32> {
    derive(
        function,
        SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1,
    )
    .unwrap()
    .into_iter()
    .map(|block| block.id.0)
    .collect()
}
#[test]
fn nonentry_branch_and_explicit_backedge_use_ordinary_fallthrough_order() {
    let graph = function(vec![jump(0, 1), branch(1, 2, 3), jump(2, 1), returned(3)]);
    assert_eq!(order(&graph), vec![0, 1, 3, 2]);
}
#[test]
fn explicit_jump_cycles_do_not_impose_fallthrough_cycles() {
    assert_eq!(order(&function(vec![jump(0, 1), jump(1, 0)])), vec![0, 1]);
}
#[test]
fn existing_diamond_order_is_preserved() {
    let graph = function(vec![branch(0, 1, 2), jump(1, 3), jump(2, 3), returned(3)]);
    assert_eq!(order(&graph), vec![0, 2, 1, 3]);
}
#[test]
fn conflicting_or_cyclic_fallthrough_is_not_silently_reordered() {
    for graph in [
        function(vec![branch(0, 1, 2), branch(1, 0, 2), returned(2)]),
        function(vec![jump(0, 1), branch(1, 0, 2), branch(2, 0, 1)]),
    ] {
        assert!(
            derive(
                &graph,
                SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1
            )
            .is_err()
        );
    }
}

#[test]
fn sparse_unsorted_identities_preserve_storage_order_and_fallthrough() {
    let mut graph = function(vec![branch(0, 1, 2), returned(1), returned(2)]);
    graph.entry_block = SelectedBlockId(100);
    graph.blocks[0].id = SelectedBlockId(100);
    graph.blocks[1].id = SelectedBlockId(u32::MAX);
    graph.blocks[2].id = SelectedBlockId(7);
    graph.blocks[0].terminator = branch(0, u32::MAX, 7);
    assert_eq!(order(&graph), vec![100, 7, u32::MAX]);
}

#[test]
fn malformed_block_rosters_reject() {
    let mut duplicate = function(vec![returned(0), returned(1)]);
    duplicate.blocks[1].id = duplicate.blocks[0].id;
    let mut missing_entry = function(vec![returned(0)]);
    missing_entry.entry_block = SelectedBlockId(9);
    for graph in [
        function(Vec::new()),
        duplicate,
        missing_entry,
        function(vec![branch(0, 1, 9), returned(1)]),
        function(vec![branch(0, 1, 0), returned(1)]),
        function(vec![returned(0), branch(1, 0, 0)]),
    ] {
        assert!(
            derive(
                &graph,
                SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1
            )
            .is_err()
        );
    }
}

#[test]
fn many_disconnected_roots_and_reverse_fallthrough_chains_keep_canonical_order() {
    let count = 4096;
    let graph = function((0..count).map(returned).collect());
    assert_eq!(order(&graph), (0..count).collect::<Vec<_>>());

    let mut graph = function(
        (0..count)
            .map(|source| {
                if source <= 1 {
                    returned(source)
                } else {
                    branch(source, 0, source - 1)
                }
            })
            .collect(),
    );
    graph.entry_block = SelectedBlockId(count - 1);
    let expected: Vec<_> = (1..count).rev().chain(std::iter::once(0)).collect();
    assert_eq!(order(&graph), expected);
}
