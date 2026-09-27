use super::super::super::{
    Operation, PlaceId, PrimitiveType, StructuralTypeId, SuccessorEdge, place_id,
    terminal_scalar_type,
};
use super::super::{
    MachineId, OperationResult, StructuralArgument, ValueDeclaration, block_id, edge_id, value_id,
};
use super::{
    Block, LoweringError, OperationKind, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, TerminalAffineCleanupAction, Terminator, complete,
};
use semantic_vocabulary::OperationId;

#[test]
fn parameter_completion_preserves_disjoint_temporary_and_local_cleanup() {
    let parameters = [parameter(91, 0), parameter(13, 1)];
    let temporary = place_id(101);
    let local = place_id(102);
    let mut blocks = vec![jump(1, 2), returning(2)];
    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &mut blocks[0].terminator
    else {
        panic!("jump");
    };
    trivial_affine_discards.push(temporary);
    let Terminator::Return {
        cleanup_actions, ..
    } = &mut blocks[1].terminator
    else {
        panic!("return");
    };
    cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(local));
    complete(&parameters, block_id(1), &mut blocks).expect("disjoint cleanup composition");
    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &blocks[0].terminator
    else {
        panic!("jump");
    };
    assert_eq!(trivial_affine_discards, &[temporary]);
    assert_eq!(
        cleanup(&blocks[1]),
        &[
            TerminalAffineCleanupAction::DiscardRoot(local),
            TerminalAffineCleanupAction::DiscardRoot(parameters[1].place),
            TerminalAffineCleanupAction::DiscardRoot(parameters[0].place),
        ]
    );
    let mut overlapping = vec![jump(1, 2), returning(2)];
    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &mut overlapping[0].terminator
    else {
        panic!("jump");
    };
    trivial_affine_discards.push(parameters[0].place);
    assert!(complete(&parameters, block_id(1), &mut overlapping).is_err());
}

fn parameter(place: u64, position: u32) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: PlaceId::new(place).unwrap(),
        position,
        is_self: false,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn block(identity: u64, terminator: Terminator) -> Block {
    Block {
        id: block_id(identity),
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator,
    }
}

fn returning(identity: u64) -> Block {
    block(
        identity,
        Terminator::Return {
            edge: edge_id(identity),
            value: value_id(1),
            cleanup_actions: Vec::new(),
        },
    )
}

