//! A case observation of a runtime-selected element:
//! `self.items[self.w] is Color::Red` is one `StructuralCaseMembership`
//! whose path is `items`, `RuntimeIndex { index, obligation }`. The verifier
//! resolves the sum through the array's element type, reconstructs
//! `index < extent` (and `0 <= index` for a signed selector) as the
//! observation's own obligation, and keys no case fact: the observation reads
//! "some element", which names no canonical place.

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
    BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralPathSegment, ValueDeclaration,
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
fn case(raw: u64, identity: &str) -> StructuralCaseDeclaration {
    StructuralCaseDeclaration {
        id: id(raw),
        identity: identity.into(),
        fields: Vec::new(),
    }
}
/// `items`, then the runtime element value 10 selects (obligation 1).
fn element_path() -> Vec<StructuralPathSegment> {
    vec![
        "items".into(),
        StructuralPathSegment::RuntimeIndex {
            index: id(10),
            obligation: id(1),
        },
    ]
}
fn membership(module: &mut TerminalModule) -> &mut OperationKind {
    &mut module.machines[0].blocks[0].operations[1].kind
}

/// `Color { Red, White, Blue }`, `Shade { Red }`, `[Color; 3]`, and a shared
/// `Main { items, tag }` parameter observed at `items[1] is Color::Red`.
fn fixture() -> TerminalModule {
    let mut module = unit_module();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(1),
            identity: "test::Color".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![case(1, "Red"), case(2, "White"), case(3, "Blue")],
            },
        },
        StructuralTypeDeclaration {
            id: id(2),
            identity: "test::Items".into(),
            shape: StructuralTypeShape::FixedArray {
                element: id(1),
                length: 3,
            },
        },
        StructuralTypeDeclaration {
            id: id(3),
            identity: "test::Main".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: id(1),
                        identity: "items".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(id(2)),
                    },
                    StructuralFieldDeclaration {
                        id: id(2),
                        identity: "tag".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(id(1)),
                    },
                ],
            },
        },
        StructuralTypeDeclaration {
            id: id(4),
            identity: "test::Shade".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![case(4, "Red")],
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
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::StructuralCaseMembership {
                source: id(1),
                path: element_path(),
                case: id(1),
            },
        },
    ];
    module
}

/// Prove the observation's reconstructed `index < 3` from the index
/// constant's reconstructed equation and the closed literal relation.
fn bundle_for(module: &TerminalModule) -> ProofBundle {
    let site = reconstruct_operation_obligations(module)
        .unwrap()
        .into_iter()
        .find(|site| site.obligation.id == id(1))
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
fn runtime_element_membership_resolves_the_element_sum_and_verifies() {
    let module = fixture();
    let bundle = bundle_for(&module);
    validate_module(&module).unwrap();
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
}

#[test]
fn runtime_element_membership_mutations_reject_formation() {
    let module = fixture();
    let mutations: Vec<(&str, Box<dyn Fn(&mut TerminalModule)>)> = vec![
        (
            "a case of another sum",
            Box::new(|module| {
                let OperationKind::StructuralCaseMembership { case, .. } = membership(module)
                else {
                    unreachable!()
                };
                *case = id(4);
            }),
        ),
        (
            "a runtime element of a record",
            Box::new(|module| {
                let OperationKind::StructuralCaseMembership { path, .. } = membership(module)
                else {
                    unreachable!()
                };
                *path = vec![StructuralPathSegment::RuntimeIndex {
                    index: id(10),
                    obligation: id(1),
                }];
            }),
        ),
        (
            "a runtime element past the sum",
            Box::new(|module| {
                let OperationKind::StructuralCaseMembership { path, .. } = membership(module)
                else {
                    unreachable!()
                };
                *path = vec![
                    "tag".into(),
                    StructuralPathSegment::RuntimeIndex {
                        index: id(10),
                        obligation: id(1),
                    },
                ];
            }),
        ),
        (
            "a Boolean selector",
            Box::new(|module| {
                module.machines[0].blocks[0].operations[0] = Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: id(1),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: id(10),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::BooleanConstant { value: true },
                };
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

    let mut out_of_extent = module.clone();
    let OperationKind::IntegerConstant { value } =
        &mut out_of_extent.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(3);
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
        .into_iter()
        .find(|site| site.obligation.id == id(1))
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

/// The observation selects "some element", so it names no canonical sum: no
/// later obligation's reconstructed facts carry a case-membership conclusion
/// for it, and a certificate cannot cite one. The same observation at a
/// literal index does publish its conditional case fact, which is what makes
/// the runtime path's absence meaningful.
#[test]
fn a_runtime_element_membership_keys_no_case_fact() {
    let later_axioms = |path: Vec<StructuralPathSegment>| {
        let mut module = fixture();
        let OperationKind::StructuralCaseMembership { path: observed, .. } =
            membership(&mut module)
        else {
            unreachable!()
        };
        *observed = path;
        module.machines[0].blocks[0].operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(3),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(12),
                scalar_type: ScalarType::Integer(u64_type()),
            }),
            kind: OperationKind::ExactIntegerAdd {
                left: id(10),
                right: id(10),
                obligation: id(2),
            },
        });
        validate_module(&module).unwrap();
        reconstruct_operation_obligations(&module)
            .unwrap()
            .into_iter()
            .find(|site| site.obligation.id == id(2))
            .expect("the add's overflow obligation")
            .semantic_axioms
    };
    let keys_case = |fact: &Proposition| {
        matches!(fact, Proposition::Implication { conclusion, .. }
            if matches!(conclusion.as_ref(), Proposition::StructuralCaseMembership { .. }))
    };
    let runtime = later_axioms(element_path());
    assert!(!runtime.iter().any(keys_case), "{runtime:?}");
    let literal = later_axioms(vec!["items".into(), StructuralPathSegment::FixedIndex(1)]);
    assert!(literal.iter().any(keys_case), "{literal:?}");
}
