//! Selective value dispatch survives checked custody and canonical execution.

use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_fuel::FuelChargeSite;
use terminal_interpreter::{
    MeasuredTerminalExecution, TerminalExecutionResult, TerminalScalarValue,
    interpret_terminal_artifact_measured,
};
use terminal_psi::{OperationKind, TerminalModule};

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
        value: IntegerValue::Unsigned(value),
    }
}

fn execute(
    source: &str,
    arguments: &[TerminalScalarValue],
) -> (TerminalModule, MeasuredTerminalExecution) {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("dispatch tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("dispatch syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("dispatch resolution");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("dispatch typing");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("checking {source}: {errors:#?}"));
    for machine in checked.machines() {
        assert_eq!(
            checked.machine_states(machine).len(),
            1,
            "dispatch does not manufacture source states"
        );
    }
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "choose")
        .unwrap_or_else(|error| panic!("lowering {source}: {error:#?}"));
    let semantic_bytes =
        terminal_codec::encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof_bytes =
        terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("encode proof");
    let module = terminal_codec::decode_module(&semantic_bytes).expect("decode semantics");
    let proof = terminal_codec::decode_proof_bundle(&proof_bytes).expect("decode proof");
    let profile = AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile)
        .expect("independent dispatch verification");
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    let execution =
        interpret_terminal_artifact_measured(&semantic_bytes, &proof_bytes, &profile, arguments)
            .unwrap_or_else(|error| panic!("execution {source}: {error:#?}"));
    (module, execution)
}

#[test]
fn typed_integer_dispatch_keeps_ordered_overlaps_and_wildcard_coverage() {
    let source =
        "machine choose(subject: u64) -> u64 { match subject { 0 -> 7, 0 -> 99, 1 -> 8, _ -> 9 } }";
    for (subject, expected) in [(0, 7), (1, 8), (2, 9)] {
        let (_, execution) = execute(source, &[unsigned(subject)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected))
        );
    }
}

#[test]
fn boolean_dispatch_executes_complete_value_coverage_without_wildcard() {
    let source = "machine choose(subject: bool) -> u64 { match subject { true -> 7, false -> 8 } }";
    for (subject, expected) in [(true, 7), (false, 8)] {
        let (_, execution) = execute(source, &[TerminalScalarValue::Boolean(subject)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected))
        );
    }
}

#[test]
fn value_dispatch_composes_inside_call_arguments_and_bitwise_operands() {
    for (expression, expected) in [
        ("identity(match subject { 0 -> 7, _ -> 9 })", 7),
        ("(match subject { 0 -> 7, _ -> 9 }) | 8u64", 15),
        ("8u64 | identity(match subject { 0 -> 7, _ -> 9 })", 15),
    ] {
        let source = format!(
            "machine identity(value: u64) -> u64 {{ value }} machine choose(subject: u64) -> u64 {{ {expression} }}"
        );
        let (_, execution) = execute(&source, &[unsigned(0)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected)),
            "{expression}"
        );
    }
}

#[test]
fn value_dispatch_subject_call_is_invoked_once_across_multiple_comparisons() {
    let (module, execution) = execute(
        "machine subject(value: u64) -> u64 { value } machine choose(value: u64) -> u64 { match subject(value) { 0 -> 11, 1 -> 22, 2 -> 7, _ -> 44 } }",
        &[unsigned(2)],
    );
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(unsigned(7))
    );
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry");
    let calls = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "one retained subject invocation");
    assert_eq!(
        execution
            .usage()
            .at(FuelChargeSite::Operation(calls[0].id))
            .expect("subject executes")
            .executions(),
        1
    );
}

#[test]
fn value_dispatch_unselected_result_call_has_zero_runtime_invocations() {
    let (module, execution) = execute(
        "machine unselected(value: u64) -> u64 { value } machine choose(subject: u64) -> u64 { match subject { 0 -> 7, _ -> unselected(99) } }",
        &[unsigned(0)],
    );
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(unsigned(7))
    );
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry");
    let calls = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        1,
        "valid unselected call remains in the artifact"
    );
    assert_eq!(
        execution
            .usage()
            .at(FuelChargeSite::Operation(calls[0].id))
            .map_or(0, |usage| usage.executions()),
        0
    );
}

