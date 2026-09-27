use super::{TerminalExecutionResult, TerminalScalarValue, encoded_arms, execute};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
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
fn false_result_guarantee_is_not_treated_as_a_closed_tautology() {
    let source = source("result == 7u16").replace("{ 7u16 }", "{ 999u16 }");
    match crate::front_end::checked_program_result(&source) {
        Err(diagnostics) => assert!(!diagnostics.is_empty()),
        Ok(checked) => {
            assert!(
                checked_trees_to_lowered_psi::lower_machine(
                    &checked,
                    TerminalMachineSelection::Name("value")
                )
                .is_err()
            )
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
