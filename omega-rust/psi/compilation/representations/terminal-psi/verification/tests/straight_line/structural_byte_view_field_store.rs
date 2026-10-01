//! `StructuralByteViewFieldStore`: a shared view field takes a literal's
//! descriptor. The verifier admits only an established literal source, whose
//! image backing outlives every root, and replays the field's predicates from
//! the literal's octets. Every forged variation below must reject.
use super::{
    AdmissionProfile, IntegerSign, IntegerType, ModuleError, Operation, OperationKind,
    OperationResult, ProofBundle, ScalarType, StructuralAccess, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalModule, ValueDeclaration, unit_module, validate_module,
    verify_module,
};
use semantic_vocabulary::PsiSemanticId;
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, ByteSequencePredicate, ByteSequencePredicateSet,
};

fn id<T: PsiSemanticId>(raw: u64) -> T {
    T::new(raw).unwrap()
}

fn utf8() -> ByteSequencePredicateSet {
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    predicates.insert(ByteSequencePredicate::ValidUtf8);
    predicates
}

fn shared_view() -> ByteSequenceCarrier {
    ByteSequenceCarrier::BorrowedView {
        access: Some(StructuralAccess::SharedBorrow),
    }
}

fn store(destination: u64, source: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(2),
        result: OperationResult::Unit,
        kind: OperationKind::StructuralByteViewFieldStore {
            destination: id(destination),
            path: Vec::new(),
            field: id(1),
            source: id(source),
        },
    }
}

/// `place(1)`: an exclusive `Room { label: &[u8] in Utf8 }` parameter.
/// `place(2)`: an established literal `"Gate"`; `place(3)`: a shared view
/// parameter. The store replaces `label` with the literal.
fn fixture() -> TerminalModule {
    let mut module = unit_module();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(1),
            identity: "test::Room".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: id(1),
                    identity: "label".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::PredicateByteSequence {
                        carrier: shared_view(),
                        predicates: utf8(),
                    },
                }],
            },
        },
        StructuralTypeDeclaration {
            id: id(2),
            identity: "test::View".into(),
            shape: StructuralTypeShape::ByteSequence(shared_view()),
        },
    ];
    let machine = &mut module.machines[0];
    machine.structural_parameters = vec![
        StructuralParameterDeclaration {
            place: id(1),
            position: 0,
            is_self: false,
            structural_type: id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
        StructuralParameterDeclaration {
            place: id(3),
            position: 1,
            is_self: false,
            structural_type: id(2),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
    ];
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id(1),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: id(2),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 0,
                structural_type: id(2),
            },
        },
        StructuralPlaceDeclaration {
            id: id(3),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
    ];
    machine.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(1),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: id(2),
                bytes: b"Gate".to_vec(),
                qualifications: Vec::new(),
            },
        },
        store(1, 2),
    ];
    module
}

fn rejects(module: &TerminalModule) {
    assert!(
        matches!(
            validate_module(module),
            Err(ModuleError::InvalidStructuralByteViewFieldStore(operation)) if operation == id(2)
        ),
        "{:?}",
        validate_module(module)
    );
}

#[test]
fn literal_source_replaces_a_qualified_shared_view_field() {
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = fixture();
        module.machines[0].structural_parameters[0].access = access;
        validate_module(&module).unwrap();
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap();
    }
}

#[test]
fn a_parameter_view_has_no_lifetime_relation_to_the_destination_root() {
    let mut module = fixture();
    module.machines[0].blocks[0].operations[1] = store(1, 3);
    rejects(&module);
}

#[test]
fn literal_octets_must_imply_the_field_predicates() {
    let mut module = fixture();
    let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *bytes = vec![b'G', 0xff];
    rejects(&module);
    // A qualification roster cannot stand in for the octets.
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields[0].field_type = StructuralFieldType::ByteSequence(shared_view());
    validate_module(&module).expect("a plain view field imposes no predicate");
}

#[test]
fn a_shared_destination_cannot_replace_its_field() {
    let mut module = fixture();
    module.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    rejects(&module);
}

#[test]
fn only_a_shared_view_field_holds_a_relocatable_descriptor() {
    for carrier in [
        ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::MutableBorrow),
        },
        ByteSequenceCarrier::BoundedOwned { capacity: 8 },
    ] {
        let mut module = fixture();
        let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
            unreachable!()
        };
        fields[0].field_type = StructuralFieldType::ByteSequence(carrier);
        rejects(&module);
    }
}

#[test]
fn the_destination_cannot_be_the_source_or_a_forged_place() {
    let mut module = fixture();
    module.machines[0].blocks[0].operations[1] = store(2, 2);
    assert!(validate_module(&module).is_err());
    let mut module = fixture();
    module.machines[0].blocks[0].operations[1].result = OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: id(1),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
    });
    assert!(validate_module(&module).is_err());
}

#[test]
fn a_store_invalidates_an_earlier_field_length_observation() {
    let mut module = fixture();
    let length = Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(3),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(1),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        }),
        kind: OperationKind::StructuralByteSequenceFieldLength {
            source: id(1),
            path: Vec::new(),
            field: id(1),
        },
    };
    let index = Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(4),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(2),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        }),
        kind: OperationKind::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Unsigned(0),
        },
    };
    let read = Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(5),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(3),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        }),
        kind: OperationKind::StructuralByteSequenceFieldRead {
            source: id(1),
            path: Vec::new(),
            field: id(1),
            index: id(2),
            length: id(1),
            obligation: id(1),
        },
    };
    // Observing the stored field after the replacement is current.
    let operations = &mut module.machines[0].blocks[0].operations;
    operations.extend([length.clone(), index.clone(), read.clone()]);
    validate_module(&module).expect("a qualified shared view field reads like a plain one");
    // The same observation taken before the replacement is stale.
    let mut stale = fixture();
    let operations = &mut stale.machines[0].blocks[0].operations;
    operations.insert(1, length);
    operations.extend([index, read]);
    assert!(matches!(
        validate_module(&stale),
        Err(ModuleError::InvalidStructuralByteSequenceFieldAccess(operation)) if operation == id(5)
    ));
}
