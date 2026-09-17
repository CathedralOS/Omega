//! The relational range route transports the ranked field, the endpoints and
//! the entry facts through a nested projection path or a borrowed subject,
//! proving membership, pinning and descent exactly as the direct owned-field
//! route does.
use super::{lower_typed_trees, typed};

const NESTED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_nested_projection_range_relational/main.omg"
));
const NESTED_LIMIT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_nested_projection_range_pinned_limit/main.omg"
));
const BORROWED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_borrowed_projection_range/main.omg"
));

const NESTED_PRECONDITION: &str = "requires countdown.inner.remaining <= ceiling;";
const NESTED_REBUILD: &str = "Countdown { label: countdown.label, inner: Inner { remaining: countdown.inner.remaining - amount } }, ceiling, amount";
const BORROWED_PRECONDITION: &str = "requires card.power <= ceiling;";

fn prove_termination(source: &str) {
    crate::checks::termination::check_machine_termination(&typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn reject_range(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed(source))
        .expect_err("the authored projected rank range must be proved");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
    );
}

fn reject_termination(source: &str) {
    crate::checks::termination::check_machine_termination(&typed(source)).expect_err(source);
}

#[test]
fn nested_projection_relation_checks_through_complete_lowering() {
    for source in [NESTED, NESTED_LIMIT, BORROWED] {
        crate::checks::termination::check_machine_termination(&typed(source))
            .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
        lower_typed_trees(typed(source))
            .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
    }
}

#[test]
fn nested_projection_relation_proves_nonzero_floor_and_exclusive_ceiling() {
    let nonzero = NESTED
        .replace(
            NESTED_PRECONDITION,
            "requires 1 <= countdown.inner.remaining && countdown.inner.remaining <= ceiling;",
        )
        .replace("in 0..=ceiling", "in 1..=ceiling")
        .replace("remaining >= amount {", "remaining >= amount + 1 {");
    prove_termination(&nonzero);
    reject_range(&nonzero.replace("remaining >= amount + 1 {", "remaining >= amount {"));
    reject_range(&nonzero.replace("1 <= countdown.inner.remaining && ", ""));
    let exclusive = NESTED
        .replace(
            NESTED_PRECONDITION,
            "requires countdown.inner.remaining < ceiling;",
        )
        .replace("in 0..=ceiling", "in 0..ceiling");
    prove_termination(&exclusive);
    reject_range(&exclusive.replace("remaining < ceiling;", "remaining <= ceiling;"));
}

#[test]
fn nested_projection_relation_requires_entry_evidence_on_the_exact_path() {
    for requirement in [
        "",
        "requires countdown.inner.remaining >= ceiling;",
        "requires countdown.label <= ceiling;",
        "requires amount <= ceiling;",
    ] {
        reject_range(&NESTED.replace(NESTED_PRECONDITION, requirement));
    }
    // A sibling record with the same field spelling is a foreign owner.
    let foreign = NESTED
        .replace(
            "data Countdown { label: u64; inner: Inner; }",
            "data Other { remaining: u64 [0..=5]; } data Countdown { label: u64; inner: Inner; other: Other; }",
        )
        .replace(
            "inner: Inner { remaining: countdown.inner.remaining - amount } }",
            "inner: Inner { remaining: countdown.inner.remaining - amount }, other: countdown.other }",
        );
    prove_termination(&foreign);
    reject_range(&foreign.replace(
        NESTED_PRECONDITION,
        "requires countdown.other.remaining <= ceiling;",
    ));
}

#[test]
fn nested_projection_relation_pins_endpoints_on_every_edge() {
    reject_range(&NESTED.replace("}, ceiling, amount)", "}, 5, amount)"));
    reject_range(&NESTED.replace("}, ceiling, amount)", "}, ceiling - 1, amount)"));
    let two_edges = NESTED.replace(
        "    transition countdown.inner.remaining >= amount {",
        &format!(
            "    transition countdown.inner.remaining >= amount + 1 {{\n        true -> walk({})\n    }}\n    transition countdown.inner.remaining >= amount {{",
            NESTED_REBUILD.replace("remaining - amount", "remaining - (amount + 1)")
        ),
    );
    prove_termination(&two_edges);
    reject_range(&two_edges.replace(
        "remaining - (amount + 1) } }, ceiling",
        "remaining - (amount + 1) } }, 5",
    ));
    reject_termination(&two_edges.replace("remaining - (amount + 1)", "remaining"));
}

