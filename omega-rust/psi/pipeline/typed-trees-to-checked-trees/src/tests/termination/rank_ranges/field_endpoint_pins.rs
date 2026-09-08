use super::{lower_typed_trees, typed};

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_remainder_limit/main.omg"
));
const ENDPOINT: &str = "countdown.limit % 5 + 6";

const FORWARDED: &str = r#"
    data Limits {
        limit: u64;
        divisor: u64 [3..=5];
    }
    machine walk(remaining: u64 [0..=5], limits: Limits)
    terminates by remaining -> Nat::Descending in 0..(limits.limit % limits.divisor + 6);
    -> u64 {
        transition remaining > 0 {
            true -> walk(remaining - 1, limits)
            false -> remaining
        }
    }
"#;
const FORWARDED_ENDPOINT: &str = "limits.limit % limits.divisor + 6";

fn accepts(source: &str) {
    lower_typed_trees(typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn rejects_range(source: &str) {
    let diagnostics = lower_typed_trees(typed(source))
        .expect_err("computed field endpoints must form and preserve every exact input");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
    );
}

#[test]
fn remainder_and_quotient_field_endpoints_check_with_direct_reconstruction() {
    accepts(COUNTDOWN);
    // Remainder bounds above do not need a bounded dividend; quotient bounds do.
    accepts(
        &COUNTDOWN
            .replace("limit: u64;", "limit: u64 [0..=20];")
            .replace(ENDPOINT, "countdown.limit / 5 + 6"),
    );
}

#[test]
fn remainder_and_quotient_field_endpoints_check_with_direct_forwarding() {
    accepts(FORWARDED);
    accepts(
        &FORWARDED
            .replace("limit: u64;", "limit: u64 [0..=20];")
            .replace("limits.limit %", "limits.limit /"),
    );
}

#[test]
fn reconstructed_endpoint_pins_every_leaf_on_every_self_edge() {
    let source = COUNTDOWN
        .replace(
            "limit: u64;",
            "limit: u64 [0..=20]; divisor: u64 [3..=5]; padding: u64 [6..=7];",
        )
        .replace(ENDPOINT, "countdown.limit % countdown.divisor + countdown.padding")
        .replace(
            "limit: countdown.limit",
            "limit: countdown.limit, divisor: countdown.divisor, padding: countdown.padding",
        )
        .replace(
            "    transition countdown.remaining > 0 {",
            "    transition countdown.remaining > 1 {\n        true -> walk(Countdown { remaining: countdown.remaining - 2, limit: countdown.limit, divisor: countdown.divisor, padding: countdown.padding })\n    }\n    transition countdown.remaining > 0 {",
        );
    for source in [
        source.clone(),
        source.replace("countdown.limit %", "countdown.limit /"),
    ] {
        accepts(&source);
        for (field, replacement) in [("limit", "0"), ("divisor", "3"), ("padding", "6")] {
            let preserved = format!("{field}: countdown.{field}");
            assert_eq!(source.matches(&preserved).count(), 2);
            // Each replacement remains in its declared domain, and the endpoint
            // still exceeds every rank. Neither fact pins the original value.
            for occurrence in [0, 1] {
                let (offset, _) = source.match_indices(&preserved).nth(occurrence).unwrap();
                let mut changed = source.clone();
                changed.replace_range(
                    offset..offset + preserved.len(),
                    &format!("{field}: {replacement}"),
                );
                rejects_range(&changed);
            }
        }
    }
}

#[test]
fn same_named_field_on_another_parameter_cannot_replace_endpoint_input() {
    let forwarded = FORWARDED
        .replace("limits: Limits)", "limits: Limits, other: Limits)")
        .replace("remaining - 1, limits)", "remaining - 1, limits, other)");
    accepts(&forwarded);
    let reconstructed = forwarded.replace(
        "remaining - 1, limits, other)",
        "remaining - 1, Limits { limit: limits.limit, divisor: limits.divisor }, other)",
    );
    accepts(&reconstructed);
    rejects_range(&reconstructed.replace("limit: limits.limit", "limit: other.limit"));
    rejects_range(&forwarded.replace(
        "remaining - 1, limits, other)",
        "remaining - 1, other, other)",
    ));
}

#[test]
fn sibling_and_other_owner_fields_cannot_replace_endpoint_input() {
    let sibling = COUNTDOWN
        .replace("limit: u64;", "limit: u64; spare: u64;")
        .replace(
            "limit: countdown.limit",
            "limit: countdown.limit, spare: countdown.spare",
        );
    accepts(&sibling);
    rejects_range(&sibling.replace("limit: countdown.limit", "limit: countdown.spare"));
    // A valid Limits reconstruction can still read a different owner's
    // same-spelled field; nominal argument rejection must not satisfy this test.
    let other_owner = format!(
        "data Other {{ limit: u64; }} {}",
        FORWARDED
            .replace("limits: Limits)", "limits: Limits, other: Other)")
            .replace(
                "remaining - 1, limits)",
                "remaining - 1, Limits { limit: limits.limit, divisor: limits.divisor }, other)"
            )
    );
    accepts(&other_owner);
    rejects_range(&other_owner.replace("limit: limits.limit", "limit: other.limit"));
}

#[test]
fn unchanged_endpoint_interval_does_not_establish_unchanged_value() {
    let source = COUNTDOWN
        .replace("limit: u64;", "limit: u64 [0..=4]; spare: u64 [0..=4];")
        .replace(
            "limit: countdown.limit",
            "limit: countdown.limit, spare: countdown.spare",
        );
    accepts(&source);
    // Both fields give exactly the same endpoint interval [6, 10].
    rejects_range(&source.replace("limit: countdown.limit", "limit: countdown.spare"));
    // A field absent from the endpoint remains free to change.
    accepts(&source.replace("spare: countdown.spare", "spare: 0"));
}

#[test]
fn endpoint_formation_rejects_zero_divisors_and_hidden_intermediate_overflow() {
    for endpoint in [
        "countdown.limit % 0 + 6",
        "countdown.limit / 0 + 6",
        "(countdown.limit + 1) % 5 + 6",
        "(countdown.limit + 1) / 5 + 6",
    ] {
        rejects_range(&COUNTDOWN.replace(ENDPOINT, endpoint));
    }
}

#[test]
fn endpoint_literals_retain_rational_meaning_and_landing_boundaries() {
    for endpoint in [
        "countdown.limit % 5 + (1 / 2 * 12)",
        "countdown.limit % 5 + (0.5 * 12)",
        "countdown.limit / 5 + (1 / 2 * 12)",
        "1 / 2 * 12",
    ] {
        accepts(
            &COUNTDOWN
                .replace("limit: u64;", "limit: u64 [0..=20];")
                .replace(ENDPOINT, endpoint),
        );
    }
    for endpoint in [
        "countdown.limit % 5 + (1 / 2)",
        "countdown.limit % 5 + (255u8 + 1u8)",
        "countdown.limit % 5 + 6u8",
    ] {
        rejects_range(&COUNTDOWN.replace(ENDPOINT, endpoint));
    }
    rejects_range(&COUNTDOWN.replace("in 0..", "in (1 / 2 * 2).."));
    let diagnostics = lower_typed_trees(typed(&COUNTDOWN.replace(ENDPOINT, "12 % 7")))
        .expect_err("anonymous remainder cannot form");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires an integer-typed operand")
    }));
}

