//! One byte of a borrowed byte view, `view[index]`. Read from source, as a
//! Build program: a Build that is handed a path looks at its first byte.
//!
//! byte_views.md, `ByteSequenceRead`: "`u64` index and length; dominating
//! length observation of the identical view; checked `index < length`". The
//! lowered form measures the view and reads the byte directly after, and the
//! read owes `index < length` against that measurement. So a read is accepted
//! exactly where the program has shown the index lies inside this view: a
//! `requires` clause, or a guard, over this view's own `len`.

use super::build_purpose::judge_build;
use super::*;
use terminal_psi::OperationKind;

/// `accepted`, `rejected` with what the program could not establish, or the
/// refusal's text.
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

#[test]
fn a_byte_is_read_where_the_view_is_known_to_reach_it() {
    // The caller owes the bound.
    assert_eq!(
        verdict(
            "machine first(path: &[u8]) -> u8 requires path.len > 0 { path[0] }
machine build_entry(path: &[u8]) { transition path.len > 0 { true -> use(first(path)) false -> none() } }
machine use(byte: u8) { }
machine none() { }"
        ),
        "accepted"
    );
    // A guard over the view's own length, with the read in the arm it
    // selects.
    assert_eq!(
        verdict(
            "machine build_entry(path: &[u8]) { transition path.len > 0 { true -> use(path[0]) false -> none() } }
machine use(byte: u8) { }
machine none() { }"
        ),
        "accepted"
    );
    // An index chosen at run time, and two reads of one view.
    assert_eq!(
        verdict(
            "machine at(path: &[u8], index: u64) -> u8 requires index < path.len { path[index] }
machine both(path: &[u8]) -> u8 requires path.len > 1 { path[0] ^ path[1] }
machine build_entry(path: &[u8]) { transition path.len > 1 { true -> use(at(path, 1), both(path)) false -> none() } }
machine use(one: u8, two: u8) { }
machine none() { }"
        ),
        "accepted"
    );
    // The same view under another name.
    assert_eq!(
        verdict(
            "machine build_entry(path: &[u8]) requires path.len > 0 { let view: &[u8] = path; use(view[0]); }
machine use(byte: u8) { }"
        ),
        "accepted"
    );
}

#[test]
fn a_byte_past_what_the_view_is_known_to_hold_is_rejected() {
    // Nothing says the view holds a byte.
    assert_eq!(
        verdict(
            "machine build_entry(path: &[u8]) { use(path[0]); }
machine use(byte: u8) { }"
        ),
        "rejected"
    );
    // The bound is one short.
    assert_eq!(
        verdict(
            "machine build_entry(path: &[u8]) requires path.len > 0 { use(path[1]); }
machine use(byte: u8) { }"
        ),
        "rejected"
    );
    // The bound is on another view: "An arbitrary equal-valued length,
    // other view ... supplies no length custody."
    assert_eq!(
        verdict(
            "machine build_entry(known: &[u8], path: &[u8]) requires known.len > 0 { use(path[0]); }
machine use(byte: u8) { }"
        ),
        "rejected"
    );
}

/// The lowered form of an accepted read is the measurement of the view and
/// then the read that names it, with nothing between.
#[test]
fn a_byte_read_names_the_measurement_made_directly_before_it() {
    let accepted = match judge_build(
        "machine build_entry(path: &[u8]) requires path.len > 0 { use(path[0]); }
machine use(byte: u8) { }",
    )
    .unwrap()
    {
        RouteVerdict::Accepted(accepted) => *accepted,
        other => panic!("{other:?}"),
    };
    let found = accepted
        .elaboration
        .module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| block.operations.windows(2))
        .filter_map(|pair| match (&pair[0].kind, &pair[1].kind) {
            (
                OperationKind::ByteSequenceLength { source: measured },
                OperationKind::ByteSequenceRead { source, length, .. },
            ) => Some(
                measured.place == *source
                    && measured.path.is_empty()
                    && pair[0].result.scalar_ref().map(|result| result.id) == Some(*length),
            ),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(found, [true]);
}

/// The forms this route does not lower yet are refused by name, with no
/// verdict: an index of another width than the read's `u64`, a byte of a
/// view that is itself stored in a field, and a store through a view.
#[test]
fn the_forms_not_lowered_are_refused_by_name() {
    assert_eq!(
        verdict(
            "machine build_entry(path: &[u8], index: u8) requires path.len > 255 { use(path[index]); }
machine use(byte: u8) { }"
        ),
        "typing byte view index that is not a u64"
    );
    assert_eq!(
        verdict(
            "data Holder<'held> { path: &'held [u8]; }
machine build_entry<'held>(holder: &Holder<'held>) requires holder.path.len > 0 { use(holder.path[0]); }
machine use(byte: u8) { }"
        ),
        "expression byte of a view stored in a field"
    );
    assert_eq!(
        verdict("machine build_entry(path: &mut [u8]) requires path.len > 0 { path[0] = 1; }"),
        "statement store into a value that is not a local"
    );
}
