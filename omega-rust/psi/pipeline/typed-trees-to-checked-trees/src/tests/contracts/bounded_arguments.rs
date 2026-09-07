use super::{lower_typed_trees, parse_typed_trees};

#[test]
fn carrier_safety_proofs_preserve_tighter_argument_bounds() {
    for (carrier, guard, value, maximum) in [
        ("u32", "left <= 4294967295 - right", "left + right", 7),
        ("u32", "left >= right", "left - right", 4),
        ("u32", "left <= 4294967295 / right", "left * right", 10),
        ("u64", "true", "left << right", 20),
    ] {
        let source = format!(
            "machine run(left: {carrier} [0..=5], right: {carrier} [1..=2]) -> {carrier} {{
                transition {guard} {{ true -> accept({value}) false -> 0 }}
                state accept(delivered: {carrier} [0..={maximum}]) -> {carrier} {{ delivered }}
            }}"
        );
        lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
        assert!(
            lower_typed_trees(parse_typed_trees(
                &source.replace(&format!("0..={maximum}"), &format!("0..={}", maximum - 1)),
            ))
            .is_err(),
            "a tighter-than-proved target accepted: {source}"
        );
    }
}

#[test]
fn named_transition_refolds_nonliteral_operands_under_its_own_guard() {
    for (guard, value, range) in [
        ("pending > 0", "pending - step", "0..=4"),
        ("!(pending <= 0)", "pending - step", "0..=4"),
        ("pending < 5", "pending + step", "1..=5"),
        ("pending > 0 && pending < 5", "pending * step", "1..=4"),
    ] {
        let source = format!(
            "machine run(pending: u32 [0..=5], step: u32 [1..=1]) -> u32 {{
                transition {guard} {{ true -> accept({value}) false -> 0 }}
                state accept(delivered: u32 [{range}]) -> u32 {{ delivered }}
            }}"
        );
        lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
    }
}

#[test]
fn named_transition_nonliteral_bounds_reject_insufficient_or_unrelated_guards() {
    for (guard, step_range) in [
        ("pending >= 0", "1..=1"),
        ("pending > 0", "1..=2"),
        ("other > 0", "1..=1"),
        ("!(pending > 0)", "1..=1"),
        ("pending > 0 || other > 0", "1..=1"),
    ] {
        let source = format!(
            "machine run(pending: u32 [0..=5], step: u32 [{step_range}], other: u32) -> u32 {{
                transition {guard} {{ true -> accept(pending - step) false -> 0 }}
                state accept(delivered: u32 [0..=4]) -> u32 {{ delivered }}
            }}"
        );
        let program = parse_typed_trees(&source);
        let plan = proof::obligations::build_proof_plan(&program);
        assert!(proof::checker::check_proof_plan(&plan).is_err(), "{source}");
    }
}

#[test]
fn named_transition_arithmetic_query_rejects_selected_and_effectful_trees() {
    for (declarations, guard, first, delivered) in [
        (
            "operator - u32::custom(left: u32, right: u32) -> u32;",
            "pending > 0",
            "0",
            "pending - step",
        ),
        (
            "operator > u32::custom(left: u32, right: u32) -> bool;",
            "pending > 0",
            "0",
            "pending - step",
        ),
        (
            "operator + u32::custom(left: u32, right: u32) -> u32;",
            "pending > 0",
            "pending + step",
            "pending - step",
        ),
        (
            "machine zero(target: &mut u32) -> u32 { target = 0; 0 }",
            "pending > 0",
            "zero(&mut pending)",
            "pending - step",
        ),
        (
            "",
            "pending > 0",
            "0",
            "((pending as u32 in Wrapping) - step) as u32",
        ),
    ] {
        let source = format!(
            "{declarations}
            machine run(input: u32 [0..=5], step: u32 [1..=1]) -> u32 {{
                let mut pending: u32 = input;
                transition {guard} {{ true -> accept({first}, {delivered}) false -> 0 }}
                state accept(first: u32, delivered: u32 [0..=4]) -> u32 {{ delivered }}
            }}"
        );
        let program = parse_typed_trees(&source);
        let plan = proof::obligations::build_proof_plan(&program);
        let obligation = plan
            .obligations
            .iter()
            .find_map(|(_, obligation)| match obligation {
                proof::obligations::ProofObligation::BoundedTransitionArgument(argument)
                    if argument.parameter.as_str() == "delivered" =>
                {
                    Some(argument)
                }
                _ => None,
            })
            .expect("bounded argument occurrence");
        assert_eq!(
            validation::arrival_integer_expression_bounds(
                &program,
                obligation.machine_symbol,
                obligation.state_symbol,
                obligation.statement_index,
                obligation.argument,
            ),
            None,
            "{source}"
        );
    }
}

