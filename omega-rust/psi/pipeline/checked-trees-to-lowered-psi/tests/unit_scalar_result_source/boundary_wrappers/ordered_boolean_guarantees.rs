use super::{
    artifact, boolean_guarantee_source, execute, integer, nested_boolean_guarantee_source,
    normal_guarantee_source,
};
use crate::unit_scalar_result_source::{CheckedScalarExpression, checked_from_source};
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue};

#[test]
fn ordered_scalar_completion_proves_normal_result_guarantees() {
    for body in [
        "Host::finish(11); value",
        "let observed: i32 = Host::measure(11); value",
        "Host::finish(11); identity(value)",
    ] {
        let source = normal_guarantee_source(body);
        let artifact = artifact(&checked_from_source(&source));
        let module = decode_module(&artifact.0).unwrap();
        let wrapper = module
            .machines
            .iter()
            .find(|machine| {
                !machine.contract.ensures.is_empty() && !machine.contract.requires.is_empty()
            })
            .expect("normal guarantee published");
        assert_eq!(wrapper.contract.ensures.len(), 1);
        let (status, observed) = execute(&artifact);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(observed.arguments, [vec![integer(11)], vec![integer(70)]]);
    }
}

#[test]
fn ordered_boolean_completion_preserves_normal_result_guarantees() {
    for body in [
        "Host::finish(false); value",
        "Host::finish(false); identity(value)",
        "let observed: bool = Host::measure(false); value",
    ] {
        for input in [false, true] {
            let source = boolean_guarantee_source(body, input);
            let artifact = artifact(&checked_from_source(&source));
            let module = decode_module(&artifact.0).unwrap();
            assert!(
                module
                    .machines
                    .iter()
                    .any(|machine| !machine.contract.ensures.is_empty())
            );
            let (status, observed) = execute(&artifact);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(input)]
                ]
            );
        }
    }
}

#[test]
fn ordered_boolean_completion_preserves_folded_source_meaning() {
    for input in [false, true] {
        for (expression, guarantee, expected) in [
            ("!false", "result == !false", true),
            ("true && value", "result == value", input),
            ("false || value", "result == value", input),
            ("value == true", "result == (value == true)", input),
            ("!(!value)", "result == !(!value)", input),
            ("true || value", "result == true", true),
            ("false && value", "result == false", false),
            ("1u8 < 2u8", "result == (1u8 < 2u8)", true),
            ("2u16 > 1u16", "result == (2u16 > 1u16)", true),
            ("1i32 >= 2i32", "result == (1i32 >= 2i32)", false),
            ("2i16 <= 2i16", "result == (2i16 <= 2i16)", true),
            ("1u32 != 2u32", "result == (1u32 != 2u32)", true),
            ("1u8 == 2u8", "result == (1u8 == 2u8)", false),
        ] {
            let source =
                boolean_guarantee_source(&format!("Host::finish(false); {expression}"), input)
                    .replace(
                        "ensures result == value\nreaches Host",
                        &format!("ensures {guarantee}\nreaches Host"),
                    );
            let published = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [false, expected].map(|value| vec![TerminalScalarValue::Boolean(value)])
            );
        }
    }
}

#[test]
fn literal_boolean_guards_still_require_every_arrival_proof() {
    for body in ["true && value", "false || value"] {
        let source = boolean_guarantee_source(&format!("Host::finish(false); {body}"), true);
        let published = artifact(&checked_from_source(&source));
        let module = decode_module(&published.0).unwrap();
        let proof = decode_proof_bundle(&published.1).unwrap();
        assert_eq!(module.scalar_block_invariants.len(), 1);
        let invariant = &module.scalar_block_invariants[0];
        let obligations = terminal_verifier::reconstruct_operation_obligations(&module).unwrap();
        assert!(obligations.iter().any(|obligation| {
            matches!(obligation.owner,
                terminal_verifier::ReconstructedTerminalObligationOwner::ScalarBlockInvariant { .. })
                && obligation.semantic_axioms.contains(&semantic_vocabulary::Proposition::Falsehood)
        }), "impossible arrival retains its own proof question");
        for arrival in &invariant.arrivals {
            let mut changed = proof.clone();
            changed
                .evidence
                .retain(|evidence| evidence.obligation != arrival.obligation);
            assert_eq!(changed.evidence.len() + 1, proof.evidence.len());
            assert!(
                terminal_verifier::verify_module(&module, &changed, &AdmissionProfile::default(),)
                    .is_err()
            );
        }
    }
}