#[test]
fn value_dispatch_default_call_keeps_its_nonzero_premise_through_terminal() {
    let source = "machine nonzero(value: i64) -> i64 requires value != 0 { value }
        machine choose(value: i64) -> i64 { match value { 0 -> 7 _ -> nonzero(value) } }";
    let signed = |value| TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 64).expect("i64"),
        value: IntegerValue::Signed(value),
    };
    for (input, expected) in [(0, 7), (-3, -3), (9, 9)] {
        let (_, execution) = execute(source, &[signed(input)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(signed(expected))
        );
    }
}

#[test]
fn value_dispatch_arms_land_exact_arithmetic_at_the_result_destination() {
    for source in [
        "machine choose(flag: bool) -> u64 {
        match flag {
            true -> 18446744073709551616 / 18446744073709551616
            false -> 7 / 2 * 2
        }
    }",
        "machine take(value: u64) -> u64 { value }
     machine choose(flag: bool) -> u64 {
        take(match flag {
            true -> match flag { _ -> 18446744073709551616 / 18446744073709551616 }
            false -> take(7 / 2 * 2)
        })
    }",
    ] {
        for (flag, expected) in [(true, 1), (false, 7)] {
            let (_, execution) = execute(source, &[TerminalScalarValue::Boolean(flag)]);
            assert_eq!(
                execution.value(),
                TerminalExecutionResult::Scalar(unsigned(expected))
            );
        }
    }
}

#[test]
fn anonymous_numeric_trees_land_at_either_typed_integer_peer() {
    for (anonymous, expected) in [
        ("18446744073709551616 / 18446744073709551616", 9),
        ("7 / 2 * 2", 15),
    ] {
        for expression in [
            format!("({anonymous}) | 8u64"),
            format!("8u64 | ({anonymous})"),
        ] {
            let source = format!("machine choose() -> u64 {{ {expression} }}");
            let (_, execution) = execute(&source, &[]);
            assert_eq!(
                execution.value(),
                TerminalExecutionResult::Scalar(unsigned(expected)),
                "{expression}"
            );
        }
    }
}

#[test]
fn anonymous_match_results_land_at_either_typed_integer_peer() {
    for dispatch in [
        "match flag { true -> 18446744073709551616 / 18446744073709551616, false -> 7 / 2 * 2 }",
        "match flag { true -> match flag { _ -> 18446744073709551616 / 18446744073709551616 }, false -> match flag { _ -> 7 / 2 * 2 } }",
    ] {
        for expression in [
            format!("({dispatch}) | 8u64"),
            format!("8u64 | ({dispatch})"),
        ] {
            let source = format!("machine choose(flag: bool) -> u64 {{ {expression} }}");
            for (flag, expected) in [(true, 9), (false, 15)] {
                let (_, execution) = execute(&source, &[TerminalScalarValue::Boolean(flag)]);
                assert_eq!(
                    execution.value(),
                    TerminalExecutionResult::Scalar(unsigned(expected)),
                    "{expression}; flag={flag}"
                );
            }
        }
    }
}

#[test]
fn anonymous_numeric_trees_land_at_explicit_integer_cast() {
    for (anonymous, expected) in [
        ("18446744073709551616 / 18446744073709551616", 1),
        ("7 / 2 * 2", 7),
    ] {
        let source = format!("machine choose() -> u64 {{ ({anonymous}) as u64 }}");
        let (_, execution) = execute(&source, &[]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected)),
            "{anonymous}"
        );
    }
}

#[test]
fn anonymous_match_results_land_at_explicit_integer_cast() {
    for dispatch in [
        "match flag { true -> 18446744073709551616 / 18446744073709551616, false -> 7 / 2 * 2 }",
        "match flag { true -> match flag { _ -> 18446744073709551616 / 18446744073709551616 }, false -> match flag { _ -> 7 / 2 * 2 } }",
    ] {
        let source = format!("machine choose(flag: bool) -> u64 {{ ({dispatch}) as u64 }}");
        for (flag, expected) in [(true, 1), (false, 7)] {
            let (_, execution) = execute(&source, &[TerminalScalarValue::Boolean(flag)]);
            assert_eq!(
                execution.value(),
                TerminalExecutionResult::Scalar(unsigned(expected)),
                "{dispatch}; flag={flag}"
            );
        }
    }
}

