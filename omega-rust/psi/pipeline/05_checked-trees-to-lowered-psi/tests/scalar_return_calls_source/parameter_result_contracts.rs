use super::{
    IntegerSign, IntegerType, IntegerValue, TerminalExecutionResult, TerminalScalarValue,
    encoded_arms, execute,
};
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