#[test]
fn literal_comparison_guarantees_reject_changed_source_meaning() {
    use checked_trees::{CheckedBooleanExpression as Boolean, ClosedScalarContractValue as Clause};
    use numerics::literals::{IntegerLanding, IntegerLiteral, LandedIntegerType};

    let source = boolean_guarantee_source("Host::finish(false); 1u8 < 2u8", true).replace(
        "ensures result == value\nreaches Host",
        "ensures result == (1u8 < 2u8)\nreaches Host",
    );
    let original = checked_from_source(&source);
    artifact(&original);
    let target = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Scalar::measure")
        .unwrap()
        .symbol;
    for mutation in 0..3 {
        let mut changed = original.clone();
        let contract = changed
            .facts
            .contract_plans
            .machines
            .iter_mut()
            .find(|contract| contract.machine == target)
            .unwrap();
        let mut guarantees = contract.closed_scalar_values.ensures().to_vec();
        let Some(Clause::Predicate(Boolean::Equal { right, .. })) = &mut guarantees[0] else {
            panic!("result equality");
        };
        let Boolean::IntegerComparison { kind, left, .. } = right.as_mut() else {
            panic!("retained comparison");
        };
        match mutation {
            0 | 1 => {
                **left = CheckedScalarExpression::IntegerLiteral {
                    literal: IntegerLiteral::from_value(if mutation == 0 { 0 } else { 1 })
                        .with_landing(IntegerLanding {
                            landed_type: if mutation == 0 {
                                LandedIntegerType::U8
                            } else {
                                LandedIntegerType::U16
                            },
                            domain: numerics::arithmetic::ArithmeticDomain::Exact,
                        }),
                };
            }
            2 => *kind = checked_trees::CheckedIntegerComparisonKind::LessOrEqual,
            _ => unreachable!(),
        }
        contract.closed_scalar_values = checked_trees::ClosedScalarValueContractPlan::new(
            contract.closed_scalar_values.requires().to_vec(),
            guarantees,
            contract.closed_scalar_values.has_crash_clauses(),
            contract.closed_scalar_values.has_outcome_specific_clauses(),
        );
        // Changed value/kind can still denote true, but semantic agreement is
        // not custody of the authored operator and its exact literal operands.
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ordered_computed_boolean_result_proves_its_normal_guarantee() {
    for (body, guarantee) in [
        ("Host::finish(false); !value", "result == !value"),
        (
            "let observed: bool = Host::measure(false); !value",
            "!value == result",
        ),
    ] {
        for input in [false, true] {
            let source = boolean_guarantee_source(body, input).replace(
                "ensures result == value\nreaches Host",
                &format!("ensures {guarantee}\nreaches Host"),
            );
            let artifact = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&artifact);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(!input)]
                ]
            );
        }
    }
}

#[test]
fn ordered_call_produced_boolean_result_preserves_its_normal_guarantee() {
    for input in [false, true] {
        for body in [
            "let saved: bool = identity(!value); Host::finish(false); saved",
            "Host::finish(false); let saved: bool = identity(!value); saved",
            "let earlier: bool = !value; let saved: bool = identity(earlier); Host::finish(false); saved",
            "let first: bool = identity(value); Host::finish(false); let saved: bool = identity(!first); saved",
        ] {
            let source = boolean_guarantee_source(body, input).replace(
                "ensures result == value\nreaches Host",
                "ensures result == !value\nreaches Host",
            );
            let published = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(!input)]
                ]
            );
        }
    }
}

