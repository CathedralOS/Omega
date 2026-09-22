//! Nested and borrowed custom-view projections are proved literal by literal.

use crate::tests::front_end::typed_program;

fn prove(source: &str) {
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .unwrap_or_else(|diagnostics| panic!("termination: {source}\n{diagnostics:#?}"));
}

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
fn nested_projection_ranks_the_innermost_field_rebuilt_through_every_literal() {
    prove(NESTED);
    // Three levels: every record-typed step is an exact field of the record
    // before it, and every literal on the way is rebuilt.
    prove(
        &NESTED
            .replace(
                "data Inner { remaining: u64; }",
                "data Core { remaining: u64; }\ndata Inner { core: Core; }",
            )
            .replace(
                "countdown.inner.remaining",
                "countdown.inner.core.remaining",
            )
            .replace(
                "Inner { remaining: countdown.inner.core.remaining - 1 }",
                "Inner { core: Core { remaining: countdown.inner.core.remaining - 1 } }",
            ),
    );
    // A store-enforced range on the nested field still bounds the produced rank.
    prove(
        &NESTED
            .replace(
                "data Inner { remaining: u64; }",
                "data Inner { remaining: u64 [0..=5]; }",
            )
            .replace(
                "terminates by countdown -> Countdown::Remaining;",
                "terminates by countdown -> Countdown::Remaining in 0..=5;",
            ),
    );
}

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

const BORROWED: &str = r#"
data Card { power: u64; }
measure Card::PowerOrder(card: Card) -> u64 { card.power }
machine walk(card: &Card)
terminates by card -> Card::PowerOrder;
-> u64 {
    transition card.power > 0 {
        true -> walk(&Card { power: card.power - 1 })
        false -> card.power
    }
}
"#;

#[test]
fn borrowed_subject_ranks_the_referent_field_while_the_binding_stays_unwritten() {
    prove(BORROWED);
    // Rebinding the reference before the edge invalidates the ranked path.
    reject(
        &BORROWED
            .replace("machine walk(card: &Card)", "machine walk(mut card: &Card)")
            .replace(
                "    transition card.power > 0",
                "    card = card; transition card.power > 0",
            ),
    );
    // A forwarded borrow does not decrease.
    reject(&BORROWED.replace("walk(&Card { power: card.power - 1 })", "walk(card)"));
}
