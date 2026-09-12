//! Selected scalar payloads retain their exact root, case, and field denotation.

use super::*;
use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralFieldId};

fn integer() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).unwrap()
}

fn term(value: u64) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(value).unwrap(), ScalarType::Integer(integer()))
}

fn field(root: u64, case: u64, field: u64) -> ScalarTerm {
    ScalarTerm::integer_field_path(
        PlaceId::new(root).unwrap(),
        vec![
            CanonicalStructuralPathSegment::Case(StructuralCaseId::new(case).unwrap()),
            CanonicalStructuralPathSegment::Field(StructuralFieldId::new(field).unwrap()),
        ],
        integer(),
    )
}

fn payload_module() -> TerminalModule {
    let mut module = case_access::dispatch_module(StructuralAccess::SharedBorrow);
    let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
        panic!("sum fixture")
    };
    cases.truncate(1);
    cases[0].fields = (1..=2)
        .map(|position| StructuralFieldDeclaration {
            id: StructuralFieldId::new(position).unwrap(),
            identity: format!("field{position}"),
            field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer())),
            relevance: terminal_psi::BindingRelevance::Relevant,
        })
        .collect();
    let machine = &mut module.machines[0];
    machine.blocks.truncate(2);
    let Terminator::StructuralCase { cases, .. } = &mut machine.blocks[0].terminator else {
        panic!("dispatch")
    };
    cases.truncate(1);
    cases[0].payload_fields = vec![
        StructuralFieldId::new(2).unwrap(),
        StructuralFieldId::new(1).unwrap(),
    ];
    machine.blocks[1].parameters = [10, 11]
        .map(|value| ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(value).unwrap(),
            scalar_type: ScalarType::Integer(integer()),
        })
        .to_vec();
    machine.blocks[1].terminator = Terminator::Return {
        edge: EdgeId::new(20).unwrap(),
        value: ValueId::new(10).unwrap(),
        cleanup_actions: Vec::new(),
    };
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(30).unwrap(),
        scalar_type: ScalarType::Integer(integer()),
    });
    machine.contract.ensures = vec![ContractClause {
        obligation: ObligationId::new(1).unwrap(),
        proposition: Proposition::Equal(term(30), field(1, 1, 2)),
    }];
    module
}

fn bounded_payload_module() -> TerminalModule {
    let mut module = payload_module();
    let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
        panic!("sum fixture")
    };
    cases[0].fields[1].field_type = StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            integer(),
            IntegerValue::Signed(0),
            IntegerValue::Signed(255),
        )
        .unwrap(),
    );
    module.machines[0].contract.ensures.clear();
    let mut receiver = unit_module().machines.remove(0);
    receiver.id = MachineId::new(901).unwrap();
    receiver.contract.id = ContractId::new(901).unwrap();
    receiver.entry = BlockId::new(901).unwrap();
    receiver.blocks[0].id = receiver.entry;
    receiver.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(901).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    receiver.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(1).unwrap(),
        scalar_type: ScalarType::Integer(integer()),
    }];
    receiver.contract.requires = payload_bounds(1);
    module.machines[0].blocks[1].operations.push(Operation {
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: receiver.id,
            arguments: vec![ValueId::new(10).unwrap()],
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: vec![
                ObligationId::new(1).unwrap(),
                ObligationId::new(2).unwrap(),
            ],
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(receiver);
    module
}

fn payload_bounds(value: u64) -> Vec<Proposition> {
    vec![
        Proposition::LessOrEqual(
            ScalarTerm::integer(integer(), IntegerValue::Signed(0)).unwrap(),
            term(value),
        ),
        Proposition::LessOrEqual(
            term(value),
            ScalarTerm::integer(integer(), IntegerValue::Signed(255)).unwrap(),
        ),
    ]
}

fn bounded_payload_bundle(module: &TerminalModule) -> ProofBundle {
    let reconstructed = reconstruct_terminal_obligations(module).unwrap();
    ProofBundle {
        evidence: reconstructed
            .obligations()
            .iter()
            .map(|site| {
                let conclusion = site.obligation.proposition.clone();
                let index = site
                    .semantic_axioms
                    .iter()
                    .position(|fact| fact == &conclusion)
                    .expect("selected payload declares this exact bound");
                ObligationEvidence {
                    obligation: site.obligation.id,
                    route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                        identity: EvidenceIdentity::new(site.obligation.id.get()).unwrap(),
                        proof_system_marker: ProofSystemMarker::CURRENT,
                        proof: ProofNode {
                            conclusion,
                            rule: ProofRule::SemanticAxiom { index },
                        },
                    }),
                }
            })
            .collect(),
        ..ProofBundle::default()
    }
}