#[test]
fn ordered_boolean_call_computations_preserve_normal_guarantees() {
    for input in [false, true] {
        for body in [
            "Host::finish(false); identity(!identity(value))",
            "let saved: bool = identity(!identity(value)); Host::finish(false); saved",
            "Host::finish(false); !identity(value)",
            "Host::finish(false); identity(value) == false",
        ] {
            let source = boolean_guarantee_source(body, input).replace(
                "ensures result == value\nreaches Host",
                "ensures result == !value\nreaches Host",
            );
            let published = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(!input)],
                ],
                "{body}"
            );
        }
    }
}

#[test]
fn ordered_boolean_call_computations_preserve_nested_effect_order() {
    for input in [false, true] {
        let source = boolean_guarantee_source("Host::finish(false); echo(!echo(value))", input)
            .replace(
                "ensures result == value\nreaches Host",
                "ensures result == !value\nreaches Host",
            );
        let source = format!(
            "machine echo(value: bool) -> bool ensures result == value\nreaches Host {{ Host::finish(value); value }}\n{source}"
        );
        let published = artifact(&checked_from_source(&source));
        let (status, observed) = execute(&published);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(
            observed.arguments,
            [false, input, !input, !input].map(|value| vec![TerminalScalarValue::Boolean(value)])
        );
    }
}

#[test]
fn ordered_boolean_call_computations_reject_an_altered_operation() {
    let source = boolean_guarantee_source("Host::finish(false); !identity(value)", false).replace(
        "ensures result == value\nreaches Host",
        "ensures result == !value\nreaches Host",
    );
    let published = artifact(&checked_from_source(&source));
    let mut module = decode_module(&published.0).unwrap();
    let proof = decode_proof_bundle(&published.1).unwrap();
    let operation = module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::BooleanNot { .. }
            )
        })
        .expect("selected negation survives publication");
    operation.kind = terminal_psi::OperationKind::BooleanConstant { value: false };
    assert!(
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).is_err()
    );
}

#[test]
fn ordered_boolean_call_computations_prove_branch_guarantees() {
    for input in [false, true] {
        for body in [
            "Host::finish(false); identity(false) || !identity(value)",
            "Host::finish(false); identity(!value) && identity(true)",
            "Host::finish(false); !(identity(value) || identity(false))",
            "Host::finish(false); identity(!(identity(value) && identity(true)))",
        ] {
            let source = boolean_guarantee_source(body, input).replace(
                "ensures result == value\nreaches Host",
                "ensures result == !value\nreaches Host",
            );
            let checked = checked_from_source(&source);
            let published = artifact(&checked);
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(!input)]
                ],
                "{body}"
            );
        }
    }
}

#[test]
fn ordered_boolean_guarantees_compose_through_dependent_joins() {
    for input in [false, true] {
        for body in [
            "Host::finish(false); (identity(value) && identity(true)) || identity(false)",
            "Host::finish(false); (identity(value) || identity(false)) && identity(true)",
            "Host::finish(false); ((identity(value) && identity(true)) || identity(false)) && identity(true)",
            "let saved: bool = identity(value) && identity(true); Host::finish(false); identity(saved) || identity(false)",
        ] {
            let source = boolean_guarantee_source(body, input);
            let published = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [false, input].map(|value| vec![TerminalScalarValue::Boolean(value)]),
                "{body}"
            );
        }
    }
}

#[test]
fn dependent_boolean_joins_retain_independent_input_values() {
    for value in [false, true] {
        for spare in [false, true] {
            let source = nested_boolean_guarantee_source(
                "value && spare",
                "Host::finish(false);",
                [value, spare, false, false],
            )
            .replace(
                "Host::finish(false); value && spare",
                "Host::finish(false); (identity(value) && identity(spare)) || identity(false)",
            );
            let published = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [false, value && spare].map(|value| vec![TerminalScalarValue::Boolean(value)])
            );
        }
    }
}

