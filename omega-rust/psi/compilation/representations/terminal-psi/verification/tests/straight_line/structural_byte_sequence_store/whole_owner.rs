use super::*;
use terminal_psi::ByteSequencePredicate;
use terminal_verifier::ModuleError;

fn qualified_owner_fixture(access: StructuralAccess) -> (TerminalModule, ProofBundle) {
    let (mut module, bundle) = fixture(access);
    module.structural_types[0].shape =
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 3 });
    module.structural_domains.push(StructuralDomainDeclaration {
        id: id(1),
        semantic_domain: id(1),
        identity: "test::Utf8".into(),
        carrier: id(1),
        byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
        content_projection: None,
        establishment_routes: Vec::new(),
    });
    module.machines[0].structural_parameters[0]
        .qualifications
        .push(id(1));
    module.machines[0].blocks[0].operations[3].kind = OperationKind::ByteSequenceStore {
        destination: id(1),
        source: id(2),
        length: id(1),
        obligation: id(1),
    };
    (module, bundle)
}

#[test]
fn qualified_whole_owner_replacement_verifies_live_length_and_capacity() {
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let (module, bundle) = qualified_owner_fixture(access);
        let obligations = reconstruct_operation_obligations(&module).unwrap();
        assert_eq!(obligations.len(), 1);
        assert_eq!(
            obligations[0].obligation.proposition,
            Proposition::LessOrEqual(measured(), count(3))
        );
        assert!(obligations[0].canonical_certificate);
        verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    }
}

#[test]
fn whole_owner_replacement_rejects_previous_sources_length() {
    let (mut module, _) = qualified_owner_fixture(StructuralAccess::MutableBorrow);
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3),
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 1,
            structural_type: id(2),
        },
    });
    let operations = &mut machine.blocks[0].operations;
    let mut store = operations.pop().unwrap();
    operations.extend([
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(4),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: id(3),
                bytes: b"Y".to_vec(),
                qualifications: Vec::new(),
            },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(5),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(3),
                scalar_type: ScalarType::Integer(integer()),
            }),
            kind: OperationKind::ByteSequenceLength { source: id(3) },
        },
    ]);
    store.id = id(6);
    store.kind = OperationKind::ByteSequenceStore {
        destination: id(1),
        source: id(3),
        length: id(3),
        obligation: id(1),
    };
    operations.push(store);
    validate_module(&module).expect("replacement's own fresh source length is valid");
    let OperationKind::ByteSequenceStore { length, .. } = &mut module.machines[0].blocks[0]
        .operations
        .last_mut()
        .unwrap()
        .kind
    else {
        unreachable!()
    };
    *length = id(1);
    assert_eq!(
        validate_module(&module).err(),
        Some(ModuleError::InvalidStructuralByteSequenceFieldStore(id(6)))
    );
}

#[test]
fn whole_owner_replacement_reads_owned_or_shared_but_not_exclusive_loans() {
    let (mut module, _) = qualified_owner_fixture(StructuralAccess::MutableBorrow);
    let machine = &mut module.machines[0];
    let mut source = machine.structural_parameters[0].clone();
    source.place = id(3);
    source.position = 1;
    source.access = StructuralAccess::SharedBorrow;
    machine.structural_parameters.push(source);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    machine.blocks[0].operations[2].kind = OperationKind::ByteSequenceLength { source: id(3) };
    machine.blocks[0].operations[3].kind = OperationKind::ByteSequenceStore {
        destination: id(1),
        source: id(3),
        length: id(1),
        obligation: id(1),
    };
    validate_module(&module).expect("qualified shared owner may supply immutable bytes");
    module.machines[0].structural_parameters[1].access = StructuralAccess::Owned;
    validate_module(&module).expect("qualified owned parameter may supply its own bytes");
    let obligations = reconstruct_operation_obligations(&module).unwrap();
    assert_eq!(obligations.len(), 1);
    assert!(
        obligations[0]
            .semantic_axioms
            .contains(&Proposition::LessOrEqual(measured(), count(3)))
    );
    assert!(
        !obligations[0]
            .semantic_axioms
            .contains(&Proposition::Equal(measured(), count(3))),
        "an owned buffer's capacity is not its live length"
    );
    module.machines[0].structural_parameters[1].access = StructuralAccess::WriteOnlyBorrow;
    // Its metadata length remains readable; the payload copy must reject.
    assert_eq!(
        validate_module(&module).err(),
        Some(ModuleError::InvalidStructuralByteSequenceFieldStore(id(4)))
    );
    module.machines[0].structural_parameters[1].access = StructuralAccess::MutableBorrow;
    assert_eq!(
        validate_module(&module).err(),
        Some(ModuleError::InvalidStructuralByteSequenceFieldStore(id(4)))
    );
    module.machines[0].structural_parameters[1].access = StructuralAccess::Owned;
    module.machines[0].structural_parameters[1]
        .qualifications
        .clear();
    assert_eq!(
        validate_module(&module).err(),
        Some(ModuleError::InvalidStructuralByteSequenceFieldStore(id(4))),
        "ownership alone cannot establish the destination's UTF-8 predicate"
    );
}

#[test]
fn whole_owner_replacement_rechecks_source_predicate_after_equal_length_rewrite() {
    let (mut module, _) = qualified_owner_fixture(StructuralAccess::MutableBorrow);
    validate_module(&module).unwrap();
    let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    bytes[0] = 255;
    assert_eq!(
        validate_module(&module).err(),
        Some(ModuleError::InvalidStructuralByteSequenceFieldStore(id(4)))
    );
}
