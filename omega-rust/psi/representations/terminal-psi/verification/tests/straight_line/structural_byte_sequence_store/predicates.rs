use super::*;
use terminal_psi::{ByteSequencePredicate, ByteSequencePredicateSet};

fn restrict_field(module: &mut TerminalModule, predicate: ByteSequencePredicate) {
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    predicates.insert(predicate);
    fields[0].field_type = StructuralFieldType::PredicateByteSequence {
        carrier: ByteSequenceCarrier::BoundedOwned { capacity: 3 },
        predicates,
    };
}

#[test]
fn restricted_field_replacement_replays_literal_bytes() {
    let (mut module, _) = fixture(StructuralAccess::MutableBorrow);
    restrict_field(&mut module, ByteSequencePredicate::ValidUtf8);
    validate_module(&module).expect("ASCII literal preserves UTF-8 field");
    let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    bytes[0] = 255;
    validate_module(&module)
        .expect_err("equal-length invalid UTF-8 cannot satisfy restricted field replacement");
}

#[test]
fn literal_qualification_cannot_replace_independent_octet_checking() {
    let (mut module, _) = fixture(StructuralAccess::MutableBorrow);
    module.structural_domains.push(StructuralDomainDeclaration {
        id: id(1),
        semantic_domain: id(1),
        identity: "test::Utf8".into(),
        carrier: id(2),
        byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
        content_projection: None,
        establishment_routes: Vec::new(),
    });
    let OperationKind::EstablishByteSequenceLiteral { qualifications, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    qualifications.push(id(1));
    validate_module(&module).expect("exact literal bytes discharge the primitive domain");
    let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    bytes[0] = 255;
    let error = validate_module(&module)
        .expect_err("forged literal membership must reject before calls consume it");
    assert!(
        matches!(
            error,
            terminal_verifier::ModuleError::InvalidByteSequenceLiteralQualifications(_)
        ),
        "{error:?}"
    );
}

#[test]
fn restricted_field_rejects_unproved_indexed_mutation() {
    let (mut module, _) = fixture(StructuralAccess::MutableBorrow);
    restrict_field(&mut module, ByteSequencePredicate::ValidUtf8);
    validate_module(&module).unwrap();
    module.machines[0].blocks[0].operations.extend([
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(5),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(3),
                scalar_type: ScalarType::Integer(integer()),
            }),
            kind: OperationKind::StructuralByteSequenceFieldLength {
                source: id(1),
                path: Vec::new(),
                field: id(1),
            },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(6),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(4),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                ),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(255),
            },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(7),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralByteSequenceFieldByteStore {
                destination: id(1),
                path: Vec::new(),
                field: id(1),
                index: id(2),
                value: id(4),
                length: id(3),
                obligation: id(2),
            },
        },
    ]);
    assert_eq!(
        validate_module(&module).err(),
        Some(terminal_verifier::ModuleError::InvalidStructuralByteSequenceFieldAccess(id(7)))
    );
}

#[test]
fn zeroed_attachment_cannot_invent_nonempty_membership() {
    let (mut module, _) = fixture(StructuralAccess::MutableBorrow);
    restrict_field(&mut module, ByteSequencePredicate::NonEmpty);
    module.machines[0].attachment = Some(id(1));
    assert_eq!(
        validate_module(&module).err(),
        Some(
            terminal_verifier::ModuleError::InvalidStructuralFieldIdentity {
                structural_type: id(1),
                field: id(1),
            }
        )
    );
}