#[test]
fn selected_bounded_payload_supplies_both_checked_call_requirements() {
    let module = bounded_payload_module();
    verify_module(
        &module,
        &bounded_payload_bundle(&module),
        &AdmissionProfile::default(),
    )
    .unwrap();
}

#[test]
fn selected_bounded_boundary_result_supplies_both_checked_call_requirements() {
    let mut module = bounded_payload_module();
    let machine = &mut module.machines[0];
    let source = machine.structural_parameters.remove(0);
    let producer = OperationId::new(2).unwrap();
    let boundary = BoundaryMachineId::new(1).unwrap();
    machine.structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer,
        structural_type: source.structural_type,
    };
    machine.blocks[0].operations.push(Operation {
        id: producer,
        result: OperationResult::Structural(StructuralOperationResult {
            place: source.place,
            structural_type: source.structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::BoundaryCall {
            boundary,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    });
    let Terminator::StructuralCase { cases, .. } = &mut machine.blocks[0].terminator else {
        panic!("case")
    };
    cases[0].trivial_affine_discards.push(source.place);
    module.boundary_machines.push(BoundaryMachineDeclaration {
        crash_routes: Vec::new(),
        id: boundary,
        identity: "Input::observe".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: terminal_psi::BoundaryMachineResult::Structural(
            terminal_psi::BoundaryStructuralResultDeclaration {
                structural_type: source.structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
            },
        ),
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    verify_module(
        &module,
        &bounded_payload_bundle(&module),
        &AdmissionProfile::default(),
    )
    .unwrap();
}

#[test]
fn selected_bounded_payload_cannot_borrow_sibling_bounds_or_widened_declaration() {
    let original = bounded_payload_module();
    let bundle = bounded_payload_bundle(&original);
    for swap in [false, true] {
        let mut module = original.clone();
        if swap {
            let Terminator::StructuralCase { cases, .. } =
                &mut module.machines[0].blocks[0].terminator
            else {
                panic!("case")
            };
            cases[0].payload_fields.reverse();
        } else {
            let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
                panic!("sum")
            };
            cases[0].fields[1].field_type = StructuralFieldType::BoundedInteger(
                semantic_vocabulary::BoundedIntegerType::new(
                    integer(),
                    IntegerValue::Signed(-1),
                    IntegerValue::Signed(256),
                )
                .unwrap(),
            );
        }
        validate_module(&module).unwrap();
        assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());
    }
}

#[test]
fn selected_bounded_payload_does_not_leak_across_an_unrestricted_case_join() {
    let mut module = bounded_payload_module();
    let bundle = bounded_payload_bundle(&module);
    let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
        panic!("sum")
    };
    let mut other = cases[0].clone();
    other.id = StructuralCaseId::new(2).unwrap();
    other.identity = "Other".into();
    other.fields[1].field_type = StructuralFieldType::Scalar(ScalarType::Integer(integer()));
    cases.push(other);
    let Terminator::StructuralCase { cases, .. } = &mut module.machines[0].blocks[0].terminator
    else {
        panic!("case")
    };
    let mut other = cases[0].clone();
    other.case = StructuralCaseId::new(2).unwrap();
    other.edge = EdgeId::new(11).unwrap();
    cases.push(other);
    validate_module(&module).unwrap();
    assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());
}

#[test]
fn selected_bounded_payload_uses_the_actual_root_declaration() {
    let mut module = bounded_payload_module();
    let bundle = bounded_payload_bundle(&module);
    let mut unrelated = module.structural_types[0].clone();
    unrelated.id = StructuralTypeId::new(2).unwrap();
    unrelated.identity = "UnrestrictedLookalike".into();
    let StructuralTypeShape::Sum { cases } = &mut unrelated.shape else {
        panic!("sum")
    };
    cases[0].fields[1].field_type = StructuralFieldType::Scalar(ScalarType::Integer(integer()));
    module.machines[0].structural_parameters[0].structural_type = unrelated.id;
    module.structural_types.push(unrelated);
    validate_module(&module).unwrap();
    assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());
}

