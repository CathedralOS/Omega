use super::*;

#[test]
fn call_bearing_integer_operations_execute_inside_selected_boolean_operands() {
    for (expression, expected) in [
        ("identity(input) + 1u8", 0u8),
        ("identity(input) - 1u8", 254),
        ("identity(input) * 2u8", 254),
        ("identity(input) / 3u8", 85),
        ("identity(input) % 3u8", 0),
        ("identity(input) & 15u8", 15),
        ("identity(input) | 0u8", 255),
        ("identity(input) ^ 15u8", 240),
        ("~identity(input)", 0),
        ("identity(input) << 1u16", 254),
        ("identity(input) >> 1u16", 127),
        ("identity(identity(input) - 1u8) - 2u8", 252),
    ] {
        let source = format!(
            r#"
            machine identity(input: u8 in Wrapping) -> u8 in Wrapping
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{ input }}
            machine value(selected: bool, flag: bool, input: u8 in Wrapping) -> bool
            requires true == true
            ensures true == true
            {{
                transition selected {{
                    true -> finish(flag && (({expression}) == {expected}u8))
                    false -> finish(flag || (({expression}) != {expected}u8))
                }}
                state finish(result: bool) -> bool {{ result }}
            }}
        "#
        );
        for combined in [false, true] {
            let artifact = encoded_arms(&source, combined);
            for selected in [false, true] {
                for flag in [false, true] {
                    assert_eq!(
                        execute(
                            &artifact,
                            &[
                                TerminalScalarValue::Boolean(selected),
                                TerminalScalarValue::Boolean(flag),
                                TerminalScalarValue::Integer {
                                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 8)
                                        .unwrap(),
                                    value: IntegerValue::Unsigned(255),
                                },
                            ]
                        )
                        .unwrap(),
                        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(flag)),
                        "{expression}, selected={selected}, flag={flag}, combined={combined}"
                    );
                }
            }
        }
    }
}

#[test]
fn computed_integer_policy_and_exact_widening_are_not_destination_inferred() {
    for (policy, expression, expected) in [
        ("Wrapping", "identity(input) + 1", "0u8"),
        ("Saturating", "identity(input) + 1", "255u8"),
        ("Wrapping", "(identity(input) as u16) + 1u16", "256u16"),
        (
            "Wrapping",
            "(identity(input) as u8 in Saturating) + 1u8",
            "255u8",
        ),
        (
            "Saturating",
            "(identity(input) as u8 in Wrapping) + 1u8",
            "0u8",
        ),
        ("Wrapping", "(identity(input) as u8) / 3u8", "85u8"),
    ] {
        let source = format!(
            r#"
            machine identity(input: u8 in {policy}) -> u8 in {policy}
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{ input }}
            machine value(selected: bool, input: u8 in {policy}) -> bool
            requires true == true
            ensures true == true
            {{
                transition selected {{
                    true -> finish(selected && (({expression}) == {expected}))
                    false -> finish(false)
                }}
                state finish(result: bool) -> bool {{ result }}
            }}
        "#
        );
        for combined in [false, true] {
            let artifact = encoded_arms(&source, combined);
            for selected in [false, true] {
                assert_eq!(
                    execute(
                        &artifact,
                        &[
                            TerminalScalarValue::Boolean(selected),
                            TerminalScalarValue::Integer {
                                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                                value: IntegerValue::Unsigned(255),
                            },
                        ]
                    )
                    .unwrap(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(selected)),
                    "{policy}: {expression}, combined={combined}"
                );
            }
        }
    }
}

