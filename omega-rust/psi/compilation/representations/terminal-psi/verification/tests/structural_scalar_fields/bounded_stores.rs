use super::{
    CanonicalStructuralPathSegment, IntegerValue, OperationKind, Proposition, ScalarTerm,
    ScalarType, StructuralAccess, StructuralFieldType, StructuralTypeShape, TerminalModule, id,
    integer_type, reconstruct_operation_obligations, structural_scalar_field_module,
    validate_module,
};
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker,
};
use terminal_psi::{
    Block, Operation, OperationResult, SuccessorEdge, Terminator, ValueDeclaration,
};
use terminal_verifier::{ObligationEvidence, ProofBundle, verify_module};

#[test]
fn bounded_store_alone_reconstructs_only_its_selected_edge_guard() {
    for store_on_true in [true, false] {
        let mut module = fixture(StructuralAccess::MutableBorrow);
        let machine = &mut module.machines[0];
        machine.parameters.push(ValueDeclaration {
            qualifications: Default::default(),
            id: id(1),
            scalar_type: integer_type(),
        });
        let store = machine.blocks[0].operations.pop().unwrap();
        let limit = &mut machine.blocks[0].operations[0];
        limit.result.scalar_mut().unwrap().id = id(2);
        limit.kind = OperationKind::IntegerConstant {
            value: IntegerValue::Signed(100),
        };
        machine.blocks[0].operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(3),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(3),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::IntegerLessOrEqual {
                left: id(1),
                right: id(2),
            },
        });
        let successor = |edge, target| SuccessorEdge {
            edge: id(edge),
            target: id(target),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        machine.blocks[0].terminator = Terminator::Conditional {
            condition: id(3),
            when_true: successor(1, if store_on_true { 2 } else { 3 }),
            when_false: successor(2, if store_on_true { 3 } else { 2 }),
        };
        for (block, operations) in [(2, vec![store]), (3, Vec::new())] {
            machine.blocks.push(Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: id(block),
                parameters: Vec::new(),
                operations,
                terminator: Terminator::ReturnUnit {
                    edge: id(block + 1),
                    trivial_affine_discards: Vec::new(),
                },
            });
        }
        let obligations = reconstruct_operation_obligations(&module).unwrap();
        assert_eq!(obligations.len(), 1);
        let ScalarType::Integer(integer) = integer_type() else {
            panic!("integer");
        };
        let limit = ScalarTerm::integer(integer, IntegerValue::Signed(100)).unwrap();
        let guard =
            Proposition::LessOrEqual(ScalarTerm::value(id(1), integer_type()), limit.clone());
        assert_eq!(
            obligations[0].semantic_axioms.contains(&guard),
            store_on_true
        );
        let opposite = Proposition::LessThan(limit, ScalarTerm::value(id(1), integer_type()));
        assert_eq!(
            obligations[0].semantic_axioms.contains(&opposite),
            !store_on_true
        );
        // Selecting an edge only supplies its actual predicate, never proof
        // authority for the unrelated lower endpoint of the bounded field.
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err()
        );
    }
}

fn fixture(access: StructuralAccess) -> TerminalModule {
    let mut module = structural_scalar_field_module();
    module.machines.truncate(1);
    let machine = &mut module.machines[0];
    machine.structural_parameters[0].access = access;
    machine.blocks[0].operations.truncate(2);
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        panic!("record");
    };
    let ScalarType::Integer(integer) = integer_type() else {
        panic!("integer");
    };
    fields[0].field_type = StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            integer,
            IntegerValue::Signed(0),
            IntegerValue::Signed(100),
        )
        .unwrap(),
    );
    let OperationKind::StructuralScalarFieldStore {
        range_obligation, ..
    } = &mut machine.blocks[0].operations[1].kind
    else {
        panic!("store");
    };
    *range_obligation = Some(id(1));
    module
}

#[test]
fn predicate_store_proves_the_replacement_and_rejects_stale_or_missing_evidence() {
    use semantic_vocabulary::ScalarRefinement;
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut module = fixture(access);
        let ScalarType::Integer(integer) = integer_type() else {
            panic!("integer");
        };
        let subject = ScalarTerm::value(ScalarRefinement::SUBJECT, integer_type());
        let zero = ScalarTerm::integer(integer, IntegerValue::Signed(0)).unwrap();
        let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
            panic!("record");
        };
        fields[0].field_type = StructuralFieldType::RefinedScalar(
            ScalarRefinement::new(
                integer_type(),
                Proposition::Disjunction(vec![
                    Proposition::LessThan(subject.clone(), zero.clone()),
                    Proposition::LessThan(zero, subject),
                ]),
            )
            .unwrap(),
        );
        let validated = validate_module(&module).unwrap();
        let context = validated.value_context(&module.machines[0]).unwrap();
        let obligations = reconstruct_operation_obligations(&module).unwrap();
        let obligation = &obligations[0];
        let certificate = proof_admission::produce_predicate_proof(
            &context,
            &obligation.obligation.proposition,
            &[],
            &obligation.semantic_axioms,
        )
        .unwrap();
        let bundle = ProofBundle {
            evidence: vec![ObligationEvidence {
                obligation: id(1),
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: id(1),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: certificate,
                }),
            }],
            ..ProofBundle::default()
        };
        verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err()
        );
        let mut invalid = module.clone();
        invalid.machines[0].blocks[0].operations[0].kind = OperationKind::IntegerConstant {
            value: IntegerValue::Signed(0),
        };
        assert!(verify_module(&invalid, &bundle, &AdmissionProfile::default()).is_err());
        let OperationKind::StructuralScalarFieldStore {
            range_obligation, ..
        } = &mut module.machines[0].blocks[0].operations[1].kind
        else {
            panic!("store");
        };
        *range_obligation = None;
        assert!(validate_module(&module).is_err());
    }
}

