//! A builtin scalar rank subject may read projected storage directly:
//! `bag.count` names the exact unsigned coordinate rooted at the `bag` record
//! formal, of whatever unsigned width the declaration gives the leaf. The
//! produced-rank arithmetic then judges membership, pinning, and strict
//! descent on that coordinate exactly as a bare formal -- a respelled copy,
//! a moved endpoint, a prefix write, or a signed leaf each fails on its own
//! terms.
use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

const MEMBER: &str = r#"
data Bag { count: u64 [0..=9]; }

machine walk(bag: Bag)
terminates by bag.count -> Nat::Descending in 0..=9;
-> u64 {
    transition { _ -> step(bag) }
    state step(current: Bag) {
        transition current.count > 0 {
            true -> step(Bag { count: current.count - 1 })
            false -> current.count
        }
    }
}
"#;

fn prove(source: &str) {
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .unwrap_or_else(|diagnostics| panic!("termination: {source}\n{diagnostics:#?}"));
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("complete checking: {source}\n{diagnostics:#?}"));
}

fn prove_termination(source: &str) {
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .unwrap_or_else(|diagnostics| panic!("termination: {source}\n{diagnostics:#?}"));
}

fn reject_range(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed_program(source))
        .expect_err("the authored member-subject range must be proved");
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
fn member_scalar_subject_proves_membership_and_descent_through_the_telescope() {
    prove(MEMBER);
    // A narrower declared leaf keeps the same produced coordinate: the
    // arithmetic domain, not the field width, decides naturality.
    let narrow = MEMBER.replace("u64", "u16");
    prove(&narrow);
    // A nested member chain resolves step by step against each declaration.
    let nested = "data Inner { count: u64 [0..=9]; }
data Outer { inner: Inner; }

machine walk(outer: Outer)
terminates by outer.inner.count -> Nat::Descending in 0..=9;
-> u64 {
    transition { _ -> step(outer) }
    state step(current: Outer) {
        transition current.inner.count > 0 {
            true -> step(Outer { inner: Inner { count: current.inner.count - 1 } })
            false -> current.inner.count
        }
    }
}
";
    prove(nested);
}

#[test]
fn member_scalar_subject_rejects_stalled_foreign_and_respelled_arrivals() {
    for actual in [
        // The unchanged record, the unchanged field, and a deeper cut all
        // fail strict descent on the exact coordinate.
        "current",
        "Bag { count: current.count }",
        "Bag { count: current.count - 2 }",
    ] {
        reject_termination(&MEMBER.replace("Bag { count: current.count - 1 }", actual));
    }
    // A sibling record of the same declaration is a foreign coordinate: its
    // `count` never equals the ranked slot's by ancestry or spelling.
    let sibling = MEMBER
        .replace(
            "machine walk(bag: Bag)",
            "machine walk(bag: Bag, other: Bag)",
        )
        .replace("_ -> step(bag)", "_ -> step(bag, other)")
        .replace(
            "state step(current: Bag) {",
            "state step(current: Bag, spare: Bag) {",
        )
        .replace(
            "step(Bag { count: current.count - 1 })",
            "step(Bag { count: current.count - 1 }, spare)",
        );
    prove(&sibling);
    for actual in [
        "step(Bag { count: spare.count - 1 }, spare)",
        "step(Bag { count: spare.count }, spare)",
        "step(current, Bag { count: spare.count - 1 })",
    ] {
        reject_range(&sibling.replace("step(Bag { count: current.count - 1 }, spare)", actual));
    }
}

#[test]
fn member_scalar_subject_keeps_endpoint_pinning_and_membership_obligations() {
    reject_range(&MEMBER.replace("in 0..=9", "in 1..=9"));
    reject_range(&MEMBER.replace("in 0..=9", "in 0..8"));
    // A `.len`-style respelling or the current state's member copy is not the
    // pinned entry endpoint either; the authored ceiling is `9`, not a
    // spelling of the moved coordinate.
    reject_range(&MEMBER.replace("in 0..=9", "in 0..=current.count"));
    // The entry membership needs the declared field bound: without it the
    // produced coordinate cannot be placed inside the authored range.
    let unbounded = MEMBER.replace("u64 [0..=9]", "u64");
    reject_range(&unbounded);
    // A signed leaf has no natural rank coordinate at all.
    reject_range(&MEMBER.replace("u64 [0..=9]", "i64"));
}

#[test]
fn member_scalar_subject_rejects_prefix_writes_on_the_carrier_path() {
    let mutable = MEMBER.replace("state step(current: Bag", "state step(mut current: Bag");
    // The constructor range check does not propagate a mutable record's
    // declared leaf bound; the termination obligation itself still stands.
    prove_termination(&mutable);
    for statement in ["current.count = 5;", "current = Bag { count: 5 };"] {
        reject_termination(&mutable.replace(
            "        transition current.count",
            &format!("        {statement}\n        transition current.count"),
        ));
    }
}

const INCREASING: &str = r#"
data Bag { cursor: u64; }

machine walk(bag: Bag, limit: u64 [0..=9])
requires bag.cursor <= limit;
terminates by bag.cursor -> Nat::IncreasingTo(limit) in 0..=limit;
-> u64 {
    transition { _ -> step(bag, limit) }
    state step(current: Bag, bound: u64 [0..=9]) {
        transition current.cursor < bound {
            true -> step(Bag { cursor: current.cursor + 1 }, bound)
            false -> current.cursor
        }
    }
}
"#;

#[test]
fn member_increasing_subject_proves_the_clamped_ascent() {
    prove(INCREASING);
    reject_termination(&INCREASING.replace(
        "Bag { cursor: current.cursor + 1 }",
        "Bag { cursor: current.cursor }",
    ));
    reject_termination(&INCREASING.replace(
        "Bag { cursor: current.cursor + 1 }",
        "Bag { cursor: current.cursor + 2 }",
    ));
    // The view bound is pinned: `bound + 1` moves the shared limit.
    reject_range(&INCREASING.replace(
        "step(Bag { cursor: current.cursor + 1 }, bound)",
        "step(Bag { cursor: current.cursor + 1 }, bound + 1)",
    ));
    reject_range(&INCREASING.replace("requires bag.cursor <= limit;", ""));
}

const DISTANCE: &str = r#"
data Bag { low: u64 [0..=9]; high: u64 [0..=9]; }

machine walk(bag: Bag)
requires bag.low <= bag.high;
terminates by (bag.low, bag.high) -> Nat::BoundedDistance in 0..=9;
-> u64 {
    transition { _ -> step(bag) }
    state step(current: Bag) {
        transition current.high > 0 && current.low < current.high {
            true -> step(Bag { low: current.low, high: current.high - 1 })
            false -> current.high
        }
    }
}
"#;

#[test]
fn member_distance_subjects_produce_the_paired_coordinate_difference() {
    prove(DISTANCE);
    // Holding the upper subject still leaves the distance unmoved.
    reject_termination(&DISTANCE.replace("high: current.high - 1", "high: current.high"));
    // Two spellings of one coordinate produce a constant zero distance: no
    // strict descent exists to prove.
    reject_termination(&DISTANCE.replace(
        "terminates by (bag.low, bag.high)",
        "terminates by (bag.low, bag.low)",
    ));
}
