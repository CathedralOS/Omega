//! Call effects invalidate the admitted writable subtree, not its siblings.
//! The callee deliberately changes a leaf; certificates must distinguish an
//! immutable pre-call capture from a fresh read of the post-call storage.

use super::{
    AdmissionProfile, CertificateEnvelope, ContractClause, EvidenceIdentity, EvidenceRoute,
    IntegerSign, IntegerType, IntegerValue, ObligationEvidence, Operation, OperationKind,
    OperationResult, ProofBundle, ProofNode, ProofRule, ProofSystemMarker, Proposition, ScalarTerm,
    ScalarType, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    VerificationError, reconstruct_terminal_obligations, unit_module, verify_module,
};
use semantic_vocabulary::{CanonicalStructuralPathSegment, PsiSemanticId};
use terminal_psi::{BindingRelevance, StructuralPathSegment};

const CELL: u64 = 1;
const ARRAY: u64 = 2;
const STORE: u64 = 3;
const ROOT: u64 = 1;
const EXIT_OBLIGATION: u64 = 500;

fn id<T: PsiSemanticId>(raw: u64) -> T {
    T::new(raw).unwrap()
}

fn integer() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).unwrap()
}

fn field(position: usize) -> ScalarTerm {
    let mut path = if position < 2 {
        vec![
            CanonicalStructuralPathSegment::Field(id(1)),
            CanonicalStructuralPathSegment::FixedIndex(position as u64),
        ]
    } else {
        vec![CanonicalStructuralPathSegment::Field(id(2))]
    };
    path.push(CanonicalStructuralPathSegment::Field(id(1)));
    ScalarTerm::integer_field_path(id(ROOT), path, integer())
}

fn observation(position: usize) -> Proposition {
    Proposition::Equal(
        field(position),
        ScalarTerm::integer(integer(), IntegerValue::Signed(7)).unwrap(),
    )
}

fn value(raw: u64) -> ScalarTerm {
    ScalarTerm::value(id(raw), ScalarType::Integer(integer()))
}

fn declaration(raw: u64) -> ValueDeclaration {
    ValueDeclaration {
        id: id(raw),
        scalar_type: ScalarType::Integer(integer()),
        qualifications: Default::default(),
    }
}

fn structural_field(
    raw: u64,
    name: &str,
    field_type: StructuralFieldType,
) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: id(raw),
        identity: name.to_owned(),
        relevance: BindingRelevance::Relevant,
        field_type,
    }
}

fn parameter(
    raw: u64,
    structural_type: u64,
    access: StructuralAccess,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: id(raw),
        position: 0,
        is_self: false,
        structural_type: id(structural_type),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn place(raw: u64) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id: id(raw),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }
}

fn operation(raw: u64, result: OperationResult, kind: OperationKind) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(raw),
        result,
        kind,
    }
}

fn module(
    path: Vec<StructuralPathSegment>,
    access: StructuralAccess,
    formal_type: u64,
) -> TerminalModule {
    let mut module = unit_module();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(CELL),
            identity: "test::Cell".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![structural_field(
                    1,
                    "value",
                    StructuralFieldType::Scalar(ScalarType::Integer(integer())),
                )],
            },
        },
        StructuralTypeDeclaration {
            id: id(ARRAY),
            identity: "test::Cells".into(),
            shape: StructuralTypeShape::FixedArray {
                element: id(CELL),
                length: 2,
            },
        },
        StructuralTypeDeclaration {
            id: id(STORE),
            identity: "test::Store".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    structural_field(1, "cells", StructuralFieldType::Structural(id(ARRAY))),
                    structural_field(2, "sibling", StructuralFieldType::Structural(id(CELL))),
                ],
            },
        },
    ];
    let mut callee = module.machines[0].clone();
    callee.id = id(901);
    callee.entry = id(901);
    callee.blocks[0].id = id(901);
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: id(901),
        trivial_affine_discards: Vec::new(),
    };
    callee.contract.id = id(901);
    callee.structural_parameters = vec![parameter(10, formal_type, access)];
    callee.structural_places = vec![place(10)];
    if formal_type == CELL && access != StructuralAccess::SharedBorrow {
        callee.blocks[0].operations = vec![
            operation(
                91,
                OperationResult::Scalar(declaration(91)),
                OperationKind::IntegerConstant {
                    value: IntegerValue::Signed(9),
                },
            ),
            operation(
                92,
                OperationResult::Unit,
                OperationKind::StructuralScalarFieldStore {
                    destination: id(10),
                    path: Vec::new(),
                    field: id(1),
                    value: id(91),
                    range_obligation: None,
                },
            ),
        ];
    }
    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![parameter(ROOT, STORE, StructuralAccess::MutableBorrow)];
    caller.structural_places = vec![place(ROOT)];
    caller.contract.requires = (0..3).map(observation).collect();
    caller.contract.ensures = vec![ContractClause {
        obligation: id(EXIT_OBLIGATION),
        proposition: Proposition::Truth,
    }];
    if path.iter().any(|segment| segment.runtime_index().is_some()) {
        caller.blocks[0].operations.push(operation(
            40,
            OperationResult::Scalar(ValueDeclaration {
                id: id(40),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
                qualifications: Default::default(),
            }),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        ));
    }
    caller.blocks[0].operations.push(operation(
        50,
        OperationResult::Unit,
        OperationKind::CallUnit {
            callee: id(901),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: id(ROOT),
                path,
                access,
            }],
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    ));
    module.machines.push(callee);
    module
}

