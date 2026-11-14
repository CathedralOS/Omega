//! An owned value moved out of storage the machine was lent with `&mut`.
//!
//! ownership.md, "Borrowed-storage invariant windows": "An owned move out of
//! a place reached through `&mut` may temporarily leave that place
//! unestablished ... Assignment of a valid owned replacement to the exact
//! vacant place closes the ownership window ... Every return or terminal
//! transfer from the region owing restoration ... must close the window."
//!
//! Each program lends a `Holder` whose `inner` field is not copyable and
//! moves that field out. The accepted ones store a replacement into the
//! same field before the machine returns; each failing twin differs in the
//! one store or use the rule is about.

use super::*;

const DATA: &str = "data Inner { value: u32; }\ndata Holder { inner: Inner; tag: u32; }\n";

fn verdict(body: &str) -> Result<Vec<String>, String> {
    let text = format!("{DATA}{body}");
    match super::entryless::judged(&text, false) {
        RouteVerdict::Rejected {
            violations,
            unestablished,
            unproved,
            conflicts,
            ..
        } => Ok(violations
            .iter()
            .map(|violation| violation.statement.clone())
            .chain(unestablished.iter().map(|used| used.detail.clone()))
            .chain(unproved.iter().map(|owed| format!("unproved: {owed:?}")))
            .chain(conflicts.iter().map(|loan| format!("conflict: {loan:?}")))
            .collect()),
        RouteVerdict::Accepted(_) => Err("accepted".into()),
        RouteVerdict::Incomplete(_) => Err("incomplete".into()),
        RouteVerdict::FallBack(refusal) => Err(format!("not supported: {}", refusal.detail)),
    }
}

fn accepted(body: &str) {
    assert_eq!(verdict(body).unwrap_err(), "accepted", "{body}");
}

fn rejected(body: &str, rule: &str) {
    let violations = verdict(body).unwrap_or_else(|other| panic!("{other}: {body}"));
    assert!(
        violations.iter().any(|violation| violation.contains(rule)),
        "{violations:?}"
    );
}

const NOT_RESTORED: &str =
    "the place does not hold a value again where the machine returns the borrow";
const USED_VACANT: &str = "owned storage is used after its value was transferred or consumed";

/// The window is opened by the move and closed by the store into the same
/// field; the twin never stores, so the caller would get its `Holder` back
/// with nothing in `inner`.
#[test]
fn a_moved_field_must_hold_a_value_again_when_the_machine_returns() {
    let program = |restore: &str| {
        format!(
            "machine swap(holder: &mut Holder, next: Inner) -> Inner {{\n    \
             let taken: Inner = holder.inner;\n    {restore}\n    taken\n}}\n"
        )
    };
    accepted(&program("holder.inner = next;"));
    rejected(&program("holder.tag = 1;"), NOT_RESTORED);
}

/// "Ancestors are unavailable for ... transfer": a transition hands the lent
/// `Holder` to another state, so the window must be closed before it. The
/// twin transfers the `Holder` with its field still vacant, meaning to
/// store in the state it enters.
#[test]
fn the_lent_value_is_not_handed_to_another_state_while_a_field_is_vacant() {
    let program = |before: &str, after: &str| {
        format!(
            "machine swap(holder: &mut Holder, next: Inner, last: Inner, go: bool) {{\n    \
             let taken: Inner = holder.inner;\n    {before}\n    \
             transition go {{\n        true -> finish(holder, last)\n        _ -> finish(holder, last)\n    }}\n    \
             machine finish(holder: &mut Holder, last: Inner) {{\n        {after}\n    }}\n}}\n"
        )
    };
    accepted(&program("holder.inner = next;", "holder.inner = last;"));
    rejected(
        &program("holder.tag = 2;", "holder.inner = last;"),
        USED_VACANT,
    );
}

/// "While the window is open, the absent subtree cannot be read ... or
/// moved again": the twin moves the field a second time before anything was
/// stored into it. A sibling field stays usable ("Disjoint established
/// siblings remain usable").
#[test]
fn a_vacant_field_is_not_read_before_it_is_restored() {
    let program = |between: &str| {
        format!(
            "machine swap(holder: &mut Holder, next: Inner) -> Inner {{\n    \
             let taken: Inner = holder.inner;\n    {between}\n    \
             holder.inner = next;\n    taken\n}}\n"
        )
    };
    accepted(&program("holder.tag = 3;"));
    rejected(&program("let again: Inner = holder.inner;"), USED_VACANT);
}

/// "Ancestors are unavailable for whole-value observation, calls, transfer":
/// the twin lends the whole `Holder` on while its field is vacant; the
/// accepted program lends it after the store.
#[test]
fn the_lent_value_is_not_passed_on_while_a_field_is_vacant() {
    let program = |first: &str, second: &str| {
        format!(
            "machine observe(holder: &Holder) {{ }}\n\
             machine swap(holder: &mut Holder, next: Inner) -> Inner {{\n    \
             let taken: Inner = holder.inner;\n    {first}\n    {second}\n    taken\n}}\n"
        )
    };
    accepted(&program("holder.inner = next;", "observe(holder);"));
    rejected(
        &program("observe(holder);", "holder.inner = next;"),
        USED_VACANT,
    );
}

/// A shared reference lends no window: the move stays outside what this
/// route reads, and is never accepted.
#[test]
fn an_owned_move_through_a_shared_reference_is_not_accepted() {
    let outcome = verdict(
        "machine take(holder: &Holder) -> Inner {\n    let taken: Inner = holder.inner;\n    taken\n}\n",
    );
    assert_ne!(outcome, Err("accepted".into()));
}
