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

const PROJECTED_SLICE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/rank_range_projected_slice_length/main.omg"
));
const NESTED_SLICE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/rank_range_nested_slice_length/main.omg"
));
const PROJECTED_SCALAR: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/computed_measure_projected_subject/main.omg"
));
const SLICE_PRECONDITION: &str = "requires bag.items.len <= capacity;";
const SCALAR_PRECONDITION: &str = "requires countdown.remaining * 2 <= limit;";
/// The same `step` telescope with a second record carrier, so the ranked
//  coordinate's role competes with a sibling slot for arrivals.
const SLICE_SIBLING: &str = r#"
data Bag { items: &[u32]; }

machine walk(bag: Bag, other: Bag, capacity: u64)
requires bag.items.len <= capacity && other.items.len <= capacity;
terminates by bag.items -> Slice::Length in 0..=capacity;
-> u64 {
    transition { _ -> step(bag, other, capacity) }
    state step(current: Bag, spare: Bag, bound: u64) {
        transition current.items.len > 0 && spare.items.len > 0 {
            true -> step(Bag { items: current.items[1..] }, spare, bound)
            false -> 0
        }
    }
}
"#;
/// A declared scalar view over a nested member chain.
const NESTED_SCALAR: &str = r#"
data Inner { remaining: u64 [0..=5]; }
data Outer { inner: Inner; }
measure Outer::Doubled(value: u64) -> u64 { value * 2 }

machine walk(outer: Outer)
terminates by outer.inner.remaining -> Outer::Doubled in 0..=10;
-> u64 {
    transition outer.inner.remaining > 0 {
        true -> walk(Outer { inner: Inner { remaining: outer.inner.remaining - 1 } })
        false -> 0
    }
}
"#;
/// A declared computation view over a bare formal: the produced rank is
/// `value * 2`, but the endpoint is still an ordinary projected field the
/// edge judgment must bind and pin on every arrival.
const COMPUTED_BARE: &str = r#"
data Limits { cap: u64 [0..=10]; }
data Countdown {}
measure Countdown::Doubled(value: u64) -> u64 { value * 2 }

machine walk(remaining: u64 [0..=5], limits: Limits)
requires remaining * 2 <= limits.cap;
terminates by remaining -> Countdown::Doubled in 0..=limits.cap;
-> u64 {
    transition remaining > 0 {
        true -> walk(remaining - 1, limits)
        false -> remaining
    }
}
"#;

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

#[test]
fn projected_slice_length_relation_checks_through_complete_lowering() {
    for source in [
        PROJECTED_SLICE,
        NESTED_SLICE,
        PROJECTED_SCALAR,
        SLICE_SIBLING,
    ] {
        crate::checks::termination::check_machine_termination(&typed(source))
            .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
        lower_typed_trees(typed(source))
            .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
    }
}

#[test]
fn computed_bare_subject_still_binds_and_pins_projected_endpoints() {
    prove_termination(COMPUTED_BARE);
    // A rebuilt carrier that moves the endpoint is not a conserved limit.
    reject_range(&COMPUTED_BARE.replace(
        "walk(remaining - 1, limits)",
        "walk(remaining - 1, Limits { cap: limits.cap + 1 })",
    ));
    // The exact literal rebuild preserves the endpoint through the same
    // checked correspondence a plain forward uses.
    prove_termination(&COMPUTED_BARE.replace(
        "walk(remaining - 1, limits)",
        "walk(remaining - 1, Limits { cap: limits.cap })",
    ));
    // The endpoint still owes entry membership against its own premise.
    reject_range(&COMPUTED_BARE.replace("requires remaining * 2 <= limits.cap;", ""));
    reject_range(&COMPUTED_BARE.replace("in 0..=limits.cap", "in 1..=limits.cap"));
}

#[test]
fn projected_slice_length_requires_entry_evidence_on_the_exact_chain() {
    for requirement in ["", "requires capacity >= 0;", "requires bound <= capacity;"] {
        reject_range(&PROJECTED_SLICE.replace(SLICE_PRECONDITION, requirement));
    }
    // The produced length is natural only through the exact leaf spelling:
    // the enclosing record's other members and the `.len` subject of a
    // foreign chain never bound it.
    let labeled = PROJECTED_SLICE
        .replace(
            "data Bag { items: &[u32]; }",
            "data Bag { label: u64; items: &[u32]; }",
        )
        .replace(
            "Bag { items: current.items[1..] }",
            "Bag { label: current.label, items: current.items[1..] }",
        );
    prove_termination(&labeled);
    reject_range(&labeled.replace(SLICE_PRECONDITION, "requires bag.label <= capacity;"));
}