fn exit_facts(module: &TerminalModule) -> Vec<Proposition> {
    reconstruct_terminal_obligations(module)
        .expect("valid projected call")
        .obligations()
        .iter()
        .find(|site| site.obligation.id == id(EXIT_OBLIGATION))
        .expect("caller exit obligation")
        .semantic_axioms
        .clone()
}

#[test]
fn call_write_footprints_preserve_only_disjoint_observations() {
    for (path, access, formal_type, survives) in [
        (
            vec!["cells".into()],
            StructuralAccess::MutableBorrow,
            ARRAY,
            [false, false, true],
        ),
        (
            vec!["sibling".into()],
            StructuralAccess::MutableBorrow,
            CELL,
            [true, true, false],
        ),
        (
            vec!["cells".into()],
            StructuralAccess::SharedBorrow,
            ARRAY,
            [true, true, true],
        ),
        (
            vec![],
            StructuralAccess::MutableBorrow,
            STORE,
            [false, false, false],
        ),
        (
            vec!["cells".into(), StructuralPathSegment::FixedIndex(0)],
            StructuralAccess::MutableBorrow,
            CELL,
            [false, true, true],
        ),
        (
            vec!["cells".into(), StructuralPathSegment::FixedIndex(0)],
            StructuralAccess::WriteOnlyBorrow,
            CELL,
            [false, true, true],
        ),
        (
            vec![
                "cells".into(),
                StructuralPathSegment::RuntimeIndex {
                    index: id(40),
                    obligation: id(41),
                },
            ],
            StructuralAccess::MutableBorrow,
            CELL,
            [false, false, true],
        ),
    ] {
        let module = module(path.clone(), access, formal_type);
        let facts = exit_facts(&module);
        for (position, survives) in survives.into_iter().enumerate() {
            assert_eq!(
                facts.contains(&observation(position)),
                survives,
                "field {position}, {access:?}, {path:?}"
            );
        }
    }
}

fn read(raw: u64) -> Operation {
    operation(
        raw,
        OperationResult::Scalar(declaration(raw)),
        OperationKind::IntegerStructuralField {
            source: id(ROOT),
            path: vec![
                CanonicalStructuralPathSegment::Field(id(1)),
                CanonicalStructuralPathSegment::FixedIndex(0),
            ],
            field: id(1),
        },
    )
}

#[test]
fn call_write_footprints_keep_captured_correlations_not_new_storage_claims() {
    let mut module = module(
        vec!["cells".into(), StructuralPathSegment::FixedIndex(0)],
        StructuralAccess::MutableBorrow,
        CELL,
    );
    let caller = &mut module.machines[0];
    caller
        .contract
        .requires
        .push(Proposition::Equal(field(0), field(2)));
    caller.blocks[0].operations.insert(0, read(21));
    caller.blocks[0].operations.push(read(22));
    caller.result = TerminalMachineResult::Scalar(declaration(99));
    caller.blocks[0].terminator = Terminator::Return {
        edge: id(900),
        value: id(21),
        cleanup_actions: Vec::new(),
    };
    caller.contract.ensures[0].proposition = Proposition::Equal(value(99), field(2));

    let captured_relation = Proposition::Equal(value(21), field(2));
    let fresh_relation = Proposition::Equal(value(22), field(2));
    let facts = exit_facts(&module);
    assert!(facts.contains(&captured_relation));
    assert!(!facts.contains(&Proposition::Equal(field(0), field(2))));
    assert!(!facts.contains(&fresh_relation));

    let certificate = |module: &TerminalModule, returned: u64, claimed: Proposition| {
        let facts = exit_facts(module);
        let returned_fact = Proposition::Equal(value(99), value(returned));
        ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation: id(EXIT_OBLIGATION),
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(1).unwrap(),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: Proposition::Equal(value(99), field(2)),
                        rule: ProofRule::EqualityTransitivity {
                            left_equals_middle: Box::new(ProofNode {
                                conclusion: returned_fact.clone(),
                                rule: ProofRule::SemanticAxiom {
                                    index: facts
                                        .iter()
                                        .position(|fact| fact == &returned_fact)
                                        .unwrap(),
                                },
                            }),
                            middle_equals_right: Box::new(ProofNode {
                                conclusion: claimed,
                                rule: ProofRule::SemanticAxiom {
                                    index: facts
                                        .iter()
                                        .position(|fact| fact == &captured_relation)
                                        .unwrap(),
                                },
                            }),
                        },
                    },
                }),
            }],
            ..ProofBundle::default()
        }
    };
    verify_module(
        &module,
        &certificate(&module, 21, captured_relation.clone()),
        &AdmissionProfile::default(),
    )
    .expect("the pre-call value still equals the untouched sibling");

    // Reconstruct a fresh bundle against the changed return. No stale module
    // identity is involved: the forged premise itself must be rejected.
    module.machines[0].blocks[0].terminator = Terminator::Return {
        edge: id(900),
        value: id(22),
        cleanup_actions: Vec::new(),
    };
    assert!(matches!(
        verify_module(&module, &certificate(&module, 22, fresh_relation), &AdmissionProfile::default()),
        Err(VerificationError::RejectedEvidence { obligation, .. }) if obligation == id(EXIT_OBLIGATION)
    ));
}