#[test]
fn named_transition_range_evidence_belongs_to_its_exact_occurrence() {
    let program = parse_typed_trees(
        "machine run(pending: u32 [0..=5], step: u32 [1..=1]) -> u32 {
            transition pending > 0 { true -> accept(pending - step) false -> 0 }
            state accept(delivered: u32 [0..=4]) -> u32 { delivered }
        }",
    );
    let plan = proof::obligations::build_proof_plan(&program);
    let obligation = plan
        .obligations
        .iter()
        .find_map(|(_, obligation)| match obligation {
            proof::obligations::ProofObligation::BoundedTransitionArgument(argument) => {
                Some(argument)
            }
            _ => None,
        })
        .expect("bounded argument occurrence");
    let query = |statement_index, expression| {
        validation::arrival_integer_expression_bounds(
            &program,
            obligation.machine_symbol,
            obligation.state_symbol,
            statement_index,
            expression,
        )
    };
    assert_eq!(
        query(obligation.statement_index, obligation.argument),
        Some((0, 4))
    );
    assert_eq!(query(usize::MAX, obligation.argument), None);
    assert_eq!(
        query(
            obligation.statement_index,
            typed_trees::expression::ExpressionHandle::invalid()
        ),
        None
    );
}

#[test]
fn named_transition_integer_query_does_not_truncate_anonymous_division() {
    let program = parse_typed_trees(
        "machine run() -> i32 {
            transition { _ -> finish(7 / 2 * 2) }
            state finish(delivered: i32 [6..=6]) -> i32 { delivered }
        }",
    );
    let plan = proof::obligations::build_proof_plan(&program);
    for (_, obligation) in plan.obligations.iter() {
        if let proof::obligations::ProofObligation::BoundedTransitionArgument(argument) = obligation
        {
            assert_eq!(
                validation::arrival_integer_expression_bounds(
                    &program,
                    argument.machine_symbol,
                    argument.state_symbol,
                    argument.statement_index,
                    argument.argument,
                ),
                None
            );
        }
    }
    assert!(proof::checker::check_proof_plan(&plan).is_err());
}

fn rejects_range(source: &str) {
    let diagnostics = match lower_typed_trees(parse_typed_trees(source)) {
        Ok(_) => panic!("out-of-range argument was accepted"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("not provably within its declared range")),
        "{diagnostics:#?}"
    );
}

#[test]
fn statement_and_value_calls_must_establish_the_parameter_range() {
    for argument in ["0", "6"] {
        for body in [
            format!("_ = accept({argument});"),
            format!("let result: u32 = accept({argument});"),
        ] {
            rejects_range(&format!(
                "machine accept(value: u32 [1..=5]) -> u32 {{ value }} machine run() {{ {body} }}"
            ));
        }
    }
    for body in ["_ = accept(1);", "let result: u32 = accept(5);"] {
        lower_typed_trees(parse_typed_trees(&format!(
            "machine accept(value: u32 [1..=5]) -> u32 {{ value }} machine run() {{ {body} }}"
        )))
        .expect("in-range call");
    }
}

#[test]
fn incoming_argument_guards_keep_their_own_polarity() {
    let positive = r#"
        machine accept(delivered: u32 [1..=5]) -> u32 { delivered }
        machine run(value: u32 [0..=5]) -> u32 {
            transition value > 0 {
                true -> accept(value)
                false -> 0
            }
        }
    "#;
    lower_typed_trees(parse_typed_trees(positive))
        .expect("the positive guard establishes the floor");
    rejects_range(&positive.replace(
        "true -> accept(value)\n                false -> 0",
        "true -> 0\n                false -> accept(value)",
    ));
}

#[test]
fn named_state_delivery_checks_a_renamed_parameter_range() {
    let source = r#"
        machine run(value: u32 [0..=5]) -> u32 {
            transition value > 0 {
                true -> accept(value)
                false -> 0
            }
            state accept(delivered: u32 [1..=5]) -> u32 { delivered }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source)).expect("guarded named-state arrival");
    rejects_range(&source.replace("value > 0", "value >= 0"));
}

#[test]
fn an_unknown_argument_cannot_claim_the_callees_range() {
    rejects_range(
        r#"
        machine accept(value: u32 [1..=5]) -> u32 { value }
        machine run(source: u32) -> u32 { accept(source) }
    "#,
    );
}

#[test]
fn immutable_singleton_bound_is_not_a_guess_about_a_variable_limit() {
    let source = r#"
        machine accept(value: u32 [1..=4]) -> u32 { value }
        machine run(limit: u32 [5..=5], value: u32 [1..=5]) -> u32 {
            transition value < limit {
                true -> accept(value)
                false -> 0
            }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source)).expect("the immutable limit is exactly five");
    rejects_range(&source.replace("limit: u32 [5..=5]", "limit: u32 [4..=6]"));
}

