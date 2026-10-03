//! A primitive read that ends at a scalar field of a runtime-selected record
//! element: `self.cells[index].hp` is one `PrimitiveScalarRead` whose path is
//! `cells`, `RuntimeIndex { index, obligation }`, `hp`. The verifier resolves
//! the field from declarations, reconstructs `index < extent` (and
//! `0 <= index` for a signed selector) against the array, and keys no field
//! fact on the read. A static path to a field keeps the canonical field read.

use super::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, IntegerSign, IntegerType, IntegerValue,
    ObligationEvidence, Operation, OperationKind, OperationResult, PrimitiveJudgment, ProofBundle,
    ProofNode, ProofRule, ProofSystemMarker, Proposition, ScalarTerm, ScalarType, StructuralAccess,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeShape, TerminalModule,
    reconstruct_operation_obligations, unit_module, validate_module, verify_module,
};
use semantic_vocabulary::{BoundedIntegerType, PsiSemanticId};
use terminal_psi::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    ValueDeclaration,
};

fn id<T: PsiSemanticId>(raw: u64) -> T {
    T::new(raw).unwrap()
}
fn u64_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}
fn i32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).unwrap()
}
fn count(raw: u128) -> ScalarTerm {
    ScalarTerm::integer(u64_type(), IntegerValue::Unsigned(raw)).unwrap()
}
fn index_term() -> ScalarTerm {
    ScalarTerm::value(id(10), ScalarType::Integer(u64_type()))
}
fn field(identity: &str, field_type: StructuralFieldType, raw: u64) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: id(raw),
        identity: identity.into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    }
}
/// `cells`, the runtime element value 10 selects (obligation 1), then `leaf`.
fn element_path(leaf: StructuralPathSegment) -> Vec<StructuralPathSegment> {
    vec![
        "cells".into(),
        StructuralPathSegment::RuntimeIndex {
            index: id(10),
            obligation: id(1),
        },
        leaf,
    ]
}
fn read_path(module: &mut TerminalModule) -> &mut Vec<StructuralPathSegment> {
    let OperationKind::PrimitiveScalarRead { path, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    path
}

/// `Cell { tag: u8, hp: i32, console: erased, level: bounded u8 }`,
/// `[Cell; 3]`, and a shared `Main { cells }` parameter read at
/// `cells[1].hp`.
fn fixture() -> TerminalModule {
    let mut module = unit_module();
    let octet = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(1),
            identity: "test::Cell".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(
                        "tag",
                        StructuralFieldType::Scalar(ScalarType::Integer(octet)),
                        1,
                    ),
                    field(
                        "hp",
                        StructuralFieldType::Scalar(ScalarType::Integer(i32_type())),
                        2,
                    ),
                    StructuralFieldDeclaration {
                        id: id(3),
                        identity: "console".into(),
                        relevance: BindingRelevance::Erased,
                        field_type: StructuralFieldType::Erased {
                            type_identity: "Console".into(),
                        },
                    },
                    field(
                        "level",
                        StructuralFieldType::BoundedInteger(
                            BoundedIntegerType::new(
                                octet,
                                IntegerValue::Unsigned(0),
                                IntegerValue::Unsigned(9),
                            )
                            .unwrap(),
                        ),
                        4,
                    ),
                ],
            },
        },
        StructuralTypeDeclaration {
            id: id(2),
            identity: "test::Cells".into(),
            shape: StructuralTypeShape::FixedArray {
                element: id(1),
                length: 3,
            },
        },
        StructuralTypeDeclaration {
            id: id(3),
            identity: "test::Main".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![field("cells", StructuralFieldType::Structural(id(2)), 1)],
            },
        },
    ];
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: id(1),
            position: 0,
            is_self: false,
            structural_type: id(3),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places = vec![StructuralPlaceDeclaration {
        id: id(1),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    machine.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(1),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(10),
                scalar_type: ScalarType::Integer(u64_type()),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(1),
            },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(2),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(11),
                scalar_type: ScalarType::Integer(i32_type()),
            }),
            kind: OperationKind::PrimitiveScalarRead {
                source: id(1),
                path: element_path("hp".into()),
            },
        },
    ];
    module
}

