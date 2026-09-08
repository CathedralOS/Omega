use super::*;

fn scalar_cycle() -> TerminalModule {
    let mut module = ranked_countdown();
    module.machines[0].ranked_scc = None;
    module.machines[0].blocks[2].operations[1].kind = OperationKind::WrappingIntegerSubtract {
        left: id(2, ValueId::new),
        right: id(5, ValueId::new),
    };
    module
}

#[test]
fn cyclic_scalar_definitions_and_transfers_use_ordinary_validation() {
    let mut module = scalar_cycle();
    validate_module(&module)
        .map(|_| ())
        .expect("dominating definitions and exact loop arguments");
    module.machines[0].blocks.reverse();
    validate_module(&module)
        .map(|_| ())
        .expect("block roster order does not select dominance");
}

#[test]
fn cyclic_scalar_first_arrival_cannot_use_latch_definition() {
    let mut module = scalar_cycle();
    module.machines[0].blocks[1].operations[1].kind = OperationKind::IntegerLessThan {
        left: id(3, ValueId::new),
        right: id(6, ValueId::new),
    };
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ValueUsedBeforeDefinition(id(6, ValueId::new)))
    );
}

#[test]
fn cyclic_scalar_definition_must_precede_same_block_use() {
    let mut module = scalar_cycle();
    module.machines[0].blocks[2].operations.swap(0, 1);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ValueUsedBeforeDefinition(id(5, ValueId::new)))
    );
}

#[test]
fn cyclic_scalar_sibling_definition_does_not_dominate_join() {
    let mut module = scalar_cycle();
    let machine = &mut module.machines[0];
    let scalar_type = machine.parameters[0].scalar_type;
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: id(20, ValueId::new),
        scalar_type,
    });
    // The exit is also reachable without ever visiting the decrement block.
    machine.blocks[3].terminator = Terminator::Return {
        edge: id(5, EdgeId::new),
        value: id(6, ValueId::new),
        cleanup_actions: Vec::new(),
    };
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ValueUsedBeforeDefinition(id(6, ValueId::new)))
    );
}

#[test]
fn multiple_entry_cycle_retains_every_predecessor_for_scalar_dominance() {
    let mut module = unranked_scalar_cycle();
    let machine = &mut module.machines[0];
    let mut left = machine.blocks[0].clone();
    left.id = id(2, BlockId::new);
    let mut right = left.clone();
    right.id = id(3, BlockId::new);
    for (block, true_target, false_target, first_edge) in [
        (&mut machine.blocks[0], 2, 3, 20),
        (&mut left, 3, 4, 30),
        (&mut right, 2, 4, 40),
    ] {
        let Terminator::Conditional {
            when_true,
            when_false,
            ..
        } = &mut block.terminator
        else {
            panic!("conditional fixture")
        };
        when_true.target = id(true_target, BlockId::new);
        when_false.target = id(false_target, BlockId::new);
        when_true.edge = id(first_edge, EdgeId::new);
        when_false.edge = id(first_edge + 1, EdgeId::new);
    }
    for (block, value) in [(&mut left, 20), (&mut right, 21)] {
        block.operations.push(Operation {
            id: id(value, OperationId::new),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(value, ValueId::new),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanNot {
                operand: id(10, ValueId::new),
            },
        });
    }
    machine.blocks.extend([left, right]);
    validate_module(&module).expect("multiple-entry cycle has no reducibility requirement");
    module.machines[0].blocks[3].operations[0].kind = OperationKind::BooleanNot {
        operand: id(20, ValueId::new),
    };
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ValueUsedBeforeDefinition(id(20, ValueId::new)))
    );
}

#[test]
fn every_cyclic_scalar_jump_checks_arity_and_type() {
    for block_position in [0, 2] {
        let mut module = scalar_cycle();
        let Terminator::Jump {
            arguments, edge, ..
        } = &mut module.machines[0].blocks[block_position].terminator
        else {
            panic!("preheader or backedge")
        };
        let edge = *edge;
        arguments.clear();
        assert_eq!(
            validate_module(&module).map(|_| ()),
            Err(ModuleError::JumpArityMismatch {
                edge,
                expected: 1,
                actual: 0
            })
        );
    }
    let mut module = scalar_cycle();
    let Terminator::Jump { arguments, .. } = &mut module.machines[0].blocks[2].terminator else {
        panic!("backedge")
    };
    arguments[0] = id(4, ValueId::new);
    assert!(matches!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::JumpTypeMismatch { .. })
    ));
}

#[test]
fn cyclic_conditional_arguments_bind_target_parameters_exactly() {
    let mut module = scalar_cycle();
    let scalar_type = module.machines[0].parameters[0].scalar_type;
    module.machines[0].blocks[2]
        .parameters
        .push(ValueDeclaration {
            id: id(20, ValueId::new),
            scalar_type,
        });
    let Terminator::Conditional { when_true, .. } = &mut module.machines[0].blocks[1].terminator
    else {
        panic!("header guard")
    };
    when_true.arguments.push(id(2, ValueId::new));
    validate_module(&module)
        .map(|_| ())
        .expect("exact conditional telescope");
    let Terminator::Conditional { when_true, .. } = &mut module.machines[0].blocks[1].terminator
    else {
        panic!("header guard")
    };
    when_true.arguments[0] = id(20, ValueId::new);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::ValueUsedBeforeDefinition(id(20, ValueId::new)))
    );
}

#[test]
fn cyclic_scalar_eligibility_does_not_waive_arithmetic_proofs() {
    let mut module = ranked_countdown();
    module.machines[0].ranked_scc = None;
    validate_module(&module)
        .map(|_| ())
        .expect("safe operand topology needs no progress claim");
    assert!(matches!(
        verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        ),
        Err(VerificationError::MissingEvidence(obligation)) if obligation == id(1, ObligationId::new)
    ));
}

#[test]
fn unranked_scalar_cycle_preserves_plain_owned_custody_but_not_linear_inputs() {
    let mut module = scalar_cycle();
    let owned_place = add_loop_preserved_affine_parameter(&mut module);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("plain affine input stays live around the loop and disposes on return");

    let mut missing_disposal = module.clone();
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut missing_disposal.machines[0].blocks[3].terminator
    else {
        panic!("normal return")
    };
    trivial_affine_discards.clear();
    assert!(matches!(
        validate_module(&missing_disposal),
        Err(ModuleError::UnitReturnAffineDiscardsMismatch { .. })
    ));

    module.machines[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Linear;
    module.machines[0]
        .entry_claims
        .push(terminal_psi::EntryClaim {
            claim: semantic_vocabulary::ClaimId::new(1).unwrap(),
            input: owned_place,
            path: Vec::new(),
        });
    let result = validate_module_representation(&module);
    assert!(
        matches!(result, Err(ModuleError::ControlCycle(_))),
        "linear input: {result:?}"
    );
}

#[test]
fn cyclic_scalar_targets_and_reachability_are_checked_before_dominance() {
    let mut module = scalar_cycle();
    let Terminator::Jump { target, .. } = &mut module.machines[0].blocks[2].terminator else {
        panic!("backedge")
    };
    *target = id(99, BlockId::new);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::UnknownTargetBlock(id(99, BlockId::new)))
    );

    let mut module = scalar_cycle();
    let mut unreachable = module.machines[0].blocks[3].clone();
    unreachable.id = id(99, BlockId::new);
    unreachable.terminator = Terminator::ReturnUnit {
        edge: id(99, EdgeId::new),
        trivial_affine_discards: Vec::new(),
    };
    module.machines[0].blocks.push(unreachable);
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::UnreachableBlock(id(99, BlockId::new)))
    );
}
