//! A write-only primitive store through a runtime-selected element: the
//! store's path ends in `RuntimeIndex { index, obligation }`, and the verifier
//! reconstructs `index < declared extent` for that segment from the array its
//! prefix resolves to.

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
    BindingRelevance, ScalarIntegerRange, StructuralFieldDeclaration, StructuralFieldType,
    ValueDeclaration,
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
/// `prefix` followed by the runtime element value 10 selects, whose bound is
/// obligation 1.
fn runtime_path(
    prefix: Vec<terminal_psi::StructuralPathSegment>,
) -> Vec<terminal_psi::StructuralPathSegment> {
    let mut path = prefix;
    path.push(terminal_psi::StructuralPathSegment::RuntimeIndex {
        index: id(10),
        obligation: id(1),
    });
    path
}
fn store_path(module: &mut TerminalModule) -> &mut Vec<terminal_psi::StructuralPathSegment> {
    let OperationKind::WriteOnlyPrimitiveStore { path, .. } =
        &mut module.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    path
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
            suspension_crossing: None,
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
            suspension_crossing: None,
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
            suspension_crossing: None,
            id: id(3),
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyPrimitiveStore {
                destination: id(1),
                path: runtime_path(Vec::new()),
                value: id(11),
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

/// A retained integer entry range publishes `index <= maximum` as a requires
/// proposition on the owning contract, so the reconstructed
/// `index < extent` obligation is discharged from that bound by discreteness —
/// the exact replay the write-only indexed-store producer relies on instead of
/// a literal index equation.
#[test]
fn retained_integer_range_index_proves_the_store_extent_bound() {
    let (mut module, _) = fixture(StructuralAccess::WriteOnlyBorrow);
    let owner = module.machines[0].id;
    {
        let machine = &mut module.machines[0];
        // `index` (value 10) becomes a scalar parameter carrying the authored
        // `0..=2` interval instead of a produced constant; drop the constant.
        machine.parameters = vec![ValueDeclaration {
            qualifications: Default::default(),
            id: id(10),
            scalar_type: u64_type(),
        }];
        machine.contract.requires = vec![Proposition::Conjunction(vec![
            Proposition::LessOrEqual(count(0), index_term()),
            Proposition::LessOrEqual(index_term(), count(2)),
        ])];
        machine.blocks[0].operations.remove(0);
    }
    module.scalar_qualifications.integer_entry_ranges = vec![ScalarIntegerRange {
        machine: owner,
        parameter: id(10),
        integer_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        minimum: IntegerValue::Unsigned(0),
        maximum: IntegerValue::Unsigned(2),
    }];

    let site = reconstruct_operation_obligations(&module)
        .unwrap()
        .pop()
        .expect("the indexed store obligation");
    assert_eq!(
        site.obligation.proposition,
        Proposition::LessThan(index_term(), count(3))
    );

    // `index <= 2` is the range's second requires conjunct; discreteness turns
    // it into `index < 3`, matching the `[u8; 3]` extent.
    let upper = Proposition::LessOrEqual(index_term(), count(2));
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: id(1),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: site.obligation.proposition.clone(),
                    rule: ProofRule::IntegerOrderDiscreteness {
                        relation: Box::new(ProofNode {
                            conclusion: upper,
                            rule: ProofRule::ConjunctionElimination {
                                conjunction: Box::new(ProofNode {
                                    conclusion: Proposition::Conjunction(vec![
                                        Proposition::LessOrEqual(count(0), index_term()),
                                        Proposition::LessOrEqual(index_term(), count(2)),
                                    ]),
                                    rule: ProofRule::Assumption { index: 0 },
                                }),
                                conjunct: 1,
                            },
                        }),
                    },
                },
            }),
        }],
        ..ProofBundle::default()
    };
    validate_module(&module).unwrap();
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
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
    *store_path(&mut module) = runtime_path(vec!["triple".into()]);
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
            // The index must be an integer operand.
            1 => {
                machine.blocks[0].operations[0] = Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: id(1),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: id(10),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::BooleanConstant { value: true },
                }
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
                *store_path(&mut changed) =
                    vec![terminal_psi::StructuralPathSegment::RuntimeIndex {
                        index: id(77),
                        obligation: id(1),
                    }];
            }
            // A repeated static index selects the primitive element, which is
            // not an array and cannot host another index.
            6 => {
                *store_path(&mut changed) = runtime_path(vec![
                    terminal_psi::StructuralPathSegment::FixedIndex(0),
                    terminal_psi::StructuralPathSegment::FixedIndex(0),
                ]);
            }
            // A field after a fixed index does not resolve to an array either.
            7 => {
                *store_path(&mut changed) = runtime_path(vec![
                    terminal_psi::StructuralPathSegment::FixedIndex(0),
                    "triple".into(),
                ]);
            }
            // A literal index that is itself out of bounds never reaches the
            // runtime operand.
            8 => {
                *store_path(&mut changed) = runtime_path(vec!["missing".into()]);
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
