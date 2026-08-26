use super::*;

#[test]
fn resultless_law_accepts_an_exact_resultless_satisfier() {
    let source = r#"
        trait ReflexiveLaw {
            machine reflexive(value: u64)
            ensures value == value;
        }

        machine reflexive(value: u64)
        satisfies ReflexiveLaw::reflexive
        ensures value == value
        {
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("an exact resultless theorem satisfier should check");
}

#[test]
fn result_bearing_machine_cannot_satisfy_a_resultless_law() {
    let source = r#"
        trait ReflexiveLaw {
            machine reflexive(value: u64)
            ensures value == value;
        }

        machine reflexive(value: u64) -> u64
        satisfies ReflexiveLaw::reflexive
        ensures value == value
        {
            transition { _ -> value }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a runtime result must not satisfy a theorem-only slot");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        messages.contains("expected return `()`, got `u64`"),
        "unexpected diagnostics: {messages}"
    );
}