#[test]
fn dependent_boolean_join_evidence_rejects_changed_guards_and_missing_arrivals() {
    let source = boolean_guarantee_source(
        "Host::finish(false); (identity(value) && identity(true)) || identity(false)",
        true,
    );
    let published = artifact(&checked_from_source(&source));
    let module = decode_module(&published.0).unwrap();
    let proof = decode_proof_bundle(&published.1).unwrap();
    assert_eq!(
        module.scalar_block_invariants.len(),
        2,
        "both dependent joins need evidence"
    );
    for index in 0..module.scalar_block_invariants.len() {
        let mut changed = module.clone();
        changed.scalar_block_invariants[index].arrivals.pop();
        assert!(
            terminal_verifier::verify_module(&changed, &proof, &AdmissionProfile::default())
                .is_err()
        );
    }
    let mut changed = module.clone();
    let predicate = &mut changed.scalar_block_invariants[1].predicate;
    let semantic_vocabulary::Proposition::Conjunction(parts) = predicate else {
        panic!("conditional demands")
    };
    let semantic_vocabulary::Proposition::Implication { premise, .. } = &mut parts[0] else {
        panic!("path guard")
    };
    **premise = semantic_vocabulary::Proposition::Truth;
    assert!(
        terminal_verifier::verify_module(&changed, &proof, &AdmissionProfile::default()).is_err()
    );
    let mut changed = module;
    changed.scalar_block_invariants.pop();
    assert!(
        terminal_verifier::verify_module(&changed, &proof, &AdmissionProfile::default()).is_err()
    );
}

#[test]
fn ordered_call_produced_boolean_equations_compose_across_calls() {
    for bits in 0..16 {
        let inputs = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0];
        let source = nested_boolean_guarantee_source(
            "(value == spare) == (other == last)", "Host::finish(false);", inputs,
        ).replace(
            "Host::finish(false); (value == spare) == (other == last)",
            "let first: bool = equal(9u16, value, spare); Host::finish(false); let second: bool = equal(5u16, other, last); first == second",
        );
        let source = format!(
            "machine equal(marker: u16, left: bool, right: bool) -> bool ensures result == (left == right) {{ left == right }}\n{source}"
        );
        let published = artifact(&checked_from_source(&source));
        let (status, observed) = execute(&published);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(
            observed.arguments,
            [
                vec![TerminalScalarValue::Boolean(false)],
                vec![TerminalScalarValue::Boolean(
                    (inputs[0] == inputs[1]) == (inputs[2] == inputs[3])
                )]
            ]
        );
    }
}

#[test]
fn ordered_call_produced_boolean_result_rejects_a_substituted_actual() {
    let source = boolean_guarantee_source(
        "let saved: bool = identity(!value); Host::finish(false); saved",
        true,
    )
    .replace(
        "ensures result == value\nreaches Host",
        "ensures result == !value\nreaches Host",
    );
    let published = artifact(&checked_from_source(&source));
    let mut module = decode_module(&published.0).unwrap();
    let proof = decode_proof_bundle(&published.1).unwrap();
    let wrapper = module
        .machines
        .iter_mut()
        .find(|machine| {
            !machine.contract.ensures.is_empty()
                && machine.blocks.iter().any(|block| {
                    block.operations.iter().any(|operation| {
                        matches!(operation.kind, terminal_psi::OperationKind::Call { .. })
                    })
                })
        })
        .unwrap();
    let input = wrapper.parameters[0].id;
    let arguments = wrapper
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            terminal_psi::OperationKind::Call { arguments, .. } => Some(arguments),
            _ => None,
        })
        .unwrap();
    assert_ne!(
        arguments[0], input,
        "the call consumes the captured negation"
    );
    arguments[0] = input;
    assert!(
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).is_err()
    );
}