#[test]
fn selected_bounded_payload_is_a_snapshot_after_mutation_and_forwarding() {
    let mut module = forwarded_call_module();
    let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
        panic!("sum")
    };
    cases[0].fields[1].field_type = StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            integer(),
            IntegerValue::Signed(0),
            IntegerValue::Signed(255),
        )
        .unwrap(),
    );
    let reconstructed = reconstruct_terminal_obligations(&module).unwrap();
    let axioms = &reconstructed.obligations()[0].semantic_axioms;
    for bound in payload_bounds(12) {
        assert!(axioms.contains(&bound));
    }
    assert!(!axioms.contains(&Proposition::Equal(term(12), field(1, 1, 2))));
}

#[test]
fn bounded_integer_record_constructor_remains_fail_closed() {
    let mut module = unit_module();
    let integer = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let place = PlaceId::new(1).unwrap();
    let operation = OperationId::new(1).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "BoundedRecord".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: field,
                identity: "value".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer)),
            }],
        },
    });
    let machine = &mut module.machines[0];
    machine
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type,
            },
        });
    machine.blocks[0].operations.push(Operation {
        id: operation,
        result: OperationResult::Structural(StructuralOperationResult {
            place,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishAffineScalarRecord {
            field,
            value: IntegerValue::Signed(0),
        },
    });
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        panic!("Unit fixture")
    };
    trivial_affine_discards.push(place);
    validate_module(&module).expect("raw i64 construction is the unchanged supported route");
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        panic!("record")
    };
    fields[0].field_type = StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            integer,
            IntegerValue::Signed(0),
            IntegerValue::Signed(255),
        )
        .unwrap(),
    );
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::AffineScalarRecordRequiresSingleI64Field { .. })
    ));
}

fn bundle() -> ProofBundle {
    payload_return_bundle(term(30), term(10), field(1, 1, 2))
}

fn payload_return_bundle(
    returned: ScalarTerm,
    selected: ScalarTerm,
    observed: ScalarTerm,
) -> ProofBundle {
    ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: ObligationId::new(1).unwrap(),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::Equal(returned.clone(), observed.clone()),
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(ProofNode {
                            conclusion: Proposition::Equal(returned, selected.clone()),
                            rule: ProofRule::SemanticAxiom { index: 3 },
                        }),
                        middle_equals_right: Box::new(ProofNode {
                            conclusion: Proposition::Equal(selected, observed),
                            rule: ProofRule::SemanticAxiom { index: 1 },
                        }),
                    },
                },
            }),
        }],
        ..ProofBundle::default()
    }
}

#[test]
fn selected_case_boolean_payload_has_a_checked_certificate() {
    let mut module = payload_module();
    let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
        panic!("sum");
    };
    for field in &mut cases[0].fields {
        field.field_type = StructuralFieldType::Scalar(ScalarType::Boolean);
    }
    let machine = &mut module.machines[0];
    for parameter in &mut machine.blocks[1].parameters {
        parameter.scalar_type = ScalarType::Boolean;
    }
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(30).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    let returned = ScalarTerm::value(ValueId::new(30).unwrap(), ScalarType::Boolean);
    let observed = ScalarTerm::boolean_field_path(
        PlaceId::new(1).unwrap(),
        vec![
            CanonicalStructuralPathSegment::Case(StructuralCaseId::new(1).unwrap()),
            CanonicalStructuralPathSegment::Field(StructuralFieldId::new(2).unwrap()),
        ],
    );
    machine.contract.ensures[0].proposition =
        Proposition::Equal(returned.clone(), observed.clone());
    let evidence = payload_return_bundle(
        returned,
        ScalarTerm::value(ValueId::new(10).unwrap(), ScalarType::Boolean),
        observed,
    );
    verify_module(&module, &evidence, &AdmissionProfile::default()).unwrap();
}

#[test]
fn selected_case_payload_denotation_has_a_checked_certificate() {
    verify_module(&payload_module(), &bundle(), &AdmissionProfile::default())
        .expect("exact reordered payload fields establish the return guarantee");
}

