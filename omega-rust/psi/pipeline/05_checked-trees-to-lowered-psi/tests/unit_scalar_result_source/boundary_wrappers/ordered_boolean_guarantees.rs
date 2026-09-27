use super::{
    artifact, boolean_guarantee_source, execute, integer, nested_boolean_guarantee_source,
    normal_guarantee_source,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
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
        let artifact = artifact(&crate::front_end::checked_program(&source));
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
            let published = artifact(&crate::front_end::checked_program(&source));
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
            let published = artifact(&crate::front_end::checked_program(&source));
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
            let checked = crate::front_end::checked_program(&source);
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
            let published = artifact(&crate::front_end::checked_program(&source));
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
            let published = artifact(&crate::front_end::checked_program(&source));
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
    let published = artifact(&crate::front_end::checked_program(&source));
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
        let published = artifact(&crate::front_end::checked_program(&source));
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
fn ordered_mutable_boolean_snapshot_preserves_its_normal_guarantee() {
    for body in [
        "let mut current: bool = value; let saved: bool = current; current = !current; Host::finish(current); saved",
        "let mut current: bool = !value; current = !current; let saved: bool = current; Host::finish(!current); saved",
        "let mut current: bool = value; let mut other: bool = !value; let saved: bool = current; other = !other; current = !current; Host::finish(current); saved",
    ] {
        for input in [false, true] {
            let source = boolean_guarantee_source(body, input);
            let published = artifact(&crate::front_end::checked_program(&source));
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
            let published = artifact(&crate::front_end::checked_program(&source));
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
fn boolean_result_equation_preserves_closed_entry_fact_proofs() {
    let source = boolean_guarantee_source("Host::finish(false); true", false)
        .replace("requires value == value", "requires !value")
        .replace(
            "ensures result == value\nreaches Host",
            "ensures result == !value\nreaches Host",
        );
    let published = artifact(&crate::front_end::checked_program(&source));
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
fn computed_boolean_guarantees_reject_changed_return_and_selected_evidence() {
    use checked_trees::{CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar};
    let source = boolean_guarantee_source("Host::finish(false); !value", false).replace(
        "ensures result == value\nreaches Host",
        "ensures result == !value\nreaches Host",
    );
    let original = crate::front_end::checked_program(&source);
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
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Main::main")
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}