#[test]
fn ordered_saved_boolean_result_preserves_its_normal_guarantee() {
    for input in [false, true] {
        for body in [
            "let saved: bool = !value; Host::finish(false); saved",
            "Host::finish(false); let saved: bool = !value; saved",
            "let saved: bool = value; Host::finish(false); !saved",
            "let saved: bool = !value; Host::finish(false); let copied: bool = saved; copied",
            "let observed: bool = Host::measure(false); let saved: bool = !value; saved",
        ] {
            let source = boolean_guarantee_source(body, input).replace(
                "ensures result == value\nreaches Host",
                "ensures result == !value\nreaches Host",
            );
            let published = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(!input)]
                ]
            );
        }
    }
}

#[test]
fn ordered_mutable_boolean_snapshot_preserves_its_normal_guarantee() {
    for body in [
        "let mut current: bool = value; let saved: bool = current; current = !current; Host::finish(current); saved",
        "let mut current: bool = !value; current = !current; let saved: bool = current; Host::finish(!current); saved",
        "let mut current: bool = value; let mut other: bool = !value; let saved: bool = current; other = !other; current = !current; Host::finish(current); saved",
    ] {
        for input in [false, true] {
            let source = boolean_guarantee_source(body, input);
            let published = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [!input, input].map(|value| vec![TerminalScalarValue::Boolean(value)])
            );
        }
    }
}

#[test]
fn ordered_saved_boolean_computations_compose_across_boundary_effects() {
    for bits in 0..16 {
        let inputs = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0];
        let source = nested_boolean_guarantee_source(
            "(value == spare) == (other == last)", "Host::finish(false);", inputs,
        ).replace(
            "Host::finish(false); (value == spare) == (other == last)",
            "let first: bool = value == spare; Host::finish(false); let second: bool = other == last; first == second",
        );
        let published = artifact(&checked_from_source(&source));
        let (status, observed) = execute(&published);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(
            observed.arguments,
            [
                vec![TerminalScalarValue::Boolean(false)],
                vec![TerminalScalarValue::Boolean(
                    (inputs[0] == inputs[1]) == (inputs[2] == inputs[3])
                )]
            ]
        );
    }
}

#[test]
fn ordered_saved_boolean_return_rejects_a_substituted_terminal_value() {
    let source =
        boolean_guarantee_source("let saved: bool = !value; Host::finish(false); saved", true)
            .replace(
                "ensures result == value\nreaches Host",
                "ensures result == !value\nreaches Host",
            );
    let published = artifact(&checked_from_source(&source));
    let mut module = decode_module(&published.0).unwrap();
    let proof = decode_proof_bundle(&published.1).unwrap();
    let wrapper = module
        .machines
        .iter_mut()
        .find(|machine| !machine.contract.ensures.is_empty())
        .unwrap();
    let input = wrapper.parameters[0].id;
    let returned = wrapper
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            terminal_psi::Terminator::Return { value, .. } => Some(value),
            _ => None,
        })
        .expect("saved scalar return");
    assert_ne!(
        *returned, input,
        "execution returns the computed value, not an input replay"
    );
    *returned = input;
    assert!(
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).is_err()
    );
}

#[test]
fn ordered_nested_boolean_result_proves_its_normal_guarantee() {
    for bits in 0..16 {
        let inputs = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0];
        let expected = (inputs[0] == inputs[1]) == (inputs[2] == inputs[3]);
        for (expression, before, expected) in [
            (
                "(value == spare) == (other == last)",
                "Host::finish(false);",
                expected,
            ),
            (
                "!((value == spare) == (other == last))",
                "let observed: bool = Host::measure(false);",
                !expected,
            ),
        ] {
            let source = nested_boolean_guarantee_source(expression, before, inputs);
            let published = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(expected)]
                ]
            );
        }
    }
}