#[test]
fn projected_slice_length_pins_endpoints_and_proves_subslice_descent() {
    // A respelled produced length is not the pinned endpoint copy.
    reject_range(&PROJECTED_SLICE.replace("}, bound)", "}, current.items.len)"));
    // A `.len` spelling is collection metadata, not a formed endpoint.
    reject_range(&PROJECTED_SLICE.replace("in 0..=capacity", "in 0..=bag.items.len"));
    // The unchanged collection, a deeper cut than the guard proves, and a
    // cyclic plateau all fail the produced-rank arithmetic.
    for actual in [
        "Bag { items: current.items }",
        "Bag { items: current.items[0..] }",
        "Bag { items: current.items[2..] }",
    ] {
        reject_termination(&PROJECTED_SLICE.replace("Bag { items: current.items[1..] }", actual));
    }
    reject_range(&PROJECTED_SLICE.replace("false -> 0", "false -> step(current, bound)"));
}

#[test]
fn projected_slice_length_rejects_prefix_writes_on_the_carrier_path() {
    let mutable = PROJECTED_SLICE.replace("state step(current: Bag", "state step(mut current: Bag");
    prove_termination(&mutable);
    for statement in ["current.items = current.items;", "current = current;"] {
        reject_termination(&mutable.replace(
            "        transition current.items.len",
            &format!("        {statement}\n        transition current.items.len"),
        ));
    }
    // A scratch local write outside every premise path still preserves.
    let disjoint = mutable.replace(
        "        transition current.items.len",
        "        let mut scratch: u64 = 0;\n        scratch = 5;\n        transition current.items.len",
    );
    prove_termination(&disjoint);
}

#[test]
fn projected_slice_length_keeps_every_role_coordinate_exact() {
    // The sibling slot forwards its own role: the ranked carrier still
    // descends while `spare` keeps `other`'s length coordinate.
    prove_termination(SLICE_SIBLING);
    // Rebuilding the ranked slot from the sibling's collection is a foreign
    // coordinate: `len(spare.items)` never equals `len(current.items)` by
    // ancestry or spelling.
    for actual in [
        "Bag { items: spare.items[1..] }",
        "Bag { items: spare.items }",
    ] {
        reject_range(&SLICE_SIBLING.replace("Bag { items: current.items[1..] }", actual));
    }
    // The shortened slice arriving in the payload slot does not descend the
    // ranked carrier, which forwards unchanged.
    reject_range(&SLICE_SIBLING.replace(
        "step(Bag { items: current.items[1..] }, spare, bound)",
        "step(current, Bag { items: spare.items[1..] }, bound)",
    ));
}

#[test]
fn nested_slice_length_relation_rebuilds_both_declarations() {
    // Forwarding the whole `inner` step, respelling the leaf unchanged, or
    // cutting deeper than the guard proves all fail descent or formability.
    for actual in [
        "Outer { inner: current.inner }",
        "Outer { inner: Inner { items: current.inner.items } }",
        "Outer { inner: Inner { items: current.inner.items[2..] } }",
    ] {
        reject_termination(&NESTED_SLICE.replace(
            "Outer { inner: Inner { items: current.inner.items[1..] } }",
            actual,
        ));
    }
    reject_range(&NESTED_SLICE.replace("requires outer.inner.items.len <= capacity;", ""));
    reject_range(&NESTED_SLICE.replace("}, bound)", "}, current.inner.items.len)"));
}

#[test]
fn projected_scalar_view_relation_uses_the_member_coordinate() {
    // The view body reads the exact `u64` leaf: stalling the field, respelling
    // the ceiling, and dropping the produced-rank fact each fail exactly.
    reject_range(&PROJECTED_SCALAR.replace("current.remaining - 1", "current.remaining"));
    reject_range(&PROJECTED_SCALAR.replace("}, bound)", "}, current.remaining)"));
    reject_range(&PROJECTED_SCALAR.replace(SCALAR_PRECONDITION, ""));
    // A ceiling the declared field bound cannot reach never holds.
    reject_range(&PROJECTED_SCALAR.replace("in 0..=limit", "in 0..=9"));
    prove_termination(NESTED_SCALAR);
    reject_range(&NESTED_SCALAR.replace("outer.inner.remaining - 1", "outer.inner.remaining"));
}