fn jump(identity: u64, target: u64) -> Block {
    block(
        identity,
        Terminator::Jump {
            edge: edge_id(identity),
            target: block_id(target),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    )
}

fn successor(identity: u64, target: u64) -> SuccessorEdge {
    SuccessorEdge {
        edge: edge_id(identity),
        target: block_id(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn transfer(place: PlaceId) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(2),
            scalar_type: terminal_scalar_type(PrimitiveType::U64).unwrap(),
        }),
        kind: OperationKind::CallStructuralScalar {
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            callee: MachineId::new(2).unwrap(),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }
}

fn cleanup(block: &Block) -> &[TerminalAffineCleanupAction] {
    let Terminator::Return {
        cleanup_actions, ..
    } = &block.terminator
    else {
        panic!("scalar return")
    };
    cleanup_actions
}

fn owned_backedge(identity: u64, parameters: &[StructuralParameterDeclaration]) -> Block {
    let mut backedge = jump(identity, 10);
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut backedge.terminator
    else {
        panic!("jump")
    };
    *structural_arguments = parameters
        .iter()
        .map(|parameter| StructuralArgument {
            place: parameter.place,
            path: Vec::new(),
            access: parameter.access,
        })
        .collect();
    backedge
}

fn owned_loop(parameters: &[StructuralParameterDeclaration]) -> Vec<Block> {
    let mut header = block(
        10,
        Terminator::Conditional {
            condition: value_id(1),
            when_true: successor(10, 20),
            when_false: successor(11, 90),
        },
    );
    header.structural_parameters = parameters.to_vec();
    // Storage order must not decide traversal or disposal order.
    vec![returning(90), owned_backedge(20, parameters), header]
}

#[test]
fn cut_header_loop_rejects_missing_affine_backedge_actuals() {
    let parameters = [parameter(91, 0), parameter(13, 1)];
    let mut blocks = owned_loop(&parameters);
    complete(&parameters, block_id(10), &mut blocks.clone()).expect("valid control");
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut blocks[1].terminator
    else {
        panic!("backedge")
    };
    structural_arguments.pop();
    let error =
        complete(&parameters, block_id(10), &mut blocks).expect_err("missing affine actual");
    assert!(matches!(error, LoweringError::Unsupported(message)
        if message == "owned backedge lost its complete structural argument roster"));
}

#[test]
fn cut_header_loop_join_cannot_restore_an_owner_consumed_on_one_branch() {
    let parameters = [parameter(91, 0), parameter(13, 1)];
    let mut blocks = owned_loop(&parameters);
    blocks[1] = block(
        20,
        Terminator::Conditional {
            condition: value_id(1),
            when_true: successor(20, 30),
            when_false: successor(21, 40),
        },
    );
    blocks.extend([jump(30, 50), jump(40, 50), owned_backedge(50, &parameters)]);
    complete(&parameters, block_id(10), &mut blocks.clone())
        .expect("both branches preserve the loop frontier");
    // The untouched branch still owns both roots. Their join must retain only
    // the intersection, not the header's initial complete frontier.
    blocks
        .iter_mut()
        .find(|block| block.id == block_id(30))
        .unwrap()
        .operations
        .push(transfer(parameters[0].place));
    let error =
        complete(&parameters, block_id(10), &mut blocks).expect_err("join cannot revive ownership");
    assert!(matches!(error, LoweringError::Unsupported(message)
        if message == "owned backedge transfers a missing or already consumed owner"));
}

#[test]
fn cut_header_loop_does_not_hide_a_nonheader_cycle() {
    let parameters = [parameter(91, 0), parameter(13, 1)];
    let mut blocks = owned_loop(&parameters);
    let Terminator::Conditional { when_true, .. } = &mut blocks[2].terminator else {
        panic!("header")
    };
    when_true.target = block_id(40);
    blocks.push(block(
        40,
        Terminator::Conditional {
            condition: value_id(1),
            when_true: successor(40, 20),
            when_false: successor(41, 20),
        },
    ));
    complete(&parameters, block_id(10), &mut blocks.clone())
        .expect("acyclic body with parallel arrivals");
    let Terminator::Conditional { when_false, .. } = &mut blocks[3].terminator else {
        panic!("inner branch")
    };
    when_false.target = block_id(40);
    let error =
        complete(&parameters, block_id(10), &mut blocks).expect_err("only header edges may be cut");
    assert!(matches!(error, LoweringError::Unsupported(message)
        if message == "owned scalar graph has unreachable or cyclic custody requiring structural forwarding evidence"));
}

#[test]
fn duplicate_transfer_and_transfer_after_join_disposal_reject() {
    let parameter = parameter(1, 0);
    let mut duplicate = vec![returning(1)];
    duplicate[0].operations = vec![transfer(parameter.place), transfer(parameter.place)];
    assert!(
        complete(
            std::slice::from_ref(&parameter),
            block_id(1),
            &mut duplicate
        )
        .is_err()
    );
    let mut joined = vec![
        block(
            1,
            Terminator::Conditional {
                condition: value_id(1),
                when_true: successor(1, 2),
                when_false: successor(2, 3),
            },
        ),
        jump(2, 4),
        jump(3, 4),
        returning(4),
    ];
    joined[1].operations.push(transfer(parameter.place));
    joined[3].operations.push(transfer(parameter.place));
    assert!(complete(std::slice::from_ref(&parameter), block_id(1), &mut joined).is_err());
}

#[test]
fn cycles_and_preexisting_cleanup_require_their_own_custody() {
    let parameter = parameter(1, 0);
    assert!(
        complete(
            std::slice::from_ref(&parameter),
            block_id(1),
            &mut [jump(1, 1)]
        )
        .is_err()
    );
    let mut block = returning(1);
    let Terminator::Return {
        cleanup_actions, ..
    } = &mut block.terminator
    else {
        panic!("return")
    };
    cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(parameter.place));
    assert!(complete(std::slice::from_ref(&parameter), block_id(1), &mut [block]).is_err());
}
