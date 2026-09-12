use super::*;
use semantic_vocabulary::{DomainSemanticId, ScalarDomainId, ScalarQualificationSetId};
use terminal_psi::{ScalarDomainDeclaration, ScalarQualificationCoercion, ScalarQualificationSet};

fn value(raw: u64, set: u64) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(raw).unwrap(),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        qualifications: ScalarQualificationSetId::new(set),
    }
}

fn module() -> TerminalModule {
    let mut module = unit_module();
    let machine = &mut module.machines[0];
    machine.parameters = vec![value(1, 0)];
    machine.result = TerminalMachineResult::Scalar(value(4, 1));
    machine.blocks[0].terminator = Terminator::Jump {
        edge: EdgeId::new(1).unwrap(),
        target: BlockId::new(2).unwrap(),
        arguments: vec![value(1, 0).id],
        structural_arguments: vec![],
        trivial_affine_discards: vec![],
        residual_affine_discards: vec![],
    };
    machine.blocks.push(Block {
        id: BlockId::new(2).unwrap(),
        parameters: vec![value(2, 1)],
        structural_parameters: vec![],
        operations: vec![],
        terminator: Terminator::Return {
            edge: EdgeId::new(2).unwrap(),
            value: value(2, 1).id,
            cleanup_actions: vec![],
        },
    });
    module.scalar_qualifications.domains = vec![ScalarDomainDeclaration {
        id: ScalarDomainId::new(1).unwrap(),
        semantic_domain: DomainSemanticId::new(1).unwrap(),
        identity: "test::Km<u64>".into(),
        carrier: value(1, 0).scalar_type,
    }];
    module.scalar_qualifications.sets = vec![ScalarQualificationSet {
        id: ScalarQualificationSetId::new(1),
        domains: vec![ScalarDomainId::new(1).unwrap()],
    }];
    module.scalar_qualifications.coercions = vec![ScalarQualificationCoercion {
        machine: machine.id,
        edge: EdgeId::new(1).unwrap(),
        argument_ordinal: 0,
        source: value(1, 0).id,
        destination: value(2, 1).id,
    }];
    module
}

#[test]
fn scalar_membership_introduction_and_return_verify_under_every_policy() {
    let module = module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    terminal_verifier::validate_module_for_interpretation(&module).unwrap();
    terminal_verifier::validate_module_for_optimization(&module).unwrap();
    terminal_verifier::validate_module_representation(&module).unwrap();
}

fn erasure_module() -> TerminalModule {
    let mut module = module();
    module.machines[0].parameters[0].qualifications = ScalarQualificationSetId::new(1);
    module.machines[0].blocks[1].parameters[0].qualifications = ScalarQualificationSetId::ZERO;
    module.machines[0]
        .result
        .scalar_mut()
        .unwrap()
        .qualifications = ScalarQualificationSetId::ZERO;
    module
}

#[test]
fn scalar_membership_explicit_erasure_verifies_under_every_policy() {
    let module = erasure_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    terminal_verifier::validate_module_for_interpretation(&module).unwrap();
    terminal_verifier::validate_module_for_optimization(&module).unwrap();
    terminal_verifier::validate_module_representation(&module).unwrap();
}

#[test]
fn scalar_membership_erasure_rejects_implicit_stale_equal_and_orphan_edges() {
    let changes: &[fn(&mut TerminalModule)] = &[
        |module| module.scalar_qualifications.coercions.clear(),
        |module| {
            module
                .scalar_qualifications
                .coercions
                .push(module.scalar_qualifications.coercions[0])
        },
        |module| module.scalar_qualifications.coercions[0].edge = EdgeId::new(2).unwrap(),
        |module| module.scalar_qualifications.coercions[0].argument_ordinal = 1,
        |module| module.scalar_qualifications.coercions[0].source = value(2, 0).id,
        |module| module.scalar_qualifications.coercions[0].destination = value(4, 0).id,
        |module| module.scalar_qualifications.coercions[0].machine = MachineId::new(999).unwrap(),
        |module| module.machines[0].parameters[0].qualifications = ScalarQualificationSetId::ZERO,
        |module| module.machines[0].parameters[0].scalar_type = ScalarType::Boolean,
    ];
    for (index, change) in changes.iter().enumerate() {
        let mut module = erasure_module();
        change(&mut module);
        assert!(
            validate_module(&module).is_err(),
            "erasure mutation {index}"
        );
    }
}