#[test]
fn integer_cast_preserves_typed_match_result_before_widening() {
    for (dispatch, false_result) in [
        ("match flag { true -> 1u32, false -> 7 / 2 * 2 }", 7),
        (
            "match flag { true -> 1u32, false -> 18446744073709551616 / 18446744073709551616 }",
            1,
        ),
        (
            "match flag { true -> 18446744073709551616 / 18446744073709551616, false -> 7u32 }",
            7,
        ),
    ] {
        let source = format!("machine choose(flag: bool) -> u64 {{ ({dispatch}) as u64 }}");
        for flag in [true, false] {
            let expected = if flag { 1 } else { false_result };
            let (module, execution) = execute(&source, &[TerminalScalarValue::Boolean(flag)]);
            assert_eq!(
                execution.value(),
                TerminalExecutionResult::Scalar(unsigned(expected)),
                "{dispatch}; flag={flag}"
            );
            assert!(
                module
                    .machines
                    .iter()
                    .flat_map(|machine| &machine.blocks)
                    .flat_map(|block| &block.operations)
                    .any(|operation| matches!(operation.kind, OperationKind::IntegerWiden { .. })),
                "the u32 result must survive until the explicit u64 conversion"
            );
        }
    }
}

#[test]
fn typed_match_peer_supplies_anonymous_numeric_result_carrier() {
    let typed_peer = "match flag { true -> 8u64, false -> 9u64 }";
    for (anonymous, when_true, when_false) in [
        ("18446744073709551616 / 18446744073709551616", 9, 9),
        ("7 / 2 * 2", 15, 15),
        (
            "match flag { true -> match flag { _ -> 18446744073709551616 / 18446744073709551616 }, false -> match flag { _ -> 7 / 2 * 2 } }",
            9,
            15,
        ),
    ] {
        for expression in [
            format!("({anonymous}) | ({typed_peer})"),
            format!("({typed_peer}) | ({anonymous})"),
        ] {
            let source = format!("machine choose(flag: bool) -> u64 {{ {expression} }}");
            for (flag, expected) in [(true, when_true), (false, when_false)] {
                let (_, execution) = execute(&source, &[TerminalScalarValue::Boolean(flag)]);
                assert_eq!(
                    execution.value(),
                    TerminalExecutionResult::Scalar(unsigned(expected)),
                    "{expression}; flag={flag}"
                );
            }
        }
    }
}

#[test]
fn composed_integer_peers_supply_anonymous_numeric_result_carriers() {
    // Each peer produces 24 in its actual u64 carrier before the outer OR.
    for typed_peer in [
        "8u64 | 16u64",
        "8u64 + 16u64",
        "~18446744073709551591u64",
        "192u64 >> 3u64",
    ] {
        for (anonymous, when_true, when_false) in [
            ("18446744073709551616 / 18446744073709551616", 25, 25),
            ("7 / 2 * 2", 31, 31),
            (
                "match flag { true -> 18446744073709551616 / 18446744073709551616, false -> 7 / 2 * 2 }",
                25,
                31,
            ),
            (
                "match flag { true -> match flag { _ -> 18446744073709551616 / 18446744073709551616 }, false -> match flag { _ -> 7 / 2 * 2 } }",
                25,
                31,
            ),
        ] {
            for expression in [
                format!("({anonymous}) | ({typed_peer})"),
                format!("({typed_peer}) | ({anonymous})"),
            ] {
                let source = format!("machine choose(flag: bool) -> u64 {{ {expression} }}");
                for (flag, expected) in [(true, when_true), (false, when_false)] {
                    let (_, execution) = execute(&source, &[TerminalScalarValue::Boolean(flag)]);
                    assert_eq!(
                        execution.value(),
                        TerminalExecutionResult::Scalar(unsigned(expected)),
                        "{expression}; flag={flag}"
                    );
                }
            }
        }
    }
}

#[test]
fn integer_cast_preserves_computed_match_arm_carrier_before_widening() {
    for typed_arm in [
        "8u32 | 16u32",
        "8u32 + 16u32",
        "~4294967271u32",
        "192u32 >> 3u32",
    ] {
        for (dispatch, when_true, when_false) in [
            (
                format!("match flag {{ true -> {typed_arm}, false -> 7 / 2 * 2 }}"),
                24,
                7,
            ),
            (
                format!(
                    "match flag {{ true -> 18446744073709551616 / 18446744073709551616, false -> {typed_arm} }}"
                ),
                1,
                24,
            ),
        ] {
            let source = format!("machine choose(flag: bool) -> u64 {{ ({dispatch}) as u64 }}");
            for (flag, expected) in [(true, when_true), (false, when_false)] {
                let (module, execution) = execute(&source, &[TerminalScalarValue::Boolean(flag)]);
                assert_eq!(
                    execution.value(),
                    TerminalExecutionResult::Scalar(unsigned(expected)),
                    "{dispatch}; flag={flag}"
                );
                assert!(
                    module
                        .machines
                        .iter()
                        .flat_map(|machine| &machine.blocks)
                        .flat_map(|block| &block.operations)
                        .any(|operation| matches!(
                            operation.kind,
                            OperationKind::IntegerWiden { .. }
                        )),
                    "the computed u32 arm must retain its carrier through the match join: {dispatch}"
                );
            }
        }
    }
}

