//! Nested and borrowed custom-view projections are proved literal by literal.

use crate::tests::front_end::typed_program;

fn reject(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed_program(source))
        .expect_err(source);
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove the `terminates by`")),
        "{source}\n{diagnostics:#?}"
    );
}

const NESTED: &str = r#"
data Inner { remaining: u64; }
data Countdown { label: u64; inner: Inner; }
measure Countdown::Remaining(countdown: Countdown) -> u64 { countdown.inner.remaining }
machine walk(countdown: Countdown)
terminates by countdown -> Countdown::Remaining;
-> u64 {
    transition countdown.inner.remaining > 0 {
        true -> walk(Countdown { label: countdown.label, inner: Inner { remaining: countdown.inner.remaining - 1 } })
        false -> countdown.inner.remaining
    }
}
"#;

#[test]
fn nested_projection_rejects_a_forwarded_or_wrong_field_or_guard() {
    // Forwarding the inner record keeps the ranked field unchanged.
    reject(&NESTED.replace(
        "inner: Inner { remaining: countdown.inner.remaining - 1 }",
        "inner: countdown.inner",
    ));
    // Decreasing the outer `label` is not a decrease of the ranked field.
    reject(
        &NESTED
            .replace("label: countdown.label,", "label: countdown.label - 1,")
            .replace(
                "Inner { remaining: countdown.inner.remaining - 1 }",
                "Inner { remaining: countdown.inner.remaining }",
            ),
    );
    // The guard must bound the ranked projection itself.
    reject(&NESTED.replace(
        "transition countdown.inner.remaining > 0",
        "transition countdown.label > 0",
    ));
    // A same-named field of another record is not a step of this path.
    reject(
        &NESTED
            .replace(
                "data Countdown { label: u64; inner: Inner; }",
                "data Other { remaining: u64; }\ndata Countdown { label: u64; inner: Inner; }",
            )
            .replace(
                "inner: Inner { remaining: countdown.inner.remaining - 1 }",
                "inner: Other { remaining: countdown.inner.remaining - 1 }",
            ),
    );
}