fn proof(module: &TerminalModule) -> ProofBundle {
    let obligations = reconstruct_operation_obligations(module).unwrap();
    let [obligation] = obligations.as_slice() else {
        panic!("one range obligation");
    };
    let ScalarType::Integer(integer) = integer_type() else {
        panic!("integer");
    };
    let value = ScalarTerm::value(id(1), integer_type());
    let constant = ScalarTerm::integer(integer, IntegerValue::Signed(99)).unwrap();
    let equality = Proposition::Equal(value.clone(), constant.clone());
    let axiom = obligation
        .semantic_axioms
        .iter()
        .position(|candidate| candidate == &equality)
        .unwrap();
    let Proposition::Conjunction(clauses) = &obligation.obligation.proposition else {
        panic!("range");
    };
    let proofs = clauses
        .iter()
        .map(|clause| {
            let Proposition::LessOrEqual(left, right) = clause else {
                panic!("endpoint");
            };
            let endpoint = usize::from(*left != value);
            let closed = Proposition::LessOrEqual(
                if *left == value {
                    constant.clone()
                } else {
                    left.clone()
                },
                if *right == value {
                    constant.clone()
                } else {
                    right.clone()
                },
            );
            ProofNode {
                conclusion: clause.clone(),
                rule: ProofRule::IntegerOrderSubstitution {
                    relation: Box::new(ProofNode {
                        conclusion: closed,
                        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                    }),
                    equality: Box::new(ProofNode {
                        conclusion: equality.clone(),
                        rule: ProofRule::SemanticAxiom { index: axiom },
                    }),
                    endpoint,
                },
            }
        })
        .collect();
    ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: id(1),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: obligation.obligation.proposition.clone(),
                    rule: ProofRule::ConjunctionIntroduction(proofs),
                },
            }),
        }],
        ..ProofBundle::default()
    }
}

#[test]
fn bounded_store_proves_range_before_write_for_mutable_and_write_only_roots() {
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let module = fixture(access);
        let bundle = proof(&module);
        verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
        let obligations = reconstruct_operation_obligations(&module).unwrap();
        assert!(obligations[0].semantic_axioms.iter().all(|axiom| !matches!(
            axiom,
            Proposition::Equal(ScalarTerm::IntegerField { .. }, _)
        )));
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err()
        );
        let mut invalid = module.clone();
        invalid.machines[0].blocks[0].operations[0].kind = OperationKind::IntegerConstant {
            value: IntegerValue::Signed(101),
        };
        assert!(verify_module(&invalid, &bundle, &AdmissionProfile::default()).is_err());
    }
}

#[test]
fn bounded_store_rejects_missing_wrong_and_unbounded_obligations() {
    let module = fixture(StructuralAccess::WriteOnlyBorrow);
    let bundle = proof(&module);
    for obligation in [None, Some(id(2))] {
        let mut invalid = module.clone();
        let OperationKind::StructuralScalarFieldStore {
            range_obligation, ..
        } = &mut invalid.machines[0].blocks[0].operations[1].kind
        else {
            panic!("store");
        };
        *range_obligation = obligation;
        assert!(verify_module(&invalid, &bundle, &AdmissionProfile::default()).is_err());
    }
    let mut invalid = module.clone();
    let StructuralTypeShape::Record { fields } = &mut invalid.structural_types[1].shape else {
        panic!("record");
    };
    fields[0].field_type = StructuralFieldType::Scalar(integer_type());
    assert!(validate_module(&invalid).is_err());
    let mut invalid = module;
    let StructuralTypeShape::Record { fields } = &mut invalid.structural_types[1].shape else {
        panic!("record");
    };
    let StructuralFieldType::BoundedInteger(bounds) = fields[0].field_type else {
        panic!("bounds");
    };
    fields[0].field_type = StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            bounds.integer_type(),
            IntegerValue::Signed(0),
            IntegerValue::Signed(98),
        )
        .unwrap(),
    );
    assert!(verify_module(&invalid, &bundle, &AdmissionProfile::default()).is_err());
}

#[test]
fn bounded_write_only_store_does_not_authorize_a_field_read() {
    let mut module = fixture(StructuralAccess::WriteOnlyBorrow);
    let machine = &mut module.machines[0];
    let mut read = machine.blocks[0].operations[0].clone();
    read.id = id(3);
    read.result.scalar_mut().unwrap().id = id(3);
    read.kind = OperationKind::IntegerStructuralField {
        source: machine.structural_parameters[0].place,
        path: vec![CanonicalStructuralPathSegment::Field(id(1))],
        field: id(1),
    };
    machine.blocks[0].operations.push(read);
    assert!(validate_module(&module).is_err());
}