#[test]
fn scalar_membership_partial_erasure_preserves_remaining_atoms_and_rejects_replacement() {
    let mut module = module();
    let mut second = module.scalar_qualifications.domains[0].clone();
    second.id = ScalarDomainId::new(2).unwrap();
    second.semantic_domain = DomainSemanticId::new(2).unwrap();
    second.identity = "test::Other<u64>".into();
    module.scalar_qualifications.domains.push(second);
    module
        .scalar_qualifications
        .sets
        .push(ScalarQualificationSet {
            id: ScalarQualificationSetId::new(2),
            domains: vec![
                ScalarDomainId::new(1).unwrap(),
                ScalarDomainId::new(2).unwrap(),
            ],
        });
    module.machines[0].parameters[0].qualifications = ScalarQualificationSetId::new(2);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    // Replacing Other by Km is neither strict addition nor strict removal.
    module.scalar_qualifications.sets[1].domains.remove(0);
    assert!(validate_module(&module).is_err());
}

#[test]
fn observing_a_qualified_boolean_preserves_its_membership() {
    let mut module = module();
    module.scalar_qualifications.domains[0].carrier = ScalarType::Boolean;
    let machine = &mut module.machines[0];
    machine.parameters[0].scalar_type = ScalarType::Boolean;
    machine.result.scalar_mut().unwrap().scalar_type = ScalarType::Boolean;
    machine.blocks[1].parameters[0].scalar_type = ScalarType::Boolean;
    let result = machine.blocks[1].parameters[0];
    let successor = |edge| terminal_psi::SuccessorEdge {
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(3).unwrap(),
        arguments: vec![result.id],
        structural_arguments: vec![],
        trivial_affine_discards: vec![],
    };
    machine.blocks[1].terminator = Terminator::Conditional {
        condition: result.id,
        when_true: successor(2),
        when_false: successor(3),
    };
    let mut destination = result;
    destination.id = ValueId::new(5).unwrap();
    machine.blocks.push(Block {
        id: BlockId::new(3).unwrap(),
        parameters: vec![destination],
        structural_parameters: vec![],
        operations: vec![],
        terminator: Terminator::Return {
            edge: EdgeId::new(4).unwrap(),
            value: destination.id,
            cleanup_actions: vec![],
        },
    });
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
}

#[test]
fn scalar_membership_rejects_missing_or_redirected_coercion_and_definitions() {
    let changes: &[fn(&mut TerminalModule)] = &[
        |module| module.scalar_qualifications.coercions.clear(),
        |module| {
            let row = module.scalar_qualifications.coercions[0];
            module.scalar_qualifications.coercions.push(row);
        },
        |module| module.scalar_qualifications.coercions[0].edge = EdgeId::new(2).unwrap(),
        |module| module.scalar_qualifications.coercions[0].argument_ordinal = 1,
        |module| module.scalar_qualifications.coercions[0].source = value(2, 1).id,
        |module| module.scalar_qualifications.coercions[0].destination = value(4, 1).id,
        |module| module.scalar_qualifications.coercions[0].machine = MachineId::new(999).unwrap(),
        |module| module.scalar_qualifications.sets.clear(),
        |module| module.scalar_qualifications.domains.clear(),
        |module| {
            module.scalar_qualifications.sets[0]
                .domains
                .push(ScalarDomainId::new(1).unwrap())
        },
        |module| module.scalar_qualifications.sets[0].domains[0] = ScalarDomainId::new(2).unwrap(),
        |module| module.scalar_qualifications.domains[0].carrier = ScalarType::Boolean,
        |module| {
            let row = module.scalar_qualifications.domains[0].clone();
            module.scalar_qualifications.domains.push(row);
        },
        |module| {
            let mut row = module.scalar_qualifications.domains[0].clone();
            row.id = ScalarDomainId::new(2).unwrap();
            module.scalar_qualifications.domains.push(row);
        },
        |module| {
            let mut row = module.scalar_qualifications.sets[0].clone();
            row.id = ScalarQualificationSetId::new(2);
            module.scalar_qualifications.sets.push(row);
        },
        |module| {
            module.machines[0]
                .result
                .scalar_mut()
                .unwrap()
                .qualifications = ScalarQualificationSetId::new(0)
        },
        |module| module.machines[0].parameters[0].qualifications = ScalarQualificationSetId::new(1),
        |module| {
            if let Terminator::Jump { arguments, .. } = &mut module.machines[0].blocks[0].terminator
            {
                arguments.clear();
            }
        },
    ];
    for (index, change) in changes.iter().enumerate() {
        let mut module = module();
        change(&mut module);
        assert!(validate_module(&module).is_err(), "mutation {index}");
    }
}