#[test]
fn nested_boolean_return_requires_exact_operations_and_carried_equations() {
    let source = nested_boolean_guarantee_source(
        "(value == spare) == (other == last)",
        "Host::finish(false);",
        [true, false, false, true],
    );
    let published = artifact(&checked_from_source(&source));
    let module = decode_module(&published.0).unwrap();
    let proof = decode_proof_bundle(&published.1).unwrap();
    let mut changed_proof = proof.clone();
    let equalities = changed_proof
        .evidence
        .iter_mut()
        .find_map(|evidence| {
            let proof_admission::EvidenceRoute::CertificateDerived(certificate) =
                &mut evidence.route
            else {
                return None;
            };
            match &mut certificate.proof.rule {
                proof_admission::ProofRule::ValueEqualityTransport { equalities, .. } => {
                    Some(equalities)
                }
                _ => None,
            }
        })
        .expect("nested return carries explicit equation transport");
    assert!(!equalities.is_empty());
    assert!(equalities.iter().all(|equality| matches!(
        equality.rule,
        proof_admission::ProofRule::SemanticAxiom { .. }
    )));
    equalities.clear();
    assert!(
        terminal_verifier::verify_module(&module, &changed_proof, &AdmissionProfile::default())
            .is_err()
    );

    let mut changed_module = module.clone();
    let wrapper = changed_module
        .machines
        .iter_mut()
        .find(|machine| !machine.contract.ensures.is_empty())
        .unwrap();
    let changed_operation = wrapper
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::BooleanEqual { .. }
            )
        })
        .unwrap();
    let terminal_psi::OperationKind::BooleanEqual { left, right } = &mut changed_operation.kind
    else {
        unreachable!()
    };
    *right = *left;
    assert!(
        terminal_verifier::verify_module(&changed_module, &proof, &AdmissionProfile::default())
            .is_err()
    );
}

#[test]
fn boolean_result_equation_preserves_closed_entry_fact_proofs() {
    let source = boolean_guarantee_source("Host::finish(false); true", false)
        .replace("requires value == value", "requires !value")
        .replace(
            "ensures result == value\nreaches Host",
            "ensures result == !value\nreaches Host",
        );
    let published = artifact(&checked_from_source(&source));
    let (status, observed) = execute(&published);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        observed.arguments,
        [
            vec![TerminalScalarValue::Boolean(false)],
            vec![TerminalScalarValue::Boolean(true)]
        ]
    );
}

#[test]
fn ordered_computed_boolean_equality_keeps_mixed_parameter_identities() {
    for input in [false, true] {
        for spare in [false, true] {
            let source = boolean_guarantee_source("Host::finish(false); value == spare", input)
                .replace(
                    "Scalar::measure(value: bool)",
                    "Scalar::measure(marker: u16, spare: bool, value: bool)",
                )
                .replace(
                    &format!("Scalar::measure({input})"),
                    &format!("Scalar::measure(9u16, {spare}, {input})"),
                )
                .replace(
                    "ensures result == value\nreaches Host",
                    "ensures result == (value == spare)\nreaches Host",
                );
            let published = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(input == spare)]
                ]
            );
        }
    }
}