#[test]
fn computed_integer_comparisons_keep_authored_order_and_meaning() {
    for (operator, expected) in [
        ("==", false),
        ("!=", true),
        ("<", true),
        ("<=", true),
        (">", false),
        (">=", false),
    ] {
        let source = format!(
            r#"
            machine identity(input: i32) -> i32
            requires 0i32 == 0i32
            ensures 0i32 == 0i32
            {{ input }}
            machine value(selected: bool, left: i32, right: i32) -> bool
            requires true == true
            ensures true == true
            {{
                transition selected {{
                    true -> finish(selected && (identity(left) {operator} identity(right)))
                    false -> finish(false)
                }}
                state finish(result: bool) -> bool {{ result }}
            }}
        "#
        );
        for combined in [false, true] {
            let artifact = encoded_arms(&source, combined);
            assert_eq!(
                execute(
                    &artifact,
                    &[TerminalScalarValue::Boolean(true), signed(3), signed(9),]
                )
                .unwrap(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected)),
                "{operator}"
            );
        }
    }
}

fn signed(value: i128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
        value: IntegerValue::Signed(value),
    }
}

#[test]
fn integer_computations_do_not_reverse_or_eagerly_execute_crashing_calls() {
    for comparison in [
        "first() > second()",
        "first() >= second()",
        "(first() - second()) == 0u8",
    ] {
        let source = format!(
            r#"
            machine first() -> u8 in Wrapping
            crashes Abort
            {{ crash Abort; }}
            machine second() -> u8 in Wrapping
            crashes Trap
            {{ crash Trap; }}
            machine value(selected: bool, flag: bool) -> bool
            requires true == true
            ensures true == true
            crashes Abort
            crashes Trap
            {{
                transition selected {{
                    true -> finish(flag && ({comparison}))
                    false -> finish(false)
                }}
                state finish(result: bool) -> bool {{ result }}
            }}
        "#
        );
        for combined in [false, true] {
            let artifact = encoded_arms(&source, combined);
            for selected in [false, true] {
                for flag in [false, true] {
                    let result = execute(
                        &artifact,
                        &[
                            TerminalScalarValue::Boolean(selected),
                            TerminalScalarValue::Boolean(flag),
                        ],
                    );
                    if selected && flag {
                        let TerminalArtifactInterpretError::Execution(
                            TerminalInterpretError::Crash(crash),
                        ) = result.unwrap_err()
                        else {
                            panic!("first call must abort: {comparison}");
                        };
                        assert_eq!(crash.cause, terminal_psi::CrashCause::Abort, "{comparison}");
                    } else {
                        assert_eq!(
                            result.unwrap(),
                            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(false))
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn computed_integer_narrowing_retains_its_exact_value_proof() {
    let source = r#"
        machine identity(input: u16) -> u16
        requires 7u16 == 7u16
        ensures 7u16 == 7u16
        { input }
        machine value(selected: bool, input: u16) -> bool
        requires true == true
        ensures true == true
        {
            transition selected {
                true -> finish(selected && (((identity(input) % 256u16) as u8) == 255u8))
                false -> finish(false)
            }
            state finish(result: bool) -> bool { result }
        }
    "#;
    for combined in [false, true] {
        let artifact = encoded_arms(source, combined);
        for input in [0, 7, 255, 511, 65535] {
            assert_eq!(
                execute(
                    &artifact,
                    &[
                        TerminalScalarValue::Boolean(true),
                        TerminalScalarValue::Integer {
                            scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
                            value: IntegerValue::Unsigned(input),
                        },
                    ]
                )
                .unwrap(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(input % 256 == 255))
            );
        }
        // Changing the actual remainder range must not be blessed by the
        // retained source cast fact. Terminal rebuilds the operand obligation.
        let mut corrupted = checked_arms(source, combined);
        let handle = corrupted
            .facts
            .values
            .scalar_computations
            .nodes
            .iter()
            .find_map(|(handle, node)| {
                let checked_trees::CheckedScalarComputationKind::Value(
                    checked_trees::CheckedScalarExpression::IntegerLiteral { literal },
                ) = &node.kind
                else {
                    return None;
                };
                (literal.value_u64() == Some(256)).then_some(handle)
            })
            .expect("remainder divisor");
        let checked_trees::CheckedScalarComputationKind::Value(
            checked_trees::CheckedScalarExpression::IntegerLiteral { literal: node },
        ) = &mut corrupted
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(handle)
            .kind
        else {
            unreachable!();
        };
        *node = numerics::literals::IntegerLiteral::from_value(512)
            .with_landing(node.landing().unwrap());
        assert!(
            checked_trees_to_terminal_psi::lower_machine(&corrupted, "value").is_err(),
            "stale exact cast evidence cannot narrow a remainder in 0..512"
        );
    }
}

#[test]
fn computed_integer_negation_lands_the_generated_zero() {
    let source = r#"
        machine identity(input: i32 in Wrapping) -> i32 in Wrapping
        requires 0i32 == 0i32
        ensures 0i32 == 0i32
        { input }
        machine value(selected: bool, input: i32 in Wrapping) -> bool
        requires true == true
        ensures true == true
        {
            transition selected {
                true -> finish(selected && (-identity(input) == -7i32))
                false -> finish(false)
            }
            state finish(result: bool) -> bool { result }
        }
    "#;
    for combined in [false, true] {
        let artifact = encoded_arms(source, combined);
        for input in [i32::MIN, -7, 0, 7, i32::MAX] {
            assert_eq!(
                execute(
                    &artifact,
                    &[
                        TerminalScalarValue::Boolean(true),
                        signed(i128::from(input))
                    ]
                )
                .unwrap(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(input == 7))
            );
        }
    }
}

#[test]
fn unproved_computed_narrowing_does_not_gain_a_runtime_conversion() {
    let source = r#"
        machine identity(input: u16) -> u16 { input }
        machine value(selected: bool, input: u16) -> bool {
            transition selected {
                true -> finish(selected && ((identity(input) as u8) == 7u8))
                false -> finish(false)
            }
            state finish(result: bool) -> bool { result }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let diagnostics = lower_typed_trees(typed).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not provably representable")),
        "{diagnostics:#?}"
    );
}

#[test]
fn integer_application_operand_namespaces_reject_malformed_templates() {
    use checked_trees::{CheckedScalarComputationKind, CheckedScalarExpression};
    use typed_trees::types::PrimitiveType;
    let source = r#"
        machine identity(input: u8 in Wrapping) -> u8 in Wrapping
        requires 0u8 == 0u8
        ensures 0u8 == 0u8
        { input }
        machine value(selected: bool, input: u8 in Wrapping) -> bool
        requires true == true
        ensures true == true
        {
            transition selected {
                true -> finish(selected && ((identity(input) + 1u8) == 0u8))
                false -> finish(false)
            }
            state finish(result: bool) -> bool { result }
        }
    "#;
    for combined in [false, true] {
        let checked = checked_arms(source, combined);
        checked_trees_to_terminal_psi::lower_machine(&checked, "value").unwrap();
        let (handle, _) = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .iter()
            .find(|(_, node)| {
                matches!(
                    &node.kind,
                    CheckedScalarComputationKind::Apply {
                        expression: CheckedScalarExpression::IntegerBinary { .. },
                        ..
                    }
                )
            })
            .unwrap();
        for mutation in 0..4 {
            let mut mutated = checked.clone();
            let node = mutated
                .facts
                .values
                .scalar_computations
                .nodes
                .get_mut(handle);
            let CheckedScalarComputationKind::Apply {
                expression,
                operands,
            } = &mut node.kind
            else {
                unreachable!()
            };
            let CheckedScalarExpression::IntegerBinary { left, .. } = expression else {
                unreachable!()
            };
            match mutation {
                0 => {
                    **left = CheckedScalarExpression::Parameter {
                        position: 2,
                        primitive_type: PrimitiveType::U8,
                    }
                }
                1 => {
                    **left = CheckedScalarExpression::Parameter {
                        position: 0,
                        primitive_type: PrimitiveType::U16,
                    }
                }
                2 => *operands = arena::HandleSpan::empty(),
                3 => node.primitive_type = PrimitiveType::U16,
                _ => unreachable!(),
            }
            assert!(
                checked_trees_to_terminal_psi::lower_machine(&mutated, "value").is_err(),
                "mutation={mutation}, combined={combined}"
            );
        }
    }
}