#[test]
fn scalar_membership_cannot_be_forged_on_an_operation_or_consumed_by_arithmetic() {
    let mut forged = module();
    forged.machines[0].blocks[1].operations.push(Operation {
        static_reach_binding: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(value(5, 1)),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(7),
        },
    });
    assert!(validate_module(&forged).is_err());
    forged.machines[0].blocks[1].operations[0] = Operation {
        static_reach_binding: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(value(5, 0)),
        kind: OperationKind::IntegerBitwiseNot {
            operand: value(2, 1).id,
        },
    };
    assert!(validate_module(&forged).is_err());
}

#[test]
fn scalar_membership_calls_transport_both_arguments_and_results() {
    let mut module = module();
    let mut callee = unit_module().machines.remove(0);
    callee.id = MachineId::new(901).unwrap();
    callee.contract.id = ContractId::new(901).unwrap();
    callee.entry = BlockId::new(901).unwrap();
    callee.parameters = vec![value(10, 1)];
    callee.result = TerminalMachineResult::Scalar(value(11, 1));
    callee.blocks[0].id = callee.entry;
    callee.blocks[0].terminator = Terminator::Return {
        edge: EdgeId::new(901).unwrap(),
        value: value(10, 1).id,
        cleanup_actions: vec![],
    };
    module.machines[0].blocks[1].operations.push(Operation {
        static_reach_binding: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(value(3, 1)),
        kind: OperationKind::Call {
            callee: callee.id,
            arguments: vec![value(2, 1).id],
            requirement_obligations: vec![],
            crash_continuations: vec![],
        },
    });
    module.machines[0].blocks[1].terminator = Terminator::Return {
        edge: EdgeId::new(2).unwrap(),
        value: value(3, 1).id,
        cleanup_actions: vec![],
    };
    module.machines.push(callee);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let mut changed = module.clone();
    changed.machines[0].blocks[1].operations[0]
        .result
        .scalar_mut()
        .unwrap()
        .qualifications = ScalarQualificationSetId::new(0);
    assert!(validate_module(&changed).is_err());
    if let OperationKind::Call { arguments, .. } =
        &mut module.machines[0].blocks[1].operations[0].kind
    {
        arguments[0] = value(1, 0).id;
    }
    assert!(validate_module(&module).is_err());
}

#[test]
fn scalar_membership_join_requires_qualification_on_every_arrival() {
    let mut module = module();
    let machine = &mut module.machines[0];
    machine.parameters.push(ValueDeclaration {
        id: ValueId::new(20).unwrap(),
        scalar_type: ScalarType::Boolean,
        qualifications: ScalarQualificationSetId::new(0),
    });
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: ValueId::new(20).unwrap(),
        when_true: SuccessorEdge {
            edge: EdgeId::new(1).unwrap(),
            target: BlockId::new(2).unwrap(),
            arguments: vec![value(1, 0).id],
            structural_arguments: vec![],
            trivial_affine_discards: vec![],
        },
        when_false: SuccessorEdge {
            edge: EdgeId::new(3).unwrap(),
            target: BlockId::new(2).unwrap(),
            arguments: vec![value(1, 0).id],
            structural_arguments: vec![],
            trivial_affine_discards: vec![],
        },
    };
    let mut second = module.scalar_qualifications.coercions[0];
    second.edge = EdgeId::new(3).unwrap();
    module.scalar_qualifications.coercions.push(second);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    module.scalar_qualifications.coercions.pop();
    assert!(validate_module(&module).is_err());
}