#[test]
fn selected_case_payload_certificate_survives_ordinary_forwarding() {
    let mut module = payload_module();
    let machine = &mut module.machines[0];
    machine.blocks[1].terminator = Terminator::Jump {
        edge: EdgeId::new(20).unwrap(),
        target: BlockId::new(12).unwrap(),
        arguments: vec![ValueId::new(10).unwrap()],
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(Block {
        id: BlockId::new(12).unwrap(),
        parameters: vec![ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(12).unwrap(),
            scalar_type: ScalarType::Integer(integer()),
        }],
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Return {
            edge: EdgeId::new(21).unwrap(),
            value: ValueId::new(12).unwrap(),
            cleanup_actions: Vec::new(),
        },
    });
    let mut evidence = bundle();
    let EvidenceRoute::CertificateDerived(certificate) = &mut evidence.evidence[0].route else {
        panic!("certificate")
    };
    let ProofRule::EqualityTransitivity {
        left_equals_middle, ..
    } = &mut certificate.proof.rule
    else {
        panic!("equality proof")
    };
    **left_equals_middle = ProofNode {
        conclusion: Proposition::Equal(term(30), term(10)),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(30), term(12)),
                rule: ProofRule::SemanticAxiom { index: 4 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(12), term(10)),
                rule: ProofRule::SemanticAxiom { index: 3 },
            }),
        },
    };
    verify_module(&module, &evidence, &AdmissionProfile::default())
        .expect("payload identity composes with ordinary scalar staging");
}

#[test]
fn selected_case_payload_certificate_rejects_substituted_field() {
    let mut module = payload_module();
    let Terminator::StructuralCase { cases, .. } = &mut module.machines[0].blocks[0].terminator
    else {
        panic!("dispatch")
    };
    cases[0].payload_fields.reverse();
    validate_module(&module).expect("a different well-typed payload order is executable");
    assert!(verify_module(&module, &bundle(), &AdmissionProfile::default()).is_err());
}

#[test]
fn selected_case_payload_certificate_rejects_different_root_and_case() {
    for change_root in [false, true] {
        let mut module = payload_module();
        if change_root {
            let machine = &mut module.machines[0];
            let mut parameter = machine.structural_parameters[0].clone();
            parameter.place = PlaceId::new(2).unwrap();
            parameter.position = 1;
            machine.structural_parameters.push(parameter);
            machine.structural_places.push(StructuralPlaceDeclaration {
                id: PlaceId::new(2).unwrap(),
                kind: StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: false,
                },
            });
            let Terminator::StructuralCase { source, .. } = &mut machine.blocks[0].terminator
            else {
                panic!("dispatch")
            };
            *source = PlaceId::new(2).unwrap();
        } else {
            let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
                panic!("sum")
            };
            let mut alternative = cases[0].clone();
            alternative.id = StructuralCaseId::new(2).unwrap();
            alternative.identity = "Alternative".into();
            cases.push(alternative);
            let Terminator::StructuralCase { cases, .. } =
                &mut module.machines[0].blocks[0].terminator
            else {
                panic!("dispatch")
            };
            let mut alternative = cases[0].clone();
            alternative.case = StructuralCaseId::new(2).unwrap();
            alternative.edge = EdgeId::new(11).unwrap();
            // Both cases converge on the same scalar parameter. Their distinct
            // payload namespaces cannot supply one unconditional field equality.
            cases.push(alternative);
        }
        validate_module(&module).expect("well-formed independent identity control");
        assert!(verify_module(&module, &bundle(), &AdmissionProfile::default()).is_err());
        let reconstructed = reconstruct_terminal_obligations(&module).unwrap();
        assert!(
            !reconstructed.obligations()[0]
                .semantic_axioms
                .contains(&Proposition::Equal(term(10), field(1, 1, 2)))
        );
    }
}

