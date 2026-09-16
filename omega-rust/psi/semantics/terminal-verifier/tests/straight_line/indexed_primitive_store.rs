//! Runtime-indexed write-only primitive stores carry an independently
//! reconstructed `index < declared extent` obligation over a u64 operand.

use super::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, IntegerSign, IntegerType, IntegerValue,
    ObligationEvidence, Operation, OperationKind, OperationResult, PrimitiveJudgment, ProofBundle,
    ProofNode, ProofRule, ProofSystemMarker, Proposition, ScalarTerm, ScalarType, StructuralAccess,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeShape, TerminalModule,
    reconstruct_operation_obligations, unit_module, validate_module, verify_module,
};
use semantic_vocabulary::PsiSemanticId;
use terminal_psi::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, ValueDeclaration,
};

fn id<T: PsiSemanticId>(raw: u64) -> T {
    T::new(raw).unwrap()
}
fn u64_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}
fn count(raw: u128) -> ScalarTerm {
    ScalarTerm::integer(
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        IntegerValue::Unsigned(raw),
    )
    .unwrap()
}
fn index_term() -> ScalarTerm {
    ScalarTerm::value(id(10), u64_type())
}

fn fixture(access: StructuralAccess) -> (TerminalModule, ProofBundle) {
    let mut module = unit_module();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(1),
            identity: "test::Octet".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            )),
        },
        StructuralTypeDeclaration {
            id: id(2),
            identity: "test::Triple".into(),
            shape: StructuralTypeShape::FixedArray {
                element: id(1),
                length: 3,
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
            structural_type: id(2),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access,
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
            id: id(1),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(10),
                scalar_type: u64_type(),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(1),
            },
        },
        Operation {
            static_reach_binding: None,
            id: id(2),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(11),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                ),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(7),
            },
        },
        Operation {
            static_reach_binding: None,
            id: id(3),
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyIndexedPrimitiveStore {
                destination: id(1),
                path: Vec::new(),
                index: id(10),
                value: id(11),
                obligation: id(1),
            },
        },
    ];
    let bundle = bundle_for(&module);
    (module, bundle)
}

/// Reconstruct the store's `index < extent` obligation and prove it from the
/// closed literal relation and the index constant's reconstructed equation.
fn bundle_for(module: &TerminalModule) -> ProofBundle {
    let site = reconstruct_operation_obligations(module)
        .unwrap()
        .pop()
        .expect("the indexed store obligation");
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
fn runtime_indexed_store_reconstructs_extent_obligation_and_verifies() {
    for access in [
        StructuralAccess::WriteOnlyBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let (module, bundle) = fixture(access);
        validate_module(&module).unwrap();
        verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    }
}

#[test]
fn nested_field_path_to_the_array_resolves_the_same_extent() {
    let (mut module, _) = fixture(StructuralAccess::WriteOnlyBorrow);
    module.structural_types.push(StructuralTypeDeclaration {
        id: id(3),
        identity: "test::Envelope".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: id(1),
                identity: "triple".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(id(2)),
            }],
        },
    });
    module.machines[0].structural_parameters[0].structural_type = id(3);
    let OperationKind::WriteOnlyIndexedPrimitiveStore { path, .. } =
        &mut module.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
        id(1),
    ));
    let bundle = bundle_for(&module);
    validate_module(&module).unwrap();
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
}

#[test]
fn indexed_store_mutations_reject_formation() {
    let (module, _) = fixture(StructuralAccess::WriteOnlyBorrow);
    for mutation in 0..10 {
        let mut changed = module.clone();
        let machine = &mut changed.machines[0];
        match mutation {
            // A shared borrow can never host a runtime-selected write.
            0 => machine.structural_parameters[0].access = StructuralAccess::SharedBorrow,
            // The index must be an exact u64 operand.
            1 => {
                machine.blocks[0].operations[0]
                    .result
                    .scalar_mut()
                    .unwrap()
                    .scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
            }
            // The stored value must exactly match the declared element type.
            2 => {
                machine.blocks[0].operations[1]
                    .result
                    .scalar_mut()
                    .unwrap()
                    .scalar_type = u64_type()
            }
            // The store's Unit effect cannot silently produce a scalar.
            3 => {
                machine.blocks[0].operations[2].result = OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: id(12),
                    scalar_type: u64_type(),
                })
            }
            // Dominance: the index and value must be defined before the store.
            4 => machine.blocks[0].operations.swap(1, 2),
            5 => {
                let OperationKind::WriteOnlyIndexedPrimitiveStore { index, .. } =
                    &mut machine.blocks[0].operations[2].kind
                else {
                    unreachable!()
                };
                *index = id(77);
            }
            // A repeated static index selects the primitive element, which is
            // not an array and cannot host another index.
            6 => {
                let OperationKind::WriteOnlyIndexedPrimitiveStore { path, .. } =
                    &mut machine.blocks[0].operations[2].kind
                else {
                    unreachable!()
                };
                *path = vec![
                    semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(0),
                    semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(0),
                ];
            }
            // A field after a fixed index does not resolve to an array either.
            7 => {
                let OperationKind::WriteOnlyIndexedPrimitiveStore { path, .. } =
                    &mut machine.blocks[0].operations[2].kind
                else {
                    unreachable!()
                };
                *path = vec![
                    semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(0),
                    semantic_vocabulary::CanonicalStructuralPathSegment::Field(id(1)),
                ];
            }
            // A literal index that is itself out of bounds never reaches the
            // runtime operand.
            8 => {
                let OperationKind::WriteOnlyIndexedPrimitiveStore { path, .. } =
                    &mut machine.blocks[0].operations[2].kind
                else {
                    unreachable!()
                };
                *path = vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    id(9),
                )];
            }
            // Obligation identities are unique across the machine.
            9 => {
                let mut second = machine.blocks[0].operations[2].clone();
                second.id = id(4);
                machine.blocks[0].operations.push(second);
            }
            _ => unreachable!(),
        }
        assert!(validate_module(&changed).is_err(), "mutation {mutation}");
    }
}

#[test]
fn unproved_or_out_of_extent_indexes_never_acquire_evidence() {
    let (module, bundle) = fixture(StructuralAccess::WriteOnlyBorrow);
    // No evidence: the reconstructed obligation cannot be assumed.
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
    // A stale extent invalidates the certificate.
    let mut shrunk = module.clone();
    let StructuralTypeShape::FixedArray { length, .. } = &mut shrunk.structural_types[1].shape
    else {
        unreachable!()
    };
    *length = 1;
    validate_module(&shrunk).unwrap();
    assert!(verify_module(&shrunk, &bundle, &AdmissionProfile::default()).is_err());
    // An out-of-bounds constant index can be formed but honestly proved by
    // nothing: the reconstructed goal `5 < 3` has no certificate.
    let mut out_of_bounds = module;
    let OperationKind::IntegerConstant { value } =
        &mut out_of_bounds.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(5);
    validate_module(&out_of_bounds).unwrap();
    let site = reconstruct_operation_obligations(&out_of_bounds)
        .unwrap()
        .pop()
        .expect("out-of-bounds obligation");
    assert_eq!(
        site.obligation.proposition,
        Proposition::LessThan(index_term(), count(3))
    );
    assert!(verify_module(&out_of_bounds, &bundle, &AdmissionProfile::default()).is_err());
}
