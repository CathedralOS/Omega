//! Block ids follow the scalar graph's layout with identity base 0: authored
//! state `n` is block `n + 1`, and every later block is private.

use semantic_vocabulary::{
    BlockId, EdgeId, OperationId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, Terminator, ValueDeclaration,
};

fn forward(blocks: &mut [Block], authored_states: usize, next_place: &mut u64) -> bool {
    super::forward_private_state_borrows(
        blocks,
        0,
        authored_states,
        &[StructuralTypeDeclaration {
            id: StructuralTypeId::new(1).unwrap(),
            identity: "Main".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        next_place,
    )
    .is_ok()
}

fn place(identity: u64) -> PlaceId {
    PlaceId::new(identity).unwrap()
}

fn loan(identity: u64) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(identity),
        position: 0,
        is_self: false,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn argument(identity: u64) -> StructuralArgument {
    StructuralArgument {
        place: place(identity),
        path: Vec::new(),
        access: StructuralAccess::MutableBorrow,
    }
}

fn jump(identity: u64, target: u64, structural_arguments: Vec<StructuralArgument>) -> Block {
    Block {
        id: BlockId::new(identity).unwrap(),
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Jump {
            edge: EdgeId::new(identity).unwrap(),
            target: BlockId::new(target).unwrap(),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments,
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    }
}

fn branch(identity: u64, when_true: u64, when_false: u64) -> Block {
    let edge = |offset: u64, target: u64| SuccessorEdge {
        edge: EdgeId::new(identity * 10 + offset).unwrap(),
        target: BlockId::new(target).unwrap(),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    Block {
        terminator: Terminator::Conditional {
            condition: ValueId::new(identity).unwrap(),
            when_true: edge(1, when_true),
            when_false: edge(2, when_false),
        },
        ..jump(identity, when_true, Vec::new())
    }
}

fn completion(identity: u64) -> Block {
    Block {
        terminator: Terminator::ReturnUnit {
            edge: EdgeId::new(identity).unwrap(),
            trivial_affine_discards: Vec::new(),
        },
        ..jump(identity, identity, Vec::new())
    }
}

/// A scalar exit-arm read of one field of `source`.
fn read(block: &mut Block, source: u64) {
    block.operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(block.id.get()).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(block.id.get() + 100).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        kind: OperationKind::BooleanStructuralField {
            source: place(source),
            path: Vec::new(),
            field: StructuralFieldId::new(1).unwrap(),
        },
    });
}

fn read_source(block: &Block) -> PlaceId {
    let OperationKind::BooleanStructuralField { source, .. } = block.operations[0].kind else {
        panic!("read");
    };
    source
}

/// `read_at`'s last state: the entry forwards its loan (place 1) into state
/// `third` (block 2, place 2), whose exit arms (blocks 4 and 5) read it and
/// jump to the shared return completion (block 3).
fn last_state_exit_arms() -> Vec<Block> {
    let mut third = branch(2, 4, 5);
    third.structural_parameters.push(loan(2));
    let mut true_arm = jump(4, 3, Vec::new());
    read(&mut true_arm, 2);
    let mut false_arm = jump(5, 3, Vec::new());
    read(&mut false_arm, 2);
    vec![
        jump(1, 2, vec![argument(1)]),
        third,
        completion(3),
        true_arm,
        false_arm,
    ]
}

#[test]
fn last_state_exit_arms_read_the_loan_by_their_own_forwarded_names() {
    let mut blocks = last_state_exit_arms();
    let mut next_place = 10;
    assert!(forward(&mut blocks, 2, &mut next_place));
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &blocks[1].terminator
    else {
        panic!("conditional");
    };
    assert_eq!(when_true.structural_arguments, vec![argument(2)]);
    assert_eq!(when_false.structural_arguments, vec![argument(2)]);
    let mut forwarded = Vec::new();
    for arm in &blocks[3..5] {
        let [parameter] = arm.structural_parameters.as_slice() else {
            panic!("each arm binds the loan once");
        };
        assert_eq!(parameter.access, StructuralAccess::MutableBorrow);
        assert_eq!(read_source(arm), parameter.place);
        forwarded.push(parameter.place);
    }
    assert_ne!(forwarded[0], forwarded[1]);
    assert!(forwarded.iter().all(|place| place.get() >= 10));
    assert_eq!(next_place, 12);
    // The entry's machine-scope loan and the completion that reads nothing
    // keep their shapes.
    assert_eq!(blocks[0], last_state_exit_arms()[0]);
    assert_eq!(blocks[2], last_state_exit_arms()[2]);
}

#[test]
fn a_state_that_transfers_its_loan_onward_keeps_its_arms_unchanged() {
    // State `second` (block 2) reads its loan in one arm and transfers it to
    // state `third` (block 3) through a private completion row (block 6).
    // Terminal keeps an explicitly transferred name live until the transfer
    // consumes it, so no second loan is invented beside it.
    let mut second = branch(2, 5, 6);
    second.structural_parameters.push(loan(2));
    let mut third = completion(3);
    third.structural_parameters.push(loan(3));
    let mut read_arm = jump(5, 4, Vec::new());
    read(&mut read_arm, 2);
    let mut blocks = vec![
        jump(1, 2, vec![argument(1)]),
        second,
        third,
        completion(4),
        read_arm,
        jump(6, 3, vec![argument(2)]),
    ];
    let expected = blocks.clone();
    let mut next_place = 10;
    assert!(forward(&mut blocks, 3, &mut next_place));
    assert_eq!(blocks, expected);
    assert_eq!(next_place, 10);
}

#[test]
fn a_private_block_shared_with_another_state_refuses_one_states_loan() {
    // Both the entry and state `third` reach private block 3, which reads
    // `third`'s loan. The entry has no such loan to supply on its edge.
    let mut third = jump(2, 3, Vec::new());
    third.structural_parameters.push(loan(2));
    let mut shared = completion(3);
    read(&mut shared, 2);
    let mut blocks = vec![branch(1, 2, 3), third, shared];
    blocks[0].terminator = match blocks[0].terminator.clone() {
        Terminator::Conditional {
            condition,
            mut when_true,
            when_false,
        } => {
            when_true.structural_arguments.push(argument(1));
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            }
        }
        _ => unreachable!(),
    };
    assert!(!forward(&mut blocks, 2, &mut 10));
}