/// Prove the read's reconstructed `index < 3` from the index constant's
/// reconstructed equation and the closed literal relation.
fn bundle_for(module: &TerminalModule) -> ProofBundle {
    let site = reconstruct_operation_obligations(module)
        .unwrap()
        .pop()
        .expect("the selected element's bound");
    assert!(site.canonical_certificate);
    assert_eq!(
        site.obligation.proposition,
        Proposition::LessThan(index_term(), count(3))
    );
    let equality = Proposition::Equal(index_term(), count(1));
    let axiom = site
        .semantic_axioms
        .iter()
        .position(|fact| *fact == equality)
        .expect("the index constant's reconstructed equation");
    ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: id(1),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: site.obligation.proposition.clone(),
                    rule: ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(ProofNode {
                            conclusion: Proposition::LessThan(count(1), count(3)),
                            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                        }),
                        equality: Box::new(ProofNode {
                            conclusion: equality,
                            rule: ProofRule::SemanticAxiom { index: axiom },
                        }),
                        endpoint: 0,
                    },
                },
            }),
        }],
        ..ProofBundle::default()
    }
}

#[test]
fn runtime_element_field_read_resolves_the_declared_field_and_verifies() {
    let module = fixture();
    let bundle = bundle_for(&module);
    validate_module(&module).unwrap();
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
}

#[test]
fn element_field_read_mutations_reject_formation() {
    let module = fixture();
    let mutations: Vec<(&str, Box<dyn Fn(&mut TerminalModule)>)> = vec![
        (
            "a field the element does not declare",
            Box::new(|module| *read_path(module) = element_path("missing".into())),
        ),
        (
            "another field of another type",
            Box::new(|module| *read_path(module) = element_path("tag".into())),
        ),
        (
            "an erased field",
            Box::new(|module| *read_path(module) = element_path("console".into())),
        ),
        // A bounded field's declared range belongs to the canonical field
        // read; this route supplies no range, so it does not admit one.
        (
            "a bounded field",
            Box::new(|module| *read_path(module) = element_path("level".into())),
        ),
        // A static carrier has one canonical spelling: IntegerStructuralField.
        (
            "a static carrier claimed through the primitive read",
            Box::new(|module| {
                *read_path(module) = vec![
                    "cells".into(),
                    StructuralPathSegment::FixedIndex(1),
                    "hp".into(),
                ]
            }),
        ),
        (
            "a field step past the scalar leaf",
            Box::new(|module| {
                let path = read_path(module);
                path.push("hp".into());
            }),
        ),
        (
            "an index where the field belongs",
            Box::new(|module| {
                *read_path(module) = element_path(StructuralPathSegment::FixedIndex(0))
            }),
        ),
        (
            "a write-only root",
            Box::new(|module| {
                module.machines[0].structural_parameters[0].access =
                    StructuralAccess::WriteOnlyBorrow
            }),
        ),
    ];
    for (name, mutation) in mutations {
        let mut changed = module.clone();
        mutation(&mut changed);
        assert!(validate_module(&changed).is_err(), "{name}");
    }
}

#[test]
fn unproved_signed_or_out_of_extent_selectors_never_acquire_evidence() {
    let module = fixture();
    let bundle = bundle_for(&module);
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err(),
        "the reconstructed bound cannot be assumed"
    );

    let mut shrunk = module.clone();
    let StructuralTypeShape::FixedArray { length, .. } = &mut shrunk.structural_types[1].shape
    else {
        unreachable!()
    };
    *length = 1;
    validate_module(&shrunk).unwrap();
    assert!(
        verify_module(&shrunk, &bundle, &AdmissionProfile::default()).is_err(),
        "a stale extent invalidates the certificate"
    );

    let mut out_of_extent = module.clone();
    let OperationKind::IntegerConstant { value } =
        &mut out_of_extent.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(5);
    validate_module(&out_of_extent).unwrap();
    assert!(
        verify_module(&out_of_extent, &bundle, &AdmissionProfile::default()).is_err(),
        "an out-of-extent selector has no certificate"
    );

    // A signed selector owes `0 <= index` as well. A certificate for the
    // unsigned upper bound alone does not discharge it.
    let mut signed = module;
    let signed_type = ScalarType::Integer(i32_type());
    signed.machines[0].blocks[0].operations[0] = Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(1),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(10),
            scalar_type: signed_type,
        }),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Signed(1),
        },
    };
    validate_module(&signed).unwrap();
    let site = reconstruct_operation_obligations(&signed)
        .unwrap()
        .pop()
        .expect("the signed selector's bound");
    let lower = Proposition::LessOrEqual(
        ScalarTerm::integer(i32_type(), IntegerValue::Signed(0)).unwrap(),
        ScalarTerm::value(id(10), signed_type),
    );
    let mentions_lower = |proposition: &Proposition| match proposition {
        Proposition::Conjunction(members) => members.contains(&lower),
        other => *other == lower,
    };
    assert!(
        mentions_lower(&site.obligation.proposition),
        "{:?}",
        site.obligation.proposition
    );
    assert!(verify_module(&signed, &bundle, &AdmissionProfile::default()).is_err());
}