#[test]
fn nested_projection_relation_rejects_stalled_resets_and_foreign_rebuilds() {
    for rebuild in [
        "Countdown { label: countdown.label, inner: countdown.inner }, ceiling, amount",
        "Countdown { label: countdown.label, inner: Inner { remaining: countdown.inner.remaining } }, ceiling, amount",
        "Countdown { label: countdown.label - 1, inner: Inner { remaining: countdown.inner.remaining } }, ceiling, amount",
        "Countdown { label: countdown.label, inner: Inner { remaining: countdown.inner.remaining + amount } }, ceiling, amount",
        "countdown, ceiling, amount",
    ] {
        reject_termination(&NESTED.replace(NESTED_REBUILD, rebuild));
    }
    // Rebuilding the ranked field inside another declaration is not the
    // selected projection, even with the same spelling.
    reject_termination(&format!(
        "data Twin {{ remaining: u64 [0..=5]; }} {}",
        NESTED.replace("inner: Inner { remaining", "inner: Twin { remaining")
    ));
}

#[test]
fn nested_projection_relation_rejects_prefix_writes_through_the_projection() {
    let mutable = NESTED.replace(
        "walk(countdown: Countdown,",
        "walk(mut countdown: Countdown,",
    );
    prove_termination(&mutable);
    for statement in [
        "countdown.inner.remaining = 5;",
        "countdown.inner = Inner { remaining: 5 };",
        "countdown.label = 5;",
        "countdown = Countdown { label: 0, inner: Inner { remaining: 5 } };",
    ] {
        reject_range(&mutable.replace(
            "    transition",
            &format!("    {statement}\n    transition"),
        ));
    }
    let disjoint = mutable.replace(
        "    transition",
        "    let mut scratch: u64 = 0;\n    scratch = 5;\n    transition",
    );
    prove_termination(&disjoint);
}

#[test]
fn nested_endpoint_transports_through_a_forwarded_sibling_record() {
    reject_range(&NESTED_LIMIT.replace("bounds: countdown.bounds", "bounds: Bounds { limit: 5 }"));
    reject_range(&NESTED_LIMIT.replace(
        "bounds: countdown.bounds",
        "bounds: Bounds { limit: countdown.bounds.limit - 1 }",
    ));
    prove_termination(&NESTED_LIMIT.replace(
        "bounds: countdown.bounds",
        "bounds: Bounds { limit: countdown.bounds.limit }",
    ));
    reject_range(&NESTED_LIMIT.replace(
        "requires countdown.inner.remaining <= countdown.bounds.limit;",
        "",
    ));
    // A second record of the same type cannot supply the pinned endpoint.
    let other = NESTED_LIMIT
        .replace(
            "walk(countdown: Countdown)",
            "walk(countdown: Countdown, other: Countdown)",
        )
        .replace(
            "bounds: countdown.bounds\n        })",
            "bounds: other.bounds\n        }, other)",
        );
    reject_range(&other);
}

#[test]
fn borrowed_projection_relation_reads_the_referent_and_keeps_the_binding_unwritten() {
    reject_range(&BORROWED.replace("}, ceiling, amount)", "}, 5, amount)"));
    reject_range(&BORROWED.replace(BORROWED_PRECONDITION, ""));
    reject_range(&BORROWED.replace(BORROWED_PRECONDITION, "requires card.power >= ceiling;"));
    for rebuild in [
        "walk(card, ceiling, amount)",
        "walk(&Card { power: card.power }, ceiling, amount)",
        "walk(&Card { power: card.power + amount }, ceiling, amount)",
    ] {
        reject_termination(&BORROWED.replace(
            "walk(&Card { power: card.power - amount }, ceiling, amount)",
            rebuild,
        ));
    }
    let rebound = BORROWED
        .replace("walk(card: &Card,", "walk(mut card: &Card,")
        .replace("    transition", "    card = card;\n    transition");
    reject_range(&rebound);
    // Another borrowed record of the same type is not the ranked referent.
    let other = BORROWED
        .replace("amount: u64 [1..=2])", "amount: u64 [1..=2], other: &Card)")
        .replace("}, ceiling, amount)", "}, ceiling, amount, other)");
    prove_termination(&other);
    reject_range(&other.replace(BORROWED_PRECONDITION, "requires other.power <= ceiling;"));
    reject_termination(&other.replace("walk(&Card { power: card.power - amount }", "walk(other"));
}
