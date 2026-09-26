//! The relational range route transports the ranked field, the endpoints and
//! the entry facts through a nested projection path or a borrowed subject,
//! proving membership, pinning and descent exactly as the direct owned-field
//! route does.
use crate::tests::front_end::typed_program;

const PROJECTED_SLICE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/rank_range_projected_slice_length/main.omg"
));
const NESTED_SLICE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/rank_range_nested_slice_length/main.omg"
));
const SLICE_PRECONDITION: &str = "requires bag.items.len <= capacity;";
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
fn prove_termination(source: &str) {
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn reject_range(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed_program(source))
        .expect_err("the authored projected rank range must be proved");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
    );
}

fn reject_termination(source: &str) {
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .expect_err(source);
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