#[test]
fn computed_boolean_guarantees_reject_changed_return_and_selected_evidence() {
    use checked_trees::{CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar};
    let source = boolean_guarantee_source("Host::finish(false); !value", false).replace(
        "ensures result == value\nreaches Host",
        "ensures result == !value\nreaches Host",
    );
    let original = checked_from_source(&source);
    let published = artifact(&original);
    let proof = decode_proof_bundle(&published.1).unwrap();
    let mut module = decode_module(&published.0).unwrap();
    let machine = module
        .machines
        .iter_mut()
        .find(|machine| !machine.contract.ensures.is_empty())
        .unwrap();
    let input = machine.parameters[0].id;
    let block = machine
        .blocks
        .iter_mut()
        .find(|block| matches!(block.terminator, terminal_psi::Terminator::Return { .. }))
        .unwrap();
    let terminal_psi::Terminator::Return { value, .. } = &mut block.terminator else {
        unreachable!()
    };
    *value = input;
    assert!(
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).is_err()
    );

    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Scalar::measure")
        .unwrap();
    let state = original.machine_states(machine)[0].symbol;
    for mutation in 0..4 {
        let mut changed = original.clone();
        let plans = &mut changed.facts.values.scalar_expressions;
        let position = plans.expressions.iter().position(|plan| plan.state == state && matches!(&plan.expression, Scalar::Boolean(expression) if matches!(expression.as_ref(), Boolean::Not(_)))).unwrap();
        match mutation {
            0 => {
                plans.expressions[position].expression =
                    Scalar::Boolean(Box::new(Boolean::Parameter { position: 0 }))
            }
            1 => plans.expressions.push(plans.expressions[position].clone()),
            2 => {
                plans.expressions[position].role =
                    checked_trees::CheckedScalarExpressionRole::ContinuationReturn
            }
            3 => {
                let binding = plans
                    .source_bindings
                    .iter()
                    .find(|(_, binding)| {
                        binding.state == state
                            && binding.role == checked_trees::CheckedScalarExpressionRole::Return
                    })
                    .unwrap()
                    .1
                    .clone();
                plans.source_bindings.append(binding);
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ordered_qualified_scalar_result_carries_its_range_refinement() {
    use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm};
    // A constrained scalar result keeps its declared interval as replayed
    // ensures evidence through ordered boundary completion; the returned
    // carrier is never narrowed and the authored guarantee still applies.
    for (body, calls) in [
        ("Host::finish(value); value", vec![0, 1]),
        (
            "Host::finish(value); let observed: i32 = Host::measure(0); value",
            vec![0, -1, 1],
        ),
        (
            "let observed: i32 = Host::measure(value); value",
            vec![-2, 1],
        ),
        (
            "Host::finish(value); let observed: i32 = Host::measure(value); let _discarded: i32 = observed; value",
            vec![0, -2, 1],
        ),
    ] {
        for input in [1i128, 25, 50] {
            let source = normal_guarantee_source(body)
                .replace(
                    "Scalar::measure(value: i32) -> i32",
                    "Scalar::measure(value: i32 [1..=50]) -> i32 [1..=100]",
                )
                .replace("Scalar::measure(70)", &format!("Scalar::measure({input})"));
            let published = artifact(&checked_from_source(&source));
            let module = decode_module(&published.0).unwrap();
            let proof = decode_proof_bundle(&published.1).unwrap();
            let wrapper = module
                .machines
                .iter()
                .find(|machine| {
                    !machine.contract.ensures.is_empty()
                        && !machine.contract.requires.is_empty()
                })
                .unwrap();
            // The authored guarantee and the replayed range refinement share
            // one obligation; the returned declaration still carries its
            // declared qualification rather than a narrowed carrier.
            assert_eq!(wrapper.contract.ensures.len(), 1);
            let result_id = match &wrapper.result {
                terminal_psi::TerminalMachineResult::Scalar(result) => result.id,
                _ => panic!("qualified scalar wrapper"),
            };
            let Proposition::Conjunction(conjuncts) =
                &wrapper.contract.ensures[0].proposition
            else {
                panic!("the refined ensures clause is a conjunction")
            };
            let mut authored_equality = false;
            let mut bounds = (false, false);
            for conjunct in conjuncts {
                match conjunct {
                    Proposition::Equal(
                        ScalarTerm::Value { id: left, .. },
                        ScalarTerm::Value { id: right, .. },
                    ) if *right == result_id || *left == result_id => {
                        authored_equality = true;
                    }
                    Proposition::LessOrEqual(
                        ScalarTerm::Value { id, .. },
                        ScalarTerm::Integer {
                            value: IntegerValue::Signed(100),
                            ..
                        },
                    ) if *id == result_id => bounds.0 = true,
                    Proposition::LessOrEqual(
                        ScalarTerm::Integer {
                            value: IntegerValue::Signed(1),
                            ..
                        },
                        ScalarTerm::Value { id, .. },
                    ) if *id == result_id => bounds.1 = true,
                    _ => {}
                }
            }
            assert!(authored_equality && bounds.0 && bounds.1);
            let mut missing = proof.clone();
            missing
                .evidence
                .retain(|evidence| {
                    evidence.obligation != wrapper.contract.ensures[0].obligation
                });
            assert!(
                terminal_verifier::verify_module(&module, &missing, &AdmissionProfile::default())
                    .is_err()
            );
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit),
                "{body} input {input}"
            );
            // Ordered boundary calls fire in source order; `Host::measure`
            // observes its own argument and `Host::finish` the passed value.
            let expected: Vec<Vec<TerminalScalarValue>> = calls
                .iter()
                .map(|call| match call {
                    -1 => vec![integer(0)],
                    _ => vec![integer(input)],
                })
                .collect();
            assert_eq!(observed.arguments, expected, "{body} input {input}");
        }
    }
}

