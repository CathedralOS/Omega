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
use terminal_verifier::{ObligationEvidence, ProofBundle, verify_module};

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