fn forwarded_call_module() -> TerminalModule {
    let mut module = payload_module();
    module.machines[0].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let mut callee = unit_module().machines.remove(0);
    callee.id = MachineId::new(901).unwrap();
    callee.contract.id = ContractId::new(901).unwrap();
    callee.structural_parameters = module.machines[0].structural_parameters.clone();
    callee.structural_places = module.machines[0].structural_places.clone();
    callee.structural_parameters[0].place = PlaceId::new(3).unwrap();
    callee.structural_places[0].id = PlaceId::new(3).unwrap();
    callee.entry = BlockId::new(901).unwrap();
    callee.blocks[0].id = callee.entry;
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(901).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    module.machines[0].blocks[1].operations.push(Operation {
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: callee.id,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: PlaceId::new(1).unwrap(),
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(callee);
    let machine = &mut module.machines[0];
    let continuation = Block {
        id: BlockId::new(12).unwrap(),
        parameters: vec![ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(12).unwrap(),
            scalar_type: ScalarType::Integer(integer()),
        }],
        structural_parameters: Vec::new(),
        operations: std::mem::take(&mut machine.blocks[1].operations),
        terminator: Terminator::Return {
            edge: EdgeId::new(21).unwrap(),
            value: ValueId::new(12).unwrap(),
            cleanup_actions: Vec::new(),
        },
    };
    machine.blocks[1].terminator = Terminator::Jump {
        edge: EdgeId::new(20).unwrap(),
        target: continuation.id,
        arguments: vec![ValueId::new(10).unwrap()],
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(continuation);
    module
}

#[test]
fn selected_case_payload_observation_expires_after_mutating_call() {
    let module = forwarded_call_module();
    validate_module(&module).expect("mutable dispatch and ordinary mutable call are valid");
    let reconstructed = reconstruct_terminal_obligations(&module).unwrap();
    assert!(
        !reconstructed.obligations()[0]
            .semantic_axioms
            .contains(&Proposition::Equal(term(10), field(1, 1, 2)))
    );
    assert!(
        !reconstructed.obligations()[0]
            .semantic_axioms
            .contains(&Proposition::Equal(term(12), field(1, 1, 2)))
    );
    assert!(verify_module(&module, &bundle(), &AdmissionProfile::default()).is_err());
}

#[test]
fn selected_case_payload_observation_expires_after_owned_call() {
    let mut module = forwarded_call_module();
    for machine in &mut module.machines {
        machine.structural_parameters[0].access = StructuralAccess::Owned;
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    }
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[2].operations[0].kind
    else {
        panic!("call")
    };
    structural_arguments[0].access = StructuralAccess::Owned;
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut module.machines[1].blocks[0].terminator
    else {
        panic!("unit return")
    };
    trivial_affine_discards.push(PlaceId::new(3).unwrap());
    // The owned callee takes a mutable loan before reading its post-call field.
    let mut mutator = module.machines[1].clone();
    mutator.id = MachineId::new(902).unwrap();
    mutator.contract.id = ContractId::new(902).unwrap();
    mutator.structural_parameters[0].place = PlaceId::new(4).unwrap();
    mutator.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    mutator.structural_places[0].id = PlaceId::new(4).unwrap();
    mutator.entry = BlockId::new(902).unwrap();
    mutator.blocks[0].id = mutator.entry;
    mutator.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(902).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    let callee = &mut module.machines[1];
    callee.blocks[0].operations.push(Operation {
        id: OperationId::new(2).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: mutator.id,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: PlaceId::new(3).unwrap(),
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    callee.blocks[0].terminator = Terminator::StructuralCase {
        source: PlaceId::new(3).unwrap(),
        cases: vec![terminal_psi::StructuralCaseSuccessorEdge {
            edge: EdgeId::new(901).unwrap(),
            target: BlockId::new(903).unwrap(),
            case: StructuralCaseId::new(1).unwrap(),
            payload_fields: vec![StructuralFieldId::new(2).unwrap()],
            trivial_affine_discards: vec![PlaceId::new(3).unwrap()],
        }],
    };
    callee.blocks.push(Block {
        id: BlockId::new(903).unwrap(),
        parameters: vec![ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(110).unwrap(),
            scalar_type: ScalarType::Integer(integer()),
        }],
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Return {
            edge: EdgeId::new(903).unwrap(),
            value: ValueId::new(110).unwrap(),
            cleanup_actions: Vec::new(),
        },
    });
    callee.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(130).unwrap(),
        scalar_type: ScalarType::Integer(integer()),
    });
    callee.contract.ensures.push(ContractClause {
        obligation: ObligationId::new(2).unwrap(),
        proposition: Proposition::Equal(term(130), field(3, 1, 2)),
    });
    let call = &mut module.machines[0].blocks[2].operations[0];
    call.result = OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(40).unwrap(),
        scalar_type: ScalarType::Integer(integer()),
    });
    call.kind = OperationKind::CallStructuralScalar {
        callee: MachineId::new(901).unwrap(),
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }],
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    module.machines.push(mutator);
    validate_module(&module).expect("case payload capture followed by consuming owned call");
    let reconstructed = reconstruct_terminal_obligations(&module).unwrap();
    assert!(
        reconstructed.obligations()[0]
            .semantic_axioms
            .contains(&Proposition::Equal(term(40), field(1, 1, 2))),
        "callee postcondition is imported with the exact consumed root"
    );
    assert!(
        !reconstructed.obligations()[0]
            .semantic_axioms
            .contains(&Proposition::Equal(term(10), field(1, 1, 2))),
        "owned callee may mutate the consumed root before publishing postconditions"
    );
    // Independently check the callee's imported postcondition, not merely its
    // appearance in reconstruction. The caller has no guarantee in this check.
    module.machines[0].contract.ensures.clear();
    let mut evidence = bundle();
    evidence.evidence[0].obligation = ObligationId::new(2).unwrap();
    let EvidenceRoute::CertificateDerived(certificate) = &mut evidence.evidence[0].route else {
        panic!("certificate")
    };
    certificate.proof = ProofNode {
        conclusion: Proposition::Equal(term(130), field(3, 1, 2)),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(130), term(110)),
                rule: ProofRule::SemanticAxiom { index: 2 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(110), field(3, 1, 2)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
        },
    };
    verify_module(&module, &evidence, &AdmissionProfile::default()).expect("owned callee postcondition has an independently checked certificate after its mutable loan");
}