#[test]
fn ordered_qualified_scalar_result_rejects_a_tightened_refinement() {
    use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm};
    // Returning 50 honors the declared `i32 [1..=100]` refinement; silently
    // tightening the replayed interval to `[1..=40]` must fail verification.
    let source = normal_guarantee_source("Host::finish(value); value")
        .replace(
            "Scalar::measure(value: i32) -> i32",
            "Scalar::measure(value: i32 [1..=50]) -> i32 [1..=100]",
        )
        .replace("Scalar::measure(70)", "Scalar::measure(50)");
    let published = artifact(&checked_from_source(&source));
    let mut module = decode_module(&published.0).unwrap();
    let proof = decode_proof_bundle(&published.1).unwrap();
    let wrapper = module
        .machines
        .iter_mut()
        .find(|machine| {
            !machine.contract.ensures.is_empty() && !machine.contract.requires.is_empty()
        })
        .unwrap();
    let conjuncts = wrapper
        .contract
        .ensures
        .iter_mut()
        .find_map(|clause| match &mut clause.proposition {
            Proposition::Conjunction(conjuncts) => Some(conjuncts),
            _ => None,
        })
        .expect("the range refinement is a conjunction of inclusive bounds");
    let mut tightened = false;
    for conjunct in conjuncts.iter_mut() {
        if let Proposition::LessOrEqual(_, ScalarTerm::Integer { value, .. }) = conjunct {
            *value = IntegerValue::Signed(40);
            tightened = true;
        }
    }
    assert!(tightened);
    assert!(
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).is_err()
    );
}

#[test]
fn ordered_qualified_scalar_result_rejects_a_substituted_return() {
    // A second entry input gives the mutation a provably-in-range but wrong
    // substitution: returning `spare` still honors the declared interval yet
    // violates the authored `result == value` guarantee.
    let source = normal_guarantee_source("Host::finish(value); value")
        .replace(
            "Scalar::measure(value: i32) -> i32",
            "Scalar::measure(value: i32 [1..=50], spare: i32) -> i32 [1..=100]",
        )
        .replace("Scalar::measure(70)", "Scalar::measure(40, 7)");
    let published = artifact(&checked_from_source(&source));
    let mut module = decode_module(&published.0).unwrap();
    let proof = decode_proof_bundle(&published.1).unwrap();
    let wrapper = module
        .machines
        .iter_mut()
        .find(|machine| {
            !machine.contract.ensures.is_empty() && !machine.contract.requires.is_empty()
        })
        .unwrap();
    let spare = wrapper.parameters[1].id;
    let returned = wrapper
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            terminal_psi::Terminator::Return { value, .. } => Some(value),
            _ => None,
        })
        .expect("qualified scalar return");
    assert_ne!(*returned, spare);
    *returned = spare;
    assert!(
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).is_err()
    );
}
