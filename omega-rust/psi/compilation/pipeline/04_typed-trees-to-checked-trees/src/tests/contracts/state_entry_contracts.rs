use crate::CheckingRequest;
use crate::flow::check_against_whole_pass as lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

fn check(source: &str, accepted: bool, rejection: &str) {
    match lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()) {
        Ok(_) => assert!(accepted, "unproved contract accepted:\n{source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(rejection)),
                "expected {rejection:?}: {diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn tail_call_return_guarantees_do_not_leak_to_sibling_branches() {
    for (fallback, accepted) in [(7, true), (0, false)] {
        check(
            &format!(
                "machine produce() -> u64 ensures result == 7 {{ 7 }}
                 machine main(selected: bool) -> u64 ensures result == 7 {{
                     transition selected {{ true -> produce() false -> {fallback} }}
                 }}"
            ),
            accepted,
            "ensures",
        );
    }
}

#[test]
fn named_states_cannot_recover_entry_domains_after_mutating_calls() {
    for parameter in ["bytes", "destination"] {
        check(
            &format!(
                r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
            machine consume(bytes: &[u8; 4]) requires bytes in Utf8 {{}}
            machine main(bytes: &mut [u8; 4])
            requires bytes in Utf8
            {{
                corrupt(bytes);
                transition {{ _ -> finish(bytes) }}
                state finish({parameter}: &mut [u8; 4]) {{ consume({parameter}); }}
            }}
        "#
            ),
            false,
            "requires",
        );
    }
}

#[test]
fn captured_boolean_guard_preserves_exact_predicate_denotation() {
    for initializer in ["valid_utf8(bytes)", "!valid_utf8(bytes)"] {
        check(
            &format!(
                r#"
            machine validate(bytes: &[u8], expected: bool) {{
                let verdict: bool = {initializer};
                transition verdict == expected {{ true -> checked(expected, bytes) _ -> done() }}
                state checked(expected: bool, bytes: &[u8]) requires {initializer} == expected {{}}
                state done() {{}}
            }}
        "#
            ),
            true,
            "requires",
        );
    }
    check(
        r#"
        machine validate(bytes: &[u8]) {
            transition valid_utf8(bytes) { true -> done() _ -> invalid(bytes) }
            state invalid(bytes: &[u8]) requires !valid_utf8(bytes) {}
            state done() {}
        }
    "#,
        true,
        "requires",
    );
}

#[test]
fn captured_boolean_guard_rejects_changed_subject_kind_and_expectation() {
    for required in [
        "valid_utf8(other) == expected",
        "ascii_only(bytes) == expected",
        "valid_utf8(bytes) == !expected",
    ] {
        check(
            &format!(
                r#"
            machine validate(bytes: &[u8], other: &[u8], expected: bool) {{
                let verdict: bool = valid_utf8(bytes);
                transition verdict == expected {{ true -> checked(expected, bytes, other) _ -> done() }}
                state checked(expected: bool, bytes: &[u8], other: &[u8]) requires {required} {{}}
                state done() {{}}
            }}
        "#
            ),
            false,
            "requires",
        );
    }
    check(
        r#"
        machine valid_utf8(bytes: &[u8]) -> bool { true }
        machine validate(bytes: &[u8], expected: bool) {
            let verdict: bool = valid_utf8(bytes);
            transition verdict == expected { true -> checked(expected, bytes) _ -> done() }
            state checked(expected: bool, bytes: &[u8]) requires valid_utf8(bytes) == expected {}
            state done() {}
        }
    "#,
        false,
        "requires",
    );
}

#[test]
fn captured_boolean_guard_requires_preserved_observed_storage() {
    for (mutation, accepted) in [
        ("", true),
        ("bytes[0] = 255;", false),
        ("corrupt(bytes);", false),
    ] {
        check(
            &format!(
                r#"
            machine corrupt(bytes: &mut [u8]) requires bytes.len > 0 {{ bytes[0] = 255; }}
            machine validate(bytes: &mut [u8], expected: bool) requires bytes.len > 0 {{
                let verdict: bool = valid_utf8(bytes);
                {mutation}
                transition verdict == expected {{ true -> checked(expected, bytes) _ -> done() }}
                state checked(expected: bool, bytes: &mut [u8]) requires valid_utf8(bytes) == expected {{}}
                state done() {{}}
            }}
        "#
            ),
            accepted,
            "requires",
        );
    }
}
