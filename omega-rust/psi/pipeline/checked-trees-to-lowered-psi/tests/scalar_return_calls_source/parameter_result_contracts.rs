use super::*;

fn source(parameter: &str, requires: &str) -> String {
    format!(
        r#"
        machine bounded({parameter}) -> u16
        {requires}
        ensures result == input
        {{ input }}
        machine value(selected: bool, input: u16) -> bool
        requires true == true
        ensures true == true
        {{
            transition selected {{
                true -> finish(selected && ((bounded(input % 256u16) as u8) == 7u8))
                false -> finish(false)
            }}
            state finish(result: bool) -> bool {{ result }}
        }}
    "#
    )
}

fn check(source: &str) {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    for combined in [false, true] {
        let artifact = encoded_arms(source, combined);
        for selected in [false, true] {
            for input in [0, 7, 255, 256, 263, 65535] {
                assert_eq!(
                    execute(
                        &artifact,
                        &[
                            TerminalScalarValue::Boolean(selected),
                            TerminalScalarValue::Integer {
                                scalar_type: integer_type,
                                value: IntegerValue::Unsigned(input)
                            },
                        ]
                    )
                    .unwrap(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(
                        selected && input % 256 == 7
                    ))
                );
            }
        }
    }
}

#[test]
fn result_alias_rejoins_the_argument_proved_at_call_entry() {
    check(&source("input: u16", "requires input < 256u16"));
}

#[test]
fn reversed_result_alias_and_parameter_bounds_preserve_meaning() {
    for requirement in [
        "requires 256u16 > input",
        "requires input <= 255u16",
        "requires 255u16 >= input",
        "requires input >= 0u16 && input < 256u16",
        "requires input < 128u16 || input <= 255u16",
    ] {
        check(&source("input: u16", requirement).replace("result == input", "input == result"));
    }
}

#[test]
fn declared_parameter_ranges_are_actual_call_requirements() {
    for parameter in ["input: u16 [0..=255]", "input: u16 [0..256]"] {
        check(&source(parameter, "requires 7u16 == 7u16"));
    }
}

#[test]
fn result_and_formal_namespaces_keep_mixed_parameter_positions() {
    let source = source(
        "flag: bool, ignored: u16, input: u16",
        "requires input < 256u16",
    )
    .replace("bounded(input %", "bounded(!selected, 500u16, input %");
    check(&source);
}

#[test]
fn result_alias_survives_an_explicit_named_state_argument() {
    check(&source("input: u16", "requires input < 256u16").replace(
        "{ input }",
        "{ transition { _ -> finish(input) } state finish(saved: u16) -> u16 { saved } }",
    ));
}

#[test]
fn conjoined_and_disjoined_result_aliases_preserve_their_propositions() {
    for guarantee in [
        "result == input && input == result",
        "result == input || result < input",
        "result <= input",
        "input >= result",
    ] {
        check(
            &source("input: u16", "requires input < 256u16").replace("result == input", guarantee),
        );
    }
}

#[test]
fn a_skipped_contracted_call_does_not_evaluate_its_crashing_argument() {
    let source = source("ignored: bool, input: u16", "requires input < 256u16")
        .replace("bounded(input %", "bounded(effect(), input %")
        .replace(
            "machine value(",
            "machine effect() -> bool\ncrashes Abort\n{ crash Abort; }\nmachine value(",
        )
        .replace(
            "ensures true == true",
            "ensures true == true\ncrashes Abort",
        );
    for combined in [false, true] {
        let artifact = encoded_arms(&source, combined);
        let arguments = |selected| {
            [
                TerminalScalarValue::Boolean(selected),
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
                    value: IntegerValue::Unsigned(263),
                },
            ]
        };
        assert_eq!(
            execute(&artifact, &arguments(false)).unwrap(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(false))
        );
        assert!(matches!(
            execute(&artifact, &arguments(true)),
            Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::Crash(_)
            ))
        ));
    }
}

#[test]
fn parameter_contracts_cannot_reuse_proofs_for_different_call_arguments() {
    use semantic_vocabulary::Proposition;
    use terminal_psi::OperationKind;

    for combined in [false, true] {
        let source = source("ignored: u16, input: u16", "requires input < 256u16")
            .replace("bounded(input %", "bounded(500u16, input %");
        let artifact = encoded_arms(&source, combined);
        let original = terminal_codec::decode_module(&artifact.0).unwrap();
        let callee_index = original
            .machines
            .iter()
            .position(|machine| {
                machine.parameters.len() == 2
                    && machine.contract.ensures.iter().any(|clause| {
                        matches!(
                            clause.proposition,
                            Proposition::Equal(
                                semantic_vocabulary::ScalarTerm::Value { .. },
                                semantic_vocabulary::ScalarTerm::Value { .. },
                            )
                        )
                    })
            })
            .expect("parameter-relative callee");
        let callee_id = original.machines[callee_index].id;
        assert_eq!(original.machines[callee_index].contract.requires.len(), 1);

        for change in 0..3 {
            let mut changed = original.clone();
            match change {
                0 => changed.machines[callee_index].contract.requires[0] = Proposition::Truth,
                1 => {
                    changed.machines[callee_index].contract.ensures[0].proposition =
                        Proposition::Truth
                }
                _ => {
                    let mut calls = 0;
                    for operation in changed
                        .machines
                        .iter_mut()
                        .flat_map(|machine| &mut machine.blocks)
                        .flat_map(|block| &mut block.operations)
                    {
                        if let OperationKind::Call {
                            callee, arguments, ..
                        } = &mut operation.kind
                            && *callee == callee_id
                        {
                            arguments.swap(0, 1);
                            calls += 1;
                        }
                    }
                    assert!(calls > 0, "must alter an actual invocation");
                }
            }
            let changed = (encode_module(&changed).unwrap(), artifact.1.clone());
            for selected in [false, true] {
                assert!(
                    execute(
                        &changed,
                        &[
                            TerminalScalarValue::Boolean(selected),
                            TerminalScalarValue::Integer {
                                scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
                                value: IntegerValue::Unsigned(263),
                            },
                        ]
                    )
                    .is_err(),
                    "change={change}, selected={selected}, combined={combined}"
                );
            }
        }
    }
}
