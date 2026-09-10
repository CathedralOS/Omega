use super::*;

#[test]
fn raw_non_utf8_and_nul_octets_need_only_the_capacity_proof() {
    let (mut module, bundle) = fixture(StructuralAccess::WriteOnlyBorrow);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *bytes = vec![0xff, 0, 0x80];
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("an unqualified raw byte field imposes no UTF-8 predicate");
}

#[test]
fn immutable_subslice_source_retains_both_bounds_and_store_capacity() {
    let (mut module, bundle) = fixture(StructuralAccess::MutableBorrow);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(5),
            structural_type: id(2),
        },
    });
    machine.blocks[0].operations.insert(
        3,
        Operation {
            id: id(5),
            result: OperationResult::Structural(StructuralOperationResult {
                place: id(3),
                structural_type: id(2),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::ByteSequenceSubslice {
                source: id(2),
                start: id(2),
                end: id(1),
                length: id(1),
                obligation: id(2),
            },
        },
    );
    machine.blocks[0].operations.insert(
        4,
        Operation {
            id: id(6),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(3),
                scalar_type: ScalarType::Integer(integer()),
            }),
            kind: OperationKind::ByteSequenceLength { source: id(3) },
        },
    );
    let OperationKind::StructuralByteSequenceFieldStore { source, length, .. } =
        &mut machine.blocks[0].operations[5].kind
    else {
        unreachable!()
    };
    *source = id(3);
    *length = id(3);
    validate_module(&module).unwrap();
    let obligations = reconstruct_operation_obligations(&module).unwrap();
    assert_eq!(obligations.len(), 2);
    assert!(
        matches!(obligations[0].obligation.proposition, Proposition::Conjunction(ref legs) if legs.len() == 2)
    );
    assert_eq!(
        obligations[1].obligation.proposition,
        Proposition::LessOrEqual(
            ScalarTerm::value(id(3), ScalarType::Integer(integer())),
            count(3)
        )
    );
    assert!(
        verify_module(&module, &bundle, &AdmissionProfile::default()).is_err(),
        "the original store certificate does not discharge the new subslice obligations"
    );
}

#[test]
fn immutable_block_source_keeps_exact_binding_and_capacity_question() {
    let (mut module, bundle) = fixture(StructuralAccess::MutableBorrow);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let machine = &mut module.machines[0];
    machine.structural_places[1].kind = StructuralPlaceKind::Parameter {
        position: 1,
        is_self: false,
    };
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: id(2),
            position: 1,
            is_self: false,
            structural_type: id(2),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3),
        kind: StructuralPlaceKind::BlockParameter {
            block: id(901),
            position: 0,
        },
    });
    let mut operations = machine.blocks[0].operations.split_off(2);
    machine.blocks[0].operations.remove(1);
    operations[0].kind = OperationKind::ByteSequenceLength { source: id(3) };
    let OperationKind::StructuralByteSequenceFieldStore { source, .. } = &mut operations[1].kind
    else {
        unreachable!()
    };
    *source = id(3);
    let terminator = machine.blocks[0].terminator.clone();
    machine.blocks[0].terminator = Terminator::Jump {
        structural_arguments: vec![StructuralArgument {
            place: id(2),
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        }],
        edge: id(901),
        target: id(901),
        arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks.push(Block {
        structural_parameters: vec![StructuralParameterDeclaration {
            place: id(3),
            position: 0,
            is_self: false,
            structural_type: id(2),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        id: id(901),
        parameters: Vec::new(),
        operations,
        terminator,
    });
    validate_module(&module).unwrap();
    let obligations = reconstruct_operation_obligations(&module).unwrap();
    assert_eq!(obligations.len(), 1);
    assert_eq!(
        obligations[0].obligation.proposition,
        Proposition::LessOrEqual(measured(), count(3))
    );
    module.machines[0].blocks[1].operations[0].kind =
        OperationKind::ByteSequenceLength { source: id(2) };
    assert!(
        validate_module(&module).is_err(),
        "equal runtime contents cannot substitute a different measured place"
    );
}

#[test]
fn cyclic_literal_store_requires_establishment_before_each_iteration_read() {
    let (mut module, bundle) = fixture(StructuralAccess::MutableBorrow);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let machine = &mut module.machines[0];
    machine.attachment = Some(id(1));
    machine.structural_parameters[0].is_self = true;
    machine.structural_places[0].kind = StructuralPlaceKind::Parameter {
        position: 0,
        is_self: true,
    };
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(3),
        scalar_type: ScalarType::Boolean,
    });
    let operations = std::mem::take(&mut machine.blocks[0].operations);
    let terminator = machine.blocks[0].terminator.clone();
    machine.blocks[0].terminator = Terminator::Jump {
        structural_arguments: Vec::new(),
        edge: id(901),
        target: id(901),
        arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks.push(Block {
        structural_parameters: Vec::new(),
        id: id(901),
        parameters: Vec::new(),
        operations,
        terminator: Terminator::Conditional {
            condition: id(3),
            when_true: SuccessorEdge {
                structural_arguments: Vec::new(),
                edge: id(902),
                target: id(901),
                arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
            when_false: SuccessorEdge {
                structural_arguments: Vec::new(),
                edge: id(903),
                target: id(902),
                arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        },
    });
    machine.blocks.push(Block {
        structural_parameters: Vec::new(),
        id: id(902),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator,
    });
    terminal_verifier::verify_module_for_interpretation(
        &module,
        &bundle,
        &AdmissionProfile::default(),
    )
    .expect("each iteration establishes, measures and proves the same literal store");
    module.machines[0].blocks[1].operations.swap(1, 2);
    assert!(
        terminal_verifier::validate_module_for_interpretation(&module).is_err(),
        "a prior iteration cannot authorize a read before this iteration's establishment"
    );
}