#[test]
fn high_unsigned_comparisons_preserve_order_across_the_signed_boundary() {
    let subjects = [
        0,
        9223372036854775807,
        9223372036854775808,
        18446744073709551591,
    ];
    for (comparison, expected_results) in [
        ("value > 0u64", [false, true, true, true]),
        ("0u64 < value", [false, true, true, true]),
        ("value < 9223372036854775808u64", [true, true, false, false]),
        (
            "9223372036854775808u64 <= value",
            [false, false, true, true],
        ),
        ("18446744073709551591u64 > value", [true, true, true, false]),
        (
            "value >= 18446744073709551591u64",
            [false, false, false, true],
        ),
    ] {
        let source = format!("machine choose(value: u64) -> bool {{ {comparison} }}");
        for (subject, expected) in subjects.into_iter().zip(expected_results) {
            let (_, execution) = execute(&source, &[unsigned(subject)]);
            assert_eq!(
                execution.value(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected)),
                "{comparison}; value={subject}"
            );
        }
    }
}

#[test]
fn high_unsigned_literals_retain_their_carrier_at_scalar_boundaries() {
    for value in [9223372036854775808u128, 18446744073709551615] {
        for expression in [
            format!("{value}u64"),
            format!("identity({value}u64)"),
            format!("{value}u64 as u64"),
        ] {
            let source = format!(
                "machine identity(value: u64) -> u64 {{ value }} machine choose() -> u64 {{ {expression} }}"
            );
            let (_, execution) = execute(&source, &[]);
            assert_eq!(
                execution.value(),
                TerminalExecutionResult::Scalar(unsigned(value)),
                "{expression}"
            );
        }
    }
}

#[test]
fn qualified_computed_peers_land_anonymous_values_before_policy_erasure() {
    let maximum = u128::from(u64::MAX);
    for policy in ["Wrapping", "Saturating"] {
        let typed_peer = format!("(value as u64 in {policy}) + 2");
        for (anonymous, wrapped_result) in [
            ("18446744073709551616 / 18446744073709551616", 2),
            ("7 / 2 * 2", 8),
        ] {
            for expression in [
                format!("(({anonymous}) + ({typed_peer})) as u64"),
                format!("(({typed_peer}) + ({anonymous})) as u64"),
            ] {
                let source = format!("machine choose(value: u64) -> u64 {{ {expression} }}");
                let (_, execution) = execute(&source, &[unsigned(maximum)]);
                let expected = if policy == "Wrapping" {
                    wrapped_result
                } else {
                    maximum
                };
                assert_eq!(
                    execution.value(),
                    TerminalExecutionResult::Scalar(unsigned(expected)),
                    "{expression}"
                );
            }
        }
    }
}

#[test]
fn qualified_computed_match_results_retain_policy_until_explicit_erasure() {
    let maximum = u128::from(u64::MAX);
    for policy in ["Wrapping", "Saturating"] {
        let typed_arm = format!("(value as u64 in {policy}) + 2");
        for (anonymous, anonymous_result) in [
            ("18446744073709551616 / 18446744073709551616", 2),
            ("7 / 2 * 2", 8),
        ] {
            for dispatch in [
                format!("match flag {{ true -> {typed_arm}, false -> {anonymous} }}"),
                format!(
                    "match flag {{ true -> match flag {{ _ -> {typed_arm} }}, false -> match flag {{ _ -> {anonymous} }} }}"
                ),
            ] {
                // Both additions retain policy: erasing at the join would make
                // the saturating arm's final addition an invalid Exact overflow.
                let source = format!(
                    "machine choose(flag: bool, value: u64) -> u64 {{ (({dispatch}) + 1) as u64 }}"
                );
                for flag in [true, false] {
                    let (_, execution) = execute(
                        &source,
                        &[TerminalScalarValue::Boolean(flag), unsigned(maximum)],
                    );
                    let expected = if !flag {
                        anonymous_result
                    } else if policy == "Wrapping" {
                        2
                    } else {
                        maximum
                    };
                    assert_eq!(
                        execution.value(),
                        TerminalExecutionResult::Scalar(unsigned(expected)),
                        "{source}; flag={flag}"
                    );
                }
            }
        }
    }
}
