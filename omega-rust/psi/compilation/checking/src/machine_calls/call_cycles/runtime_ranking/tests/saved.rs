//! A state-local immutable `let` nothing writes or exclusively borrows
//! denotes the exact value its initializer produced at binding time, so a
//! component edge may name the local where it means that value: the saved
//! binding substitutes the initializer polynomial and both sides prove the
//! same rank decrease and the same pinned endpoint. Anything that can leave
//! the name stale -- a second write, an exclusive borrow of a carrier, a
//! carrier the state writes -- binds nothing, and the strict engine fails
//! closed on the unknown atom.
use super::{admitted, typed_source};

/// `second` halves its work through two saved locals: `next` is the decreased
/// rank actual and `kept` the pinned endpoint bound. Both bindings are exact
/// copies of caller carriers, so the `second -> first` edge proves strict
/// descent on `remaining` and endpoint equality on `bound`.
const SAVED: &str = r#"
    machine Main::first(&mut self, remaining: u64 [0..=10], bound: u64 [0..=20])
    requires remaining <= bound;
    terminates by remaining in 0..=bound;
    -> u64 {
        transition remaining > 0 {
            true -> self.second(remaining, bound)
            false -> remaining
        }
    }
    machine Main::second(&mut self, remaining: u64 [1..=10], bound: u64 [1..=20])
    requires remaining <= bound;
    terminates by remaining in 0..=bound;
    -> u64 {
        let next: u64 = remaining - 1;
        let kept: u64 = bound;
        transition { _ -> self.first(next, kept) }
    }
"#;

#[test]
fn saved_locals_transport_rank_and_endpoint_through_call_actuals() {
    let program = typed_source(&format!("data Main {{}} {SAVED}"));
    assert_eq!(admitted(&program).len(), 1);
}

#[test]
fn a_written_local_binds_no_saved_value() {
    // `next` still spells the decreased value, but the later write means the
    // name denotes its CURRENT binding, not the entry observation -- the
    // strict engine cannot reach a binding for it and the edge fails.
    let written = SAVED.replace(
        "let next: u64 = remaining - 1;",
        "let mut next: u64 = remaining - 1; next = remaining - 1;",
    );
    assert_ne!(written, SAVED);
    assert!(admitted(&typed_source(&format!("data Main {{}} {written}"))).is_empty());
}

#[test]
fn a_local_reading_a_written_carrier_saves_nothing() {
    // `snap` reads `remaining` at binding time while the state writes the
    // carrier afterward; the observation would go stale against the entry
    // atoms, so no binding forms and the actual is unproven.
    let stale = SAVED.replace(
        "let kept: u64 = bound;",
        "let snap: u64 = remaining; remaining = 0; let kept: u64 = bound + snap - snap;",
    );
    assert_ne!(stale, SAVED);
    assert!(admitted(&typed_source(&format!("data Main {{}} {stale}"))).is_empty());
}

#[test]
fn an_exclusively_exposed_carrier_saves_nothing() {
    // Rooting `alias` at `remaining` exposes the carrier: any read a saved
    // local made of it could already be stale, so `next` binds nothing and
    // the decrease is unproven. The borrow itself is inert-looking enough to
    // reach the judgment.
    let exposed = SAVED
        .replace(
            "machine Main::second(&mut self, remaining: u64",
            "machine Main::second(&mut self, mut remaining: u64",
        )
        .replace(
            "let next: u64 = remaining - 1;",
            "let alias: &mut u64 = &mut remaining; let next: u64 = remaining - 1;",
        );
    assert_ne!(exposed, SAVED);
    assert!(admitted(&typed_source(&format!("data Main {{}} {exposed}"))).is_empty());
}

#[test]
fn a_saved_copy_of_the_rank_is_not_a_decrease() {
    // `next` binds exactly `remaining`: both edges preserve the rank, so the
    // complete cycle carries no strict decrease and the component rejects.
    let preserved = SAVED.replace(
        "let next: u64 = remaining - 1;",
        "let next: u64 = remaining;",
    );
    assert_ne!(preserved, SAVED);
    assert!(admitted(&typed_source(&format!("data Main {{}} {preserved}"))).is_empty());
}

#[test]
fn a_saved_value_cannot_move_the_endpoint() {
    // `kept` binds `bound - 1`, a real polynomial but a different endpoint:
    // the destination's `0..=bound` then names a smaller ceiling and the
    // pinning equality fails rather than approximating it.
    let shifted = SAVED.replace("let kept: u64 = bound;", "let kept: u64 = bound - 1;");
    assert_ne!(shifted, SAVED);
    assert!(admitted(&typed_source(&format!("data Main {{}} {shifted}"))).is_empty());
}

#[test]
fn a_local_outside_the_strict_language_binds_nothing() {
    // `wrap` keeps a stable binding but its record initializer leaves the
    // arithmetic language; naming it as the rank actual finds no polynomial.
    let structured = SAVED.replace(
        "let next: u64 = remaining - 1;",
        "let wrap: Wrap = Wrap { value: remaining - 1 }; let next: u64 = wrap.value;",
    );
    assert_ne!(structured, SAVED);
    assert!(
        admitted(&typed_source(&format!(
            "data Main {{}} data Wrap {{ value: u64; }} {structured}"
        )))
        .is_empty()
    );
}
