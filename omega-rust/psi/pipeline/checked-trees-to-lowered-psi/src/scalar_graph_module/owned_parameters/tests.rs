use super::*;

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
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn transfer(place: PlaceId) -> Operation {
    Operation {
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: value_id(2),
            scalar_type: terminal_scalar_type(PrimitiveType::U64).unwrap(),
        }),
        kind: OperationKind::CallStructuralScalar {
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

#[test]
fn return_disposes_only_surviving_affine_parameters_in_reverse_declaration_order() {
    let first = parameter(91, 0);
    let second = parameter(13, 1);
    let mut shared = parameter(23, 2);
    shared.access = StructuralAccess::SharedBorrow;
    let mut unrestricted = parameter(44, 3);
    unrestricted.multiplicity = StructuralMultiplicity::Unrestricted;
    let parameters = [first.clone(), second.clone(), shared, unrestricted];
    let mut blocks = vec![returning(1)];
    complete(&parameters, block_id(1), &mut blocks).unwrap();
    assert_eq!(
        cleanup(&blocks[0]),
        &[
            TerminalAffineCleanupAction::DiscardRoot(second.place),
            TerminalAffineCleanupAction::DiscardRoot(first.place)
        ]
    );
    let mut blocks = vec![returning(1)];
    blocks[0].operations.push(transfer(second.place));
    complete(&parameters, block_id(1), &mut blocks).unwrap();
    assert_eq!(
        cleanup(&blocks[0]),
        &[TerminalAffineCleanupAction::DiscardRoot(first.place)]
    );
}

#[test]
fn conditional_join_disposes_the_untransferred_owner_on_its_own_edge() {
    let parameter = parameter(1, 0);
    // Deliberately store blocks out of traversal order.
    let mut blocks = vec![
        returning(4),
        jump(3, 4),
        block(
            1,
            Terminator::Conditional {
                condition: value_id(1),
                when_true: successor(1, 2),
                when_false: successor(2, 3),
            },
        ),
        jump(2, 4),
    ];
    blocks[3].operations.push(transfer(parameter.place));
    complete(std::slice::from_ref(&parameter), block_id(1), &mut blocks).unwrap();
    assert!(cleanup(&blocks[0]).is_empty());
    for (position, expected) in [(1, vec![parameter.place]), (3, Vec::new())] {
        let Terminator::Jump {
            trivial_affine_discards,
            ..
        } = &blocks[position].terminator
        else {
            panic!("jump")
        };
        assert_eq!(*trivial_affine_discards, expected);
    }
}

#[test]
fn conditional_edge_reconciliation_preserves_the_other_successor_frontier() {
    let parameter = parameter(1, 0);
    let mut blocks = vec![
        block(
            1,
            Terminator::Conditional {
                condition: value_id(1),
                when_true: successor(1, 2),
                when_false: successor(2, 3),
            },
        ),
        jump(2, 4),
        block(
            3,
            Terminator::Conditional {
                condition: value_id(1),
                when_true: successor(4, 4),
                when_false: successor(5, 5),
            },
        ),
        returning(4),
        returning(5),
    ];
    blocks[1].operations.push(transfer(parameter.place));
    complete(std::slice::from_ref(&parameter), block_id(1), &mut blocks).unwrap();
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &blocks[2].terminator
    else {
        panic!("conditional")
    };
    assert_eq!(when_true.trivial_affine_discards, vec![parameter.place]);
    assert!(when_false.trivial_affine_discards.is_empty());
    assert!(cleanup(&blocks[3]).is_empty());
    assert_eq!(
        cleanup(&blocks[4]),
        &[TerminalAffineCleanupAction::DiscardRoot(parameter.place)]
    );
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

#[test]
fn actual_affine_limits_artifact_rejects_missing_duplicate_and_transferred_cleanup_and_reads() {
    let source = "
        data Limits { limit: u64; }
        machine consume(limits: Limits) -> u64 { limits.limit }
        machine reset(value: &mut u64) -> u64 { value = 0; 0 }
        machine root(first: Limits, second: Limits) -> u64 {
            let mut scratch: u64 = 1;
            let consumed: u64 = consume(second);
            let cleared: u64 = reset(&mut scratch);
            first.limit
        }
    ";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolved");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).expect("checked");
    let artifact = terminal_production::produce_terminal_artifact(&checked, "root")
        .expect("publish affine scalar graph");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("module");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).expect("proof");
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile).expect("verified original");
    let root_position = module
        .machines
        .iter()
        .position(|machine| machine.id == module.entry)
        .unwrap();
    let first = module.machines[root_position].structural_parameters[0].place;
    let second = module.machines[root_position].structural_parameters[1].place;
    assert!(
        module.machines[root_position]
            .structural_parameters
            .iter()
            .all(|parameter| parameter.multiplicity == StructuralMultiplicity::Affine)
    );
    for forged in [
        Vec::new(),
        vec![TerminalAffineCleanupAction::DiscardRoot(first); 2],
        vec![
            TerminalAffineCleanupAction::DiscardRoot(second),
            TerminalAffineCleanupAction::DiscardRoot(first),
        ],
    ] {
        let mut changed = module.clone();
        let actions = changed.machines[root_position]
            .blocks
            .iter_mut()
            .find_map(|block| match &mut block.terminator {
                Terminator::Return {
                    cleanup_actions, ..
                } => Some(cleanup_actions),
                _ => None,
            })
            .expect("return cleanup");
        assert_eq!(
            *actions,
            vec![TerminalAffineCleanupAction::DiscardRoot(first)]
        );
        *actions = forged;
        assert!(terminal_verifier::verify_module(&changed, &proof, &profile).is_err());
    }
    let mut changed = module.clone();
    let source = changed.machines[root_position]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::IntegerStructuralField { source, .. } if *source == first => {
                Some(source)
            }
            _ => None,
        })
        .expect("surviving owner field read");
    *source = second;
    assert!(
        terminal_verifier::verify_module(&changed, &proof, &profile).is_err(),
        "read cannot revive the transferred owner"
    );
}