/// The read selects "some element", so it names no canonical leaf: no later
/// obligation's reconstructed facts equate its value with a field term, and a
/// certificate cannot cite one.
#[test]
fn a_runtime_element_read_keys_no_field_fact() {
    let mut module = fixture();
    let machine = &mut module.machines[0];
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(3),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(12),
            scalar_type: ScalarType::Integer(i32_type()),
        }),
        kind: OperationKind::ExactIntegerAdd {
            left: id(11),
            right: id(11),
            obligation: id(2),
        },
    });
    validate_module(&module).unwrap();
    let site = reconstruct_operation_obligations(&module)
        .unwrap()
        .into_iter()
        .find(|site| site.obligation.id == id(2))
        .expect("the add's overflow obligation");
    let read = ScalarTerm::value(id(11), ScalarType::Integer(i32_type()));
    let keys_field = |fact: &Proposition| {
        matches!(fact, Proposition::Equal(left, right)
            if (*left == read && matches!(right, ScalarTerm::IntegerField { .. }))
                || (*right == read && matches!(left, ScalarTerm::IntegerField { .. })))
    };
    assert!(
        !site.semantic_axioms.iter().any(keys_field),
        "{:?}",
        site.semantic_axioms
    );
}

/// A field store through a runtime element writes some element of the array
/// it selects in. It forgets what was observed beneath that array
/// (`cells[0].hp`) and nothing else: the sibling `i` keeps its read equation,
/// which is what lets a loop that stores `cells[i].hp` keep its selector's
/// bounds across the store. It publishes no field equation of its own.
#[test]
fn a_runtime_element_field_store_forgets_only_its_array() {
    use semantic_vocabulary::CanonicalStructuralPathSegment as Canonical;
    let mut module = fixture();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[2].shape else {
        unreachable!()
    };
    fields.push(field(
        "i",
        StructuralFieldType::Scalar(ScalarType::Integer(u64_type())),
        2,
    ));
    let machine = &mut module.machines[0];
    machine.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let scalar = |raw: u64, scalar_type: ScalarType| {
        OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(raw),
            scalar_type,
        })
    };
    let operation = |raw: u64, result: OperationResult, kind: OperationKind| Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(raw),
        result,
        kind,
    };
    let selector = ScalarTerm::value(id(20), ScalarType::Integer(u64_type()));
    let sibling = ScalarTerm::integer_field_path(id(1), vec![Canonical::Field(id(2))], u64_type());
    let element = ScalarTerm::integer_field_path(
        id(1),
        vec![
            Canonical::Field(id(1)),
            Canonical::FixedIndex(0),
            Canonical::Field(id(2)),
        ],
        i32_type(),
    );
    let element_value = ScalarTerm::value(id(21), ScalarType::Integer(i32_type()));
    machine.blocks[0].operations = vec![
        operation(
            1,
            scalar(20, ScalarType::Integer(u64_type())),
            OperationKind::IntegerStructuralField {
                source: id(1),
                path: Vec::new(),
                field: id(2),
            },
        ),
        operation(
            2,
            scalar(21, ScalarType::Integer(i32_type())),
            OperationKind::IntegerStructuralField {
                source: id(1),
                path: vec![Canonical::Field(id(1)), Canonical::FixedIndex(0)],
                field: id(2),
            },
        ),
        operation(
            3,
            scalar(10, ScalarType::Integer(u64_type())),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(1),
            },
        ),
        operation(
            4,
            scalar(11, ScalarType::Integer(i32_type())),
            OperationKind::IntegerConstant {
                value: IntegerValue::Signed(7),
            },
        ),
        operation(
            5,
            OperationResult::Unit,
            OperationKind::StructuralScalarFieldStore {
                destination: id(1),
                path: vec![
                    "cells".into(),
                    StructuralPathSegment::RuntimeIndex {
                        index: id(10),
                        obligation: id(1),
                    },
                ],
                field: id(2),
                value: id(11),
                range_obligation: None,
            },
        ),
        operation(
            6,
            scalar(12, ScalarType::Integer(u64_type())),
            OperationKind::ExactIntegerAdd {
                left: id(20),
                right: id(20),
                obligation: id(2),
            },
        ),
    ];
    validate_module(&module).unwrap();
    let sites = reconstruct_operation_obligations(&module).unwrap();
    let at = |obligation: u64| {
        sites
            .iter()
            .find(|site| site.obligation.id == id(obligation))
            .expect("reconstructed site")
    };
    let sibling_fact = Proposition::Equal(selector, sibling);
    let element_fact = Proposition::Equal(element_value, element);
    let before = &at(1).semantic_axioms;
    assert!(before.contains(&sibling_fact), "{before:?}");
    assert!(before.contains(&element_fact), "{before:?}");
    let after = &at(2).semantic_axioms;
    assert!(
        after.contains(&sibling_fact),
        "the sibling survives: {after:?}"
    );
    assert!(
        !after.contains(&element_fact),
        "the array is forgotten: {after:?}"
    );
    let stored = ScalarTerm::value(id(11), ScalarType::Integer(i32_type()));
    assert!(
        !after
            .iter()
            .any(|fact| matches!(fact, Proposition::Equal(left, right)
            if (*left == stored && matches!(right, ScalarTerm::IntegerField { .. }))
                || (*right == stored && matches!(left, ScalarTerm::IntegerField { .. })))),
        "a runtime element store publishes no field equation: {after:?}"
    );
}

