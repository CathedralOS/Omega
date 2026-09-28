use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

mod local_snapshots;
mod result_fields;
mod storage_snapshots;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()) {
        Ok(_) => assert!(accepted, "unproved scalar exit accepted:\n{source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("ensures")),
                "expected an exit guarantee rejection: {diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn short_circuit_returns_need_no_value_evidence_for_skipped_calls() {
    for (operator, left, expected, accepted) in [
        ("&&", "false", "false", true),
        ("||", "true", "true", true),
        ("&&", "false", "true", false),
        ("||", "true", "false", false),
        ("&&", "true", "true", false),
        ("||", "false", "true", false),
    ] {
        check(
            &format!(
                r#"
                machine unexplained() -> bool {{ true }}
                machine produce() -> bool
                ensures result == {expected}
                {{ {left} {operator} unexplained() }}
                "#,
            ),
            accepted,
        );
    }
}

#[test]
fn arithmetic_return_guarantees_use_selected_width_and_policy() {
    for (returned, expected) in [
        ("3u8 + 4u8", "7"),
        ("((255u8 as u8 in Wrapping) + 2) as u8", "1"),
        ("((255u8 as u8 in Saturating) + 2) as u8", "255"),
        ("(((0u8 as u8 in Wrapping) - 2) >> 1) as u8", "127"),
        ("(~0u8) as u8", "255"),
    ] {
        for accepted in [true, false] {
            let comparison = if accepted { "==" } else { "!=" };
            check(
                &format!(
                    r#"
                machine produce() -> u8
                ensures result {comparison} {expected}
                {{ {returned} }}
            "#
                ),
                accepted,
            );
        }
    }
}

#[test]
fn effectful_short_circuit_results_do_not_reread_mutated_operands() {
    for (initial, returned, wrong_guarantee) in [
        ("true", "flag && clear_and_return_true(&mut flag)", "false"),
        ("false", "flag || set_and_return_false(&mut flag)", "true"),
    ] {
        check(
            &format!(
                r#"
            machine clear_and_return_true(flag: &mut bool) -> bool {{ flag = false; true }}
            machine set_and_return_false(flag: &mut bool) -> bool {{ flag = true; false }}
            machine produce() -> bool
            ensures result == {wrong_guarantee}
            {{
                transition {{ _ -> finish() }}
                state finish() -> bool {{
                    let mut flag: bool = {initial};
                    {returned}
                }}
            }}
        "#
            ),
            false,
        );
    }
}

#[test]
fn pure_boolean_return_expressions_use_live_scalar_operands() {
    for (initial, returned, expected) in [
        ("true", "flag && true", "true"),
        ("false", "flag || false", "false"),
        ("false", "!flag", "true"),
    ] {
        check(
            &format!(
                r#"
            machine produce() -> bool
            ensures result == {expected}
            {{
                transition {{ _ -> finish() }}
                state finish() -> bool {{
                    let flag: bool = {initial};
                    {returned}
                }}
            }}
        "#
            ),
            true,
        );
    }
}

#[test]
fn named_state_boolean_results_discharge_boolean_guarantees() {
    for (value, accepted) in [("false", false), ("true", true)] {
        check(
            &format!(
                r#"
            machine produce() -> bool
            ensures result == true
            {{
                transition {{ _ -> finish() }}
                state finish() -> bool {{ {value} }}
            }}
        "#
            ),
            accepted,
        );
    }
}
