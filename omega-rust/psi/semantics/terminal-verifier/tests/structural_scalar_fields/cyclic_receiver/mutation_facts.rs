//! Mutation invalidates current structural facts without changing captured scalar values.

use super::*;
use terminal_verifier::VerificationError;

#[test]
fn stores_and_mutating_calls_forget_old_boolean_observations() {
    // Acyclic controls isolate the stale-fact bug from cyclic eligibility.
    for cyclic in [false, true] {
        for mutation in [12, 13] {
            let mut module = receiver_module(cyclic);
            module.machines[0].blocks[0].operations.retain(|operation| {
                operation.id != id::<OperationId>(if mutation == 12 { 13 } else { 12 })
                    && !matches!(operation.id.get(), 16 | 17)
            });
            let axioms = exit_axioms(&mut module);
            assert!(
                !axioms.contains(&equation(10, 1, 1)),
                "cyclic={cyclic}, mutation={mutation}"
            );
            assert!(axioms.contains(&equation(14, 1, 1)));
            assert!(
                axioms.contains(&Proposition::Equal(
                    ScalarTerm::value(id(11), ScalarType::Boolean),
                    ScalarTerm::Boolean(false),
                )),
                "a stored SSA value remains its captured value"
            );
        }
    }
}

fn premise_certificate(
    module: &TerminalModule,
    obligation: u64,
    conclusion: Proposition,
) -> ObligationEvidence {
    let questions = reconstruct_terminal_obligations(module).unwrap();
    let site = questions
        .obligations()
        .iter()
        .find(|site| site.obligation.id == id(obligation))
        .expect("the exact proof site");
    let rule = if let Some(index) = site
        .requirements
        .iter()
        .position(|fact| fact == &conclusion)
    {
        ProofRule::Assumption { index }
    } else {
        ProofRule::SemanticAxiom {
            index: site
                .semantic_axioms
                .iter()
                .position(|fact| fact == &conclusion)
                .expect("the unchanged receiver retains the exact entry premise"),
        }
    };
    certificate(obligation, conclusion, rule)
}

fn integer_constant(operation: u64, value: i128) -> Operation {
    Operation {
        static_reach_binding: None,
        id: id(operation),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(operation),
            scalar_type: integer_type(),
        }),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Signed(value),
        },
    }
}

fn integer_store(operation: u64, destination: u64, value: u64) -> Operation {
    let mut operation = store(operation, destination, value);
    let OperationKind::StructuralScalarFieldStore { field, .. } = &mut operation.kind else {
        unreachable!()
    };
    *field = id(3);
    operation
}

fn integer_field_is_zero(root: u64) -> Proposition {
    let ScalarType::Integer(integer) = integer_type() else {
        unreachable!()
    };
    Proposition::Equal(
        ScalarTerm::integer_field_path(
            id(root),
            vec![CanonicalStructuralPathSegment::Field(id(3))],
            integer,
        ),
        ScalarTerm::integer(integer, IntegerValue::Signed(0)).unwrap(),
    )
}

