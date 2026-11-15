//! The length of a fixed array, `array.len`. Read from source, as a Build
//! program: a Build that holds a table asks how many rows it has.
//!
//! domains.md: "`[u8; N] in D` is the same exact `N`-byte fixed-array carrier
//! as `[u8; N]`; its `.len` is `N`". The length is the extent the type
//! states, so the reader gives the expression that number and lowers no
//! operation for it. A guarantee over the result is then held to the number,
//! whichever fields the array is reached through.

use super::build_purpose::judge_build;
use super::*;

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
fn the_length_of_a_fixed_array_is_the_extent_its_type_states() {
    // A field of data lent to the machine, and a field of a field.
    assert_eq!(
        verdict(
            "data Holder { slots: [u8; 4]; }
data Outer { holder: Holder; }
machine held(holder: &Holder) -> u64 ensures result == 4 { holder.slots.len }
machine nested(outer: &Outer) -> u64 ensures result == 4 { outer.holder.slots.len }
machine build_entry() { let outer: Outer; let seen: u64 = held(&outer.holder) + nested(&outer); }"
        ),
        "accepted"
    );
    // A local array.
    assert_eq!(
        verdict(
            "machine build_entry() -> u64 ensures result == 3 { let values: [u32; 3] = [1, 2, 3]; values.len }"
        ),
        "accepted"
    );
}

#[test]
fn a_guarantee_of_another_length_is_rejected() {
    // The same machine as above, promising one row more than the type has.
    let rejected = verdict(
        "data Holder { slots: [u8; 4]; }
machine held(holder: &Holder) -> u64 ensures result == 5 { holder.slots.len }
machine build_entry() { let holder: Holder; let seen: u64 = held(&holder); }",
    );
    assert!(rejected.starts_with("rejected"), "{rejected}");
}

#[test]
fn the_length_of_an_array_that_must_be_evaluated_first_is_refused() {
    // A call result is no place: the call would have to run for its length
    // to be read, and nothing is lowered for a length.
    assert_eq!(
        verdict(
            "machine make() -> [u8; 4] { [0, 0, 0, 0] }
machine build_entry() -> u64 { make().len }"
        ),
        "expression length of a fixed array that is not a field path while reading the body of \
         `build_entry`"
    );
}