#[test]
fn selected_case_payload_owned_entry_observation_expires_at_mutable_loan() {
    for mutate in [false, true] {
        let mut module = forwarded_call_module();
        let machine = &mut module.machines[0];
        machine.structural_parameters[0].access = StructuralAccess::Owned;
        let zero = ScalarTerm::integer(integer(), IntegerValue::Signed(0)).unwrap();
        let premise = Proposition::Equal(field(1, 1, 2), zero.clone());
        let scalar_premise = Proposition::Equal(term(50), zero);
        machine.parameters.push(ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(50).unwrap(),
            scalar_type: ScalarType::Integer(integer()),
        });
        machine.contract.requires = vec![premise.clone(), scalar_premise.clone()];
        machine.contract.ensures[0].proposition = premise.clone();
        if !mutate {
            machine.blocks[2].operations.clear();
        }
        let reconstructed = reconstruct_terminal_obligations(&module).unwrap();
        let site = &reconstructed.obligations()[0];
        assert!(
            site.requirements.contains(&scalar_premise),
            "SSA-only entry assumptions remain invocation facts"
        );
        assert_eq!(
            site.requirements.contains(&premise) || site.semantic_axioms.contains(&premise),
            !mutate,
            "owned field entry requirement must follow current-root invalidation, mutate={mutate}"
        );
    }
}

#[test]
fn selected_case_payload_observation_expires_at_mixed_structural_result_call() {
    let mut module = forwarded_call_module();
    for machine in &mut module.machines {
        machine.structural_parameters[0].access = StructuralAccess::Owned;
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    }
    let structural_type = StructuralTypeId::new(1).unwrap();
    let callee = &mut module.machines[1];
    callee.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: PlaceId::new(4).unwrap(),
        structural_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    callee.structural_places.push(StructuralPlaceDeclaration {
        id: PlaceId::new(4).unwrap(),
        kind: StructuralPlaceKind::Result,
    });
    callee.blocks[0].terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(901).unwrap(),
        source: PlaceId::new(3).unwrap(),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: PlaceId::new(5).unwrap(),
        kind: StructuralPlaceKind::OperationResult {
            producer: OperationId::new(1).unwrap(),
            structural_type,
        },
    });
    let call = &mut machine.blocks[2].operations[0];
    call.result = OperationResult::Structural(StructuralOperationResult {
        place: PlaceId::new(5).unwrap(),
        structural_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    });
    let structural_arguments = vec![StructuralArgument {
        place: PlaceId::new(1).unwrap(),
        path: Vec::new(),
        access: StructuralAccess::Owned,
    }];
    call.kind = OperationKind::CallStructuralWithScalarArguments {
        callee: MachineId::new(901).unwrap(),
        arguments: Vec::new(),
        structural_arguments,
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    machine.result = TerminalMachineResult::Unit;
    machine.contract.ensures[0].proposition = Proposition::Truth;
    machine.blocks[2].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(21).unwrap(),
        trivial_affine_discards: vec![PlaceId::new(5).unwrap()],
    };
    let reconstructed = reconstruct_terminal_obligations(&module).unwrap();
    assert!(
        !reconstructed.obligations()[0]
            .semantic_axioms
            .contains(&Proposition::Equal(term(10), field(1, 1, 2)))
    );
    module.machines[0].contract.ensures.clear();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("whole owned transfer and returned-result disposal verify");
}