#[test]
fn selected_authored_quotient_and_remainder_cannot_inherit_builtin_bounds() {
    for (operator, name) in [("%", "remainder"), ("/", "quotient")] {
        let source = COUNTDOWN
            .replace("limit: u64;", "limit: u64 [0..=20];")
            .replace(ENDPOINT, &format!("countdown.limit {operator} 5 + 6"));
        accepts(&source);
        rejects_range(&format!(
            "operator {operator} u64::{name}(left: u64, right: u64) -> u64; {source}"
        ));
    }
}

#[test]
fn legal_prefix_and_alias_writes_cannot_supply_fixed_endpoint_evidence() {
    let mutable = FORWARDED.replace("limits: Limits)", "mut limits: Limits)");
    for prefix in [
        "limits.limit = 0;",
        "limits = Limits { limit: 0, divisor: 3 };",
        "let alias: &mut u64 = &mut limits.limit; reset(alias);",
    ] {
        let source = format!(
            "machine reset(value: &mut u64) {{ value = 0; }} {}",
            mutable.replace(
                "        transition",
                &format!("        {prefix}\n        transition")
            )
        );
        // Legal mutable storage isolates endpoint rejection from invalid writes
        // to immutable parameters. Formation already excludes mutable inputs.
        accepts(&source.replace(FORWARDED_ENDPOINT, "6"));
        rejects_range(&source);
    }
}

#[test]
fn disjoint_prefix_writes_preserve_endpoint_fields() {
    let source = FORWARDED
        .replace("limits: Limits)", "limits: Limits, marker: u64)")
        .replace(
            "        transition",
            "        let mut scratch: u64 = 0;\n        transition",
        )
        .replace("remaining - 1, limits)", "remaining - 1, limits, marker)");
    let helper = "machine reset(value: &mut u64) { value = 0; }";
    accepts(&format!("{helper} {source}"));
    let prefix = source.replace(
        "        transition",
        "        reset(&mut scratch);\n        transition",
    );
    accepts(&format!("{helper} {prefix}"));
}

#[test]
fn hoisted_operand_calls_still_need_named_state_endpoint_transport() {
    let source = FORWARDED
        .replace("limits: Limits)", "limits: Limits, marker: u64)")
        .replace(
            "        transition",
            "        let mut scratch: u64 = 0;\n        transition",
        )
        .replace(
            "remaining - 1, limits)",
            "remaining - 1, limits, reset(&mut scratch))",
        );
    let source = format!("machine reset(value: &mut u64) -> u64 {{ value = 0; 0 }} {source}");
    let program = typed(&source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .unwrap();
    assert!(
        program.machine_states(machine).len() > 1,
        "the operand call introduces a state arrival even without authored named states"
    );
    // This valid language use exceeds the single-state endpoint proof tier.
    // A disjoint write frame cannot replace the missing arrival mapping.
    rejects_range(&source);
}
