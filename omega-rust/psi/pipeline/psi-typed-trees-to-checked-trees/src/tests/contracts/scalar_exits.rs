use super::*;

mod local_snapshots;
mod storage_snapshots;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(parse_typed_trees(source)) {
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
fn arithmetic_return_reads_only_live_binding_values() {
    for (replacement, accepted) in [("", true), ("replace(&mut value);", false)] {
        check(
            &format!(
                r#"
            machine replace(value: &mut u8) {{ value = 8; }}
            machine produce() -> u8
            ensures result == 7
            {{
                let value: u8 = 3;
                {replacement}
                ((value as u8 in Wrapping) + 4) as u8
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn signed_return_guarantees_use_selected_division_overflow_policy() {
    for (policy, operator, expected) in [
        ("Wrapping", "/", "-128"),
        ("Saturating", "/", "127"),
        ("Wrapping", "%", "0"),
        ("Saturating", "%", "0"),
    ] {
        for accepted in [true, false] {
            let comparison = if accepted { "==" } else { "!=" };
            check(
                &format!(
                    r#"
                machine produce() -> i8
                ensures result {comparison} {expected}
                {{ (((-127i8 - 1i8) as i8 in {policy}) {operator} -1i8) as i8 }}
            "#
                ),
                accepted,
            );
        }
    }
}

#[test]
fn arithmetic_return_after_state_transfer_keeps_the_current_binding() {
    for (value, accepted) in [(3, true), (4, false)] {
        check(
            &format!(
                r#"
            machine produce(unused: bool) -> u8
            ensures result == 7
            {{
                transition {{ _ -> finish({value}) }}
                state finish(current: u8) -> u8 {{
                    ((current as u8 in Wrapping) + 4) as u8
                }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn scalar_values_follow_explicit_multi_hop_state_arguments() {
    for (forwarded, accepted) in [("value", true), ("other", false)] {
        check(
            &format!(
                r#"
            machine produce() -> u8
            ensures result == 7
            {{
                let first: u8 = 3;
                let second: u8 = 4;
                transition {{ _ -> relay(first, second) }}
                state relay(value: u8, other: u8) -> u8 {{
                    transition {{ _ -> finish({forwarded}) }}
                }}
                state finish(current: u8) -> u8 {{
                    ((current as u8 in Wrapping) + 4) as u8
                }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn scalar_state_arrival_values_require_agreement_from_every_predecessor() {
    for (alternative, accepted) in [("3", true), ("4", false), ("unknown", false)] {
        check(
            &format!(
                r#"
            machine produce(flag: bool, unknown: u8) -> u8
            ensures result == 7
            {{
                transition flag {{ true -> finish(3) false -> finish({alternative}) }}
                state finish(current: u8) -> u8 {{
                    ((current as u8 in Wrapping) + 4) as u8
                }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn resolved_result_parameter_is_not_the_synthetic_return_value() {
    for (stored, returned, accepted) in [(8, 7, false), (7, 8, true)] {
        check(
            &format!(
                r#"
            machine produce(result: &mut u64) -> u64
            ensures result == 7
            {{
                result = {stored};
                {returned}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn scalar_exit_equality_retains_full_unsigned_integer_precision() {
    for (returned, accepted) in [
        ("18446744073709551614u64", false),
        ("18446744073709551615u64", true),
    ] {
        check(
            &format!(
                r#"
            machine produce() -> u64
            ensures result == 18446744073709551615u64
            {{
                transition {{ _ -> finish() }}
                state finish() -> u64 {{ let value: u64 = {returned}; value }}
            }}
        "#
            ),
            accepted,
        );
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
fn named_state_literal_results_discharge_exact_exit_values() {
    for (value, accepted) in [(8, false), (7, true)] {
        check(
            &format!(
                r#"
            machine produce() -> u64
            ensures result == 7
            {{
                transition {{ _ -> finish() }}
                state finish() -> u64 {{ {value} }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn expression_return_arms_use_their_own_exact_results() {
    for (left, right, accepted) in [(8, 7, false), (7, 8, false), (7, 7, true)] {
        check(
            &format!(
                r#"
            machine produce(flag: bool) -> u64
            ensures result == 7
            {{
                transition {{ _ -> finish(flag) }}
                state finish(selected: bool) -> u64 {{
                    transition selected {{ true -> {left} false -> {right} }}
                }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn named_state_local_return_uses_the_latest_assignment() {
    for (initial, replacement, accepted) in [(7, 8, false), (8, 7, true)] {
        check(
            &format!(
                r#"
            machine produce() -> u64
            ensures result == 7
            {{
                transition {{ _ -> finish() }}
                state finish() -> u64 {{
                    let mut value: u64 = {initial};
                    value = {replacement};
                    value
                }}
            }}
        "#
            ),
            accepted,
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

#[test]
fn scalar_exit_comparisons_use_the_materialized_result() {
    for (value, accepted) in [(6, false), (9, false), (7, true), (8, true)] {
        check(
            &format!(
                r#"
            machine produce() -> u64
            ensures result >= 7 && result < 9
            {{
                transition {{ _ -> finish() }}
                state finish() -> u64 {{ {value} }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn scalar_output_guarantees_follow_renamed_two_hop_origins() {
    for (value, accepted) in [(8, false), (7, true)] {
        check(
            &format!(
                r#"
            machine write(output: &mut u64)
            ensures output == 7
            {{
                transition {{ _ -> middle(output) }}
                state middle(forwarded: &mut u64) {{
                    transition {{ _ -> finish(forwarded) }}
                }}
                state finish(destination: &mut u64) {{ destination = {value}; }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn later_calls_retire_only_overlapping_scalar_output_values() {
    for (call, accepted) in [
        ("corrupt(destination);", false),
        ("replace(destination, unknown);", false),
        ("inspect(destination);", true),
        ("corrupt(other);", true),
    ] {
        check(
            &format!(
                r#"
            machine corrupt(value: &mut u64) {{ value = 8; }}
            machine replace(value: &mut u64, replacement: u64) {{ value = replacement; }}
            machine inspect(value: &u64) {{}}
            machine write(output: &mut u64, spare: &mut u64, replacement: u64)
            ensures output == 7
            {{
                transition {{ _ -> finish(output, spare, replacement) }}
                state finish(destination: &mut u64, other: &mut u64, unknown: u64) {{
                    destination = 7;
                    {call}
                }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn scalar_output_values_do_not_cross_swapped_origins() {
    for (arguments, accepted) in [("spare, output", false), ("output, spare", true)] {
        check(
            &format!(
                r#"
            machine write(output: &mut u64, spare: &mut u64)
            ensures output == 7
            {{
                transition {{ _ -> middle({arguments}) }}
                state middle(first: &mut u64, second: &mut u64) {{
                    transition {{ _ -> finish(first, second) }}
                }}
                state finish(destination: &mut u64, other: &mut u64) {{
                    destination = 7;
                    other = 8;
                }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn frozen_scalar_copy_cannot_prove_the_mutated_source() {
    for (guarantee, accepted) in [("output == 7", false), ("result == 7", true)] {
        check(
            &format!(
                r#"
            machine write(output: &mut u64) -> u64
            ensures {guarantee}
            {{
                transition {{ _ -> finish(output) }}
                state finish(destination: &mut u64) -> u64 {{
                    destination = 7;
                    let saved: u64 = destination;
                    destination = 8;
                    saved
                }}
            }}
        "#
            ),
            accepted,
        );
    }
}