#[test]
fn bare_dispatch_guards_establish_bounded_arguments() {
    for condition in [
        "fuel > 1",
        "!(fuel <= 1)",
        "!!(fuel > 1)",
        "fuel > 1 && fuel < 100",
    ] {
        for target_declaration in ["machine", "state"] {
            let target = format!(
                "{target_declaration} advance(delivered: u64 [1..=128]) -> u64 {{ delivered }}"
            );
            let (local_target, external_target) = if target_declaration == "state" {
                (target.as_str(), "")
            } else {
                ("", target.as_str())
            };
            let source = format!(
                "machine run(fuel: u64 [1..=128]) -> u64 {{
                    transition {{ {condition} -> advance(fuel - 1) _ -> 0 }}
                    {local_target}
                }} {external_target}"
            );
            lower_typed_trees(parse_typed_trees(&source))
                .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
        }
    }
}

#[test]
fn later_dispatch_arm_keeps_its_own_fuel_guard() {
    let source = r#"
        machine run(fuel: u64 [1..=128], matched: bool) -> u64 {
            transition {
                fuel > 1 && matched -> 0
                fuel > 1 -> advance(fuel - 1)
                _ -> 0
            }
            state advance(delivered: u64 [1..=128]) -> u64 { delivered }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source)).expect("the second arm supplies its own floor");
}

#[test]
fn insufficient_dispatch_guards_cannot_deliver_a_bounded_decrement() {
    for condition in [
        "fuel >= 1",
        "fuel <= 1",
        "!(fuel > 1)",
        "fuel > 1 || matched",
        "matched",
        "_",
    ] {
        rejects_range(&format!(
            "machine run(fuel: u64 [1..=128], matched: bool) -> u64 {{
                transition {{ {condition} -> advance(fuel - 1) _ -> 0 }}
                state advance(delivered: u64 [1..=128]) -> u64 {{ delivered }}
            }}"
        ));
    }
}

#[test]
fn dispatch_guards_preserve_bounded_fuel_on_ranked_state_cycles() {
    let source = r#"
        machine run(fuel: u64 [1..=128]) -> u64
        terminates by fuel;
        {
            transition { _ -> seek(fuel) }
            state seek(fuel: u64 [1..=128]) -> u64 {
                transition { fuel > 1 -> advance(fuel - 1) _ -> 0 }
            }
            state advance(fuel: u64 [1..=128]) -> u64 {
                transition { fuel > 1 -> seek(fuel - 1) _ -> 0 }
            }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source)).expect("bounded decreasing fuel stays in range");
}

#[test]
fn dispatch_guard_and_argument_writes_retire_the_bounded_value() {
    for (guard, arguments) in [
        ("current > 0 && zero(&mut current)", "true, current"),
        ("current > 0", "zero(&mut current), current"),
    ] {
        rejects_range(&format!(
            "machine zero(target: &mut u8) -> bool {{ target = 0; true }}
            machine run() -> u8 {{
                let mut current: u8 = 3;
                transition {{ {guard} -> finish({arguments}) _ -> 0 }}
                state finish(first: bool, delivered: u8 [1..=255]) -> u8 {{ delivered }}
            }}"
        ));
    }
}

#[test]
fn authored_dispatch_operators_cannot_supply_builtin_bounds() {
    for (declaration, condition) in [
        (
            "operator > u64::custom(left: u64, right: u64) -> bool;",
            "fuel > 1",
        ),
        (
            "operator == bool::custom(left: bool, right: bool) -> bool;",
            "(fuel > 1) == true",
        ),
    ] {
        rejects_range(&format!(
            "{declaration}
            machine run(fuel: u64 [1..=128]) -> u64 {{
                transition {{ ({condition}) -> advance(fuel - 1) _ -> 0 }}
                state advance(delivered: u64 [1..=128]) -> u64 {{ delivered }}
            }}"
        ));
    }
}

#[test]
fn bounded_argument_proof_independently_preserves_negation_polarity() {
    // Exercise the proof consumer directly so rejection cannot be attributed
    // solely to the earlier arithmetic validation pass.
    for (condition, accepted) in [
        ("!(fuel <= 1)", true),
        ("!!(fuel > 1)", true),
        ("!(fuel > 1)", false),
        ("!!(fuel <= 1)", false),
    ] {
        let source = format!(
            "machine run(fuel: u64 [1..=128]) -> u64 {{
                transition {{ {condition} -> advance(fuel - 1) _ -> 0 }}
                state advance(delivered: u64 [1..=128]) -> u64 {{ delivered }}
            }}"
        );
        let program = parse_typed_trees(&source);
        let plan = proof::obligations::build_proof_plan(&program);
        match proof::checker::check_proof_plan(&plan) {
            Ok(()) => assert!(accepted, "{source}"),
            Err(diagnostics) => {
                assert!(!accepted, "{source}\n{diagnostics:#?}");
                assert!(
                    diagnostics.iter().any(|diagnostic| {
                        diagnostic
                            .message
                            .contains("cannot prove transition argument")
                            && diagnostic.message.contains("bounded parameter")
                    }),
                    "{diagnostics:#?}"
                );
            }
        }
    }
}