#[test]
fn integer_guarantees_expire_at_stores_and_mutating_calls() {
    for through_call in [false, true] {
        let mut module = receiver_module(false);
        // A nonmutating callee legitimately preserves the entry fact field_3=0.
        // Its requirement and guarantee both have explicit assumption proofs.
        module.machines[1].blocks[0].operations.clear();
        module.machines[1].contract.requires = vec![integer_field_is_zero(2)];
        module.machines[1].contract.ensures = vec![ContractClause {
            obligation: id(91),
            proposition: integer_field_is_zero(2),
        }];
        let mut mutator = module.machines[1].clone();
        mutator.id = id(3);
        mutator.contract = contract(3);
        mutator.structural_parameters[0].place = id(3);
        mutator.structural_places = vec![structural_place(id(3))];
        mutator.entry = id(4);
        mutator.blocks[0].id = id(4);
        mutator.blocks[0].operations = vec![integer_constant(30, 1), integer_store(31, 3, 30)];
        mutator.blocks[0].terminator = Terminator::ReturnUnit {
            edge: id(4),
            trivial_affine_discards: Vec::new(),
        };
        module.machines.push(mutator);
        let mut preserving_call = call();
        let OperationKind::CallUnit {
            requirement_obligations,
            ..
        } = &mut preserving_call.kind
        else {
            unreachable!()
        };
        requirement_obligations.push(id(80));
        let caller = &mut module.machines[0];
        caller.blocks[0].operations = vec![preserving_call, integer_constant(11, 1)];
        caller.contract.requires = vec![integer_field_is_zero(1)];
        caller.contract.ensures = vec![ContractClause {
            obligation: id(90),
            proposition: integer_field_is_zero(1),
        }];
        let questions = reconstruct_terminal_obligations(&module).unwrap();
        let guarantee = questions
            .obligations()
            .iter()
            .find(|site| site.obligation.id == id(90))
            .unwrap();
        let imported_position = guarantee
            .semantic_axioms
            .iter()
            .position(|fact| fact == &integer_field_is_zero(1))
            .expect("callee imports field_3=0");
        let bundle = ProofBundle {
            evidence: vec![
                premise_certificate(&module, 80, integer_field_is_zero(1)),
                certificate(
                    90,
                    integer_field_is_zero(1),
                    ProofRule::SemanticAxiom {
                        index: imported_position,
                    },
                ),
                premise_certificate(&module, 91, integer_field_is_zero(2)),
            ],
            ..ProofBundle::default()
        };
        verify_module_for_interpretation(&module, &bundle, &AdmissionProfile::default())
            .expect("the preserving call and its imported integer guarantee verify");

        let mutation = if through_call {
            let mut invoke = call();
            invoke.id = id(32);
            let OperationKind::CallUnit { callee, .. } = &mut invoke.kind else {
                unreachable!()
            };
            *callee = id(3);
            invoke
        } else {
            integer_store(12, 1, 11)
        };
        module.machines[0].blocks[0].operations.push(mutation);
        let questions = reconstruct_terminal_obligations(&module).unwrap();
        let guarantee = questions
            .obligations()
            .iter()
            .find(|site| site.obligation.id == id(90))
            .unwrap();
        assert!(
            !guarantee
                .semantic_axioms
                .contains(&integer_field_is_zero(1)),
            "the same integer field is now 1, through_call={through_call}"
        );
        let result =
            verify_module_for_interpretation(&module, &bundle, &AdmissionProfile::default());
        assert!(
            matches!(result, Err(VerificationError::RejectedEvidence { obligation, .. })
            if obligation == id(90)),
            "stale imported certificate, through_call={through_call}: {result:?}"
        );
    }
}

fn require_stale_entry_rejection(through_call: bool) {
    let mut module = receiver_module(false);
    let proposition = Proposition::Equal(
        ScalarTerm::boolean_field(id(1), id(1)),
        ScalarTerm::Boolean(true),
    );
    module.machines[0].blocks[0].operations = vec![boolean_constant(11, false)];
    module.machines[0].contract.requires = vec![proposition.clone()];
    module.machines[0].contract.ensures = vec![ContractClause {
        obligation: id(90),
        proposition: proposition.clone(),
    }];
    module.machines[1].blocks[0].operations = vec![boolean_constant(20, false), store(21, 2, 20)];
    let bundle = ProofBundle {
        evidence: vec![premise_certificate(&module, 90, proposition)],
        ..ProofBundle::default()
    };
    verify_module_for_interpretation(&module, &bundle, &AdmissionProfile::default())
        .expect("entry truth legitimately remains true when the receiver is unchanged");
    module.machines[0].blocks[0]
        .operations
        .push(if through_call {
            call()
        } else {
            store(12, 1, 11)
        });
    // The entry satisfies field_1=true, but this write makes the same field false.
    // Filtering semantic axioms alone must not leave the entry Assumption usable
    // as a current-field guarantee.
    let result = verify_module_for_interpretation(&module, &bundle, &AdmissionProfile::default());
    assert!(
        result.is_err(),
        "accepted a false post-write guarantee from an entry assumption, through_call={through_call}: {result:?}"
    );
}

#[test]
fn acyclic_store_cannot_reuse_mutable_entry_requirement_as_exit_fact() {
    require_stale_entry_rejection(false);
}

#[test]
fn acyclic_mutating_call_cannot_reuse_mutable_entry_requirement_as_exit_fact() {
    require_stale_entry_rejection(true);
}