/// Constructing the enclosing record does not stop a scalar observation from
/// reading an element just because that element also carries a shared view.
#[test]
fn constructed_view_bearing_array_root_preserves_indexed_scalar_read_authority() {
    use terminal_psi::{
        ByteSequenceCarrier, RecordFieldInitializer, RecordFieldValue, StructuralArgument,
        StructuralOperationResult,
    };
    let mut module = fixture();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields.push(field(
        "label",
        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        }),
        5,
    ));
    let machine = &mut module.machines[0];
    machine.structural_parameters[0].structural_type = id(2);
    machine.structural_parameters[0].access = StructuralAccess::Owned;
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(2),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(3),
            structural_type: id(3),
        },
    });
    let OperationKind::PrimitiveScalarRead { source, .. } =
        &mut machine.blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *source = id(2);
    machine.blocks[0].operations.insert(
        0,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(3),
            result: OperationResult::Structural(StructuralOperationResult {
                place: id(2),
                structural_type: id(3),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishRecord {
                fields: vec![RecordFieldInitializer {
                    field: id(1),
                    value: RecordFieldValue::Structural(StructuralArgument {
                        place: id(1),
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    }),
                }],
            },
        },
    );
    let bundle = bundle_for(&module);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );

    // A view-bearing root cannot disguise a descriptor as a primitive leaf.
    let mut wrong_leaf = module.clone();
    let OperationKind::PrimitiveScalarRead { path, .. } =
        &mut wrong_leaf.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    *path.last_mut().unwrap() = "label".into();
    assert!(validate_module(&wrong_leaf).is_err());

    // Result declaration order alone never supplies initialization authority.
    let mut read_before_construction = module;
    read_before_construction.machines[0].blocks[0]
        .operations
        .rotate_left(1);
    assert!(validate_module(&read_before_construction).is_err());
}
