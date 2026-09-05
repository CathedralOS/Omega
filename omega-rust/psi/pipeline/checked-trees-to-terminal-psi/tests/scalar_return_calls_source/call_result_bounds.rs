use super::*;

fn source(guarantee: &str) -> String {
    r#"
        machine bounded() -> u16
        requires 7u16 == 7u16
        ensures GUARANTEE
        { 7u16 }
        machine value(selected: bool) -> bool
        requires true == true
        ensures true == true
        {
            transition selected {
                true -> finish(selected && ((bounded() as u8) == 7u8))
                false -> finish(false)
            }
            state finish(result: bool) -> bool { result }
        }
    "#
    .replace("GUARANTEE", guarantee)
}

fn check_narrowing(guarantee: &str) {
    let source = source(guarantee);
    for combined in [false, true] {
        let artifact = encoded_arms(&source, combined);
        for selected in [false, true] {
            assert_eq!(
                execute(&artifact, &[TerminalScalarValue::Boolean(selected)]).unwrap(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(selected))
            );
        }
    }
}

#[test]
fn nested_narrowing_uses_the_exact_callee_result_guarantee() {
    check_narrowing("result == 7u16");
    check_narrowing("7u16 == result");
}

#[test]
fn nested_narrowing_uses_inclusive_result_bounds() {
    check_narrowing("result <= 255u16");
    check_narrowing("255u16 >= result");
}

#[test]
fn nested_narrowing_uses_strict_result_bounds() {
    check_narrowing("result < 256u16");
    check_narrowing("256u16 > result");
}

#[test]
fn nested_narrowing_uses_conjoined_result_bounds() {
    check_narrowing("result >= 7u16 && result < 256u16");
    check_narrowing("result > 6u16 && result <= 255u16");
    check_narrowing("result != 0u16 && result <= 255u16");
    check_narrowing("result >= 7u16 && result <= 255u16 && result >= 7u16");
}

#[test]
fn nested_narrowing_uses_disjoined_result_bounds() {
    check_narrowing("result == 7u16 || result == 9u16");
    check_narrowing("result == 9u16 || result == 7u16 || result == 9u16");
}

#[test]
fn changed_callee_body_or_guarantee_rejects_stale_serialized_proof() {
    use semantic_vocabulary::Proposition;
    use terminal_psi::OperationKind;

    for combined in [false, true] {
        let artifact = encoded_arms(&source("result == 7u16"), combined);
        let original = terminal_codec::decode_module(&artifact.0).unwrap();
        let callee_index = original
            .machines
            .iter()
            .position(|machine| {
                machine.contract.ensures.iter().any(|clause| {
                    matches!(
                        clause.proposition,
                        Proposition::Equal(semantic_vocabulary::ScalarTerm::Value { .. }, _)
                    )
                })
            })
            .expect("callee guarantee names the actual result");
        for change_body in [false, true] {
            let mut changed = original.clone();
            let callee = &mut changed.machines[callee_index];
            if change_body {
                let operation = callee
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|operation| {
                        matches!(
                            operation.kind,
                            OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(7)
                            }
                        )
                    })
                    .expect("callee returned literal");
                operation.kind = OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(999),
                };
            } else {
                callee.contract.ensures[0].proposition = Proposition::Truth;
            }
            // Keep the original proof bytes; neither a changed body nor a
            // weakened postcondition can reuse their result/cast evidence.
            let changed = (encode_module(&changed).unwrap(), artifact.1.clone());
            for selected in [false, true] {
                assert!(
                    execute(&changed, &[TerminalScalarValue::Boolean(selected)]).is_err(),
                    "change_body={change_body}, selected={selected}, combined={combined}"
                );
            }
        }
    }
}

#[test]
fn false_result_guarantee_is_not_treated_as_a_closed_tautology() {
    let source = source("result == 7u16").replace("{ 7u16 }", "{ 999u16 }");
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    match lower_typed_trees(typed) {
        Err(diagnostics) => assert!(!diagnostics.is_empty()),
        Ok(checked) => {
            assert!(checked_trees_to_terminal_psi::lower_machine(&checked, "value").is_err())
        }
    }
}

#[test]
fn a_normal_return_bound_does_not_execute_a_skipped_crashing_callee() {
    let source = source("result == 7u16")
        .replace("bounded() ->", "bounded(should_crash: bool) ->")
        .replace(
            "{ 7u16 }",
            r#"crashes Abort
        {
            transition should_crash {
                true -> fail()
                false -> success()
            }
            state fail() -> u16 { crash Abort; }
            state success() -> u16 { 7u16 }
        }"#,
        )
        .replace("bounded()", "bounded(true)")
        .replace("value(selected: bool)", "value(selected: bool, flag: bool)")
        .replace(
            "ensures true == true",
            "ensures true == true\ncrashes Abort",
        )
        .replace("selected &&", "flag &&");
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
                    assert!(matches!(result,
                        Err(TerminalArtifactInterpretError::Execution(
                            TerminalInterpretError::Crash(crash)
                        )) if crash.cause == terminal_psi::CrashCause::Abort));
                } else {
                    assert_eq!(
                        result.unwrap_or_else(|error| panic!(
                            "selected={selected}, flag={flag}, combined={combined}: {error:?}"
                        )),
                        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(false))
                    );
                }
            }
        }
    }
}

#[test]
fn signed_result_endpoints_support_exact_narrowing() {
    for guarantee in [
        "result == -7i16",
        "-7i16 == result",
        "result > -129i16 && result < 128i16",
    ] {
        let source = source(guarantee)
            .replace("u16", "i16")
            .replace("u8", "i8")
            .replace("{ 7i16 }", "{ -7i16 }")
            .replace("== 7i8", "== -7i8");
        for combined in [false, true] {
            let artifact = encoded_arms(&source, combined);
            for selected in [false, true] {
                assert_eq!(
                    execute(&artifact, &[TerminalScalarValue::Boolean(selected)]).unwrap(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(selected))
                );
            }
        }
    }
}
