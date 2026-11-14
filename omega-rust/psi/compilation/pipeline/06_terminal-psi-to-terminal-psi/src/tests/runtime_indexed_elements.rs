//! An element chosen at run time, inside a record the machine was lent. Read
//! from source, as a Build program: the toolchain's `Optimizations::enable`
//! counts a selection in `optimizations.selected[slot]`, where `slot` comes
//! from the optimization it was given.
//!
//! Ownership follows exact regions (ownership.md: "Affine | Default owned
//! data | Move at most once"). Which element such a projection names is not
//! known where ownership is decided, so the element has no region of its
//! own: observing it depends on the whole array it indexes. The array is
//! therefore required whole. That never admits an element that is gone, and
//! the index itself is still held to the array's extent by its own proof.

use super::build_purpose::judge_build;
use super::*;

const DATA: &str = r#"
data Kind { case First; case Second; }
data Table { reports: u8 in Saturating; selected: [u8 in Saturating; 4]; }
machine keep(table: Table) { }
"#;

/// `accepted`, `rejected` with what the program could not establish, or
/// the refusal's text.
fn verdict(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        Ok(RouteVerdict::Rejected { unestablished, .. }) => match &unestablished[..] {
            [first, ..] => format!("rejected: {}", first.detail),
            [] => "rejected".to_owned(),
        },
        other => panic!("{other:?}"),
    }
}

fn counting(last: u64, entry: &str) -> String {
    format!(
        r#"{DATA}
machine slot_of(kind: Kind) -> u64 ensures result <= {last}u64 {{
    match kind {{ Kind::First -> 1u64 Kind::Second -> {last}u64 }}
}}
machine count(table: &mut Table, kind: Kind) {{
    let slot: u64 = slot_of(kind);
    table.selected[slot] = table.selected[slot] + 1;
}}
machine read(table: &Table, kind: Kind) -> u8 in Saturating {{
    let slot: u64 = slot_of(kind);
    table.selected[slot]
}}
{entry}
"#
    )
}

/// The toolchain's shape: read the element the slot selects, add one, and
/// store it back, all through the record the machine was lent.
#[test]
fn an_element_chosen_at_run_time_is_read_and_written_through_a_lent_record() {
    assert_eq!(
        verdict(&counting(
            3,
            "machine build_entry(kind: Kind) { let mut table: Table; count(&mut table, kind); }"
        )),
        "accepted"
    );
    assert_eq!(
        verdict(&counting(
            3,
            "machine build_entry(kind: Kind) {
    let table: Table;
    let seen: u8 in Saturating = read(&table, kind);
}"
        )),
        "accepted"
    );
}

/// The twin differs only in the last slot: 4 is one past the array, so the
/// element's own proof fails.
#[test]
fn a_slot_that_may_lie_outside_the_array_is_rejected() {
    let verdict = verdict(&counting(
        4,
        "machine build_entry(kind: Kind) { let mut table: Table; count(&mut table, kind); }",
    ));
    assert!(verdict.starts_with("rejected"), "{verdict}");
}

/// The array is required whole: once the record that holds it has been
/// moved away, no element of it can be read, whichever one the slot names.
#[test]
fn an_element_of_a_moved_record_is_not_available() {
    let verdict = verdict(&format!(
        r#"{DATA}
machine slot_of(kind: Kind) -> u64 ensures result < 4u64 {{
    match kind {{ Kind::First -> 1u64 Kind::Second -> 3u64 }}
}}
machine build_entry(kind: Kind) {{
    let table: Table;
    keep(table);
    let slot: u64 = slot_of(kind);
    let seen: u8 in Saturating = table.selected[slot];
}}
"#
    ));
    assert_eq!(
        verdict,
        "rejected: owned storage is used after its value was transferred or consumed"
    );
}
