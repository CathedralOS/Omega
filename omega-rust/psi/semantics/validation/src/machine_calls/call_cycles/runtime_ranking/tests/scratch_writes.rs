//! Prefix stores before a component call are judged against the caller's
//! premise carriers, located through each call-site state's telescope.

use super::{admitted, typed_source};

const SCRATCH: &str = "data Main {}
machine Main::scan_a(&mut self, index: u64 [0..=4], limit: u64 [0..=4], mut count: u64)
requires index <= limit;
terminates by (index, limit) -> Nat::BoundedDistance in 0..=4;
-> u64 {
    count = index;
    transition index < limit { true -> self.scan_b(index, limit, count) false -> count }
}
machine Main::scan_b(&mut self, index: u64 [0..=4], limit: u64 [0..=4], count: u64)
requires index <= limit;
terminates by (index, limit) -> Nat::BoundedDistance in 0..=4;
-> u64 {
    transition index < limit { true -> self.scan_a(index + 1, limit, count) false -> count }
}";

#[test]
fn entry_site_prefix_may_store_into_a_mutable_input_outside_the_premises() {
    assert_eq!(admitted(&typed_source(SCRATCH)).len(), 1);
    assert_eq!(
        admitted(&typed_source(
            &SCRATCH.replace("count = index;", "count = 3;")
        ))
        .len(),
        1
    );
}

#[test]
fn entry_site_prefix_store_into_a_premise_carrier_rejects() {
    let subject = SCRATCH
        .replace(
            "index: u64 [0..=4], limit: u64 [0..=4], mut count",
            "mut index: u64 [0..=4], limit: u64 [0..=4], mut count",
        )
        .replace("count = index;", "index = index;");
    assert!(admitted(&typed_source(&subject)).is_empty());
    let requires = SCRATCH.replace(
        "requires index <= limit;\nterminates by (index, limit) -> Nat::BoundedDistance in 0..=4;\n-> u64 {\n    count = index;",
        "requires index <= limit && count <= limit;\nterminates by (index, limit) -> Nat::BoundedDistance in 0..=4;\n-> u64 {\n    count = index;",
    );
    assert_ne!(requires, SCRATCH);
    assert!(admitted(&typed_source(&requires)).is_empty());
}

const SUBORDINATE: &str = "data Main {}
machine Main::count(&mut self, remaining: u32 [0..=9], mut seen: u32)
terminates by remaining in 0..=9;
-> u32 {
    transition remaining > 0 { true -> hold(remaining, seen) false -> remaining }
    state hold(pending: u32 [0..=9], mut scratch: u32) {
        scratch = pending;
        transition pending > 0 { true -> self.step(pending, scratch) false -> pending }
    }
}
machine Main::step(&mut self, n: u32 [0..=9], seen: u32)
terminates by n in 0..=9;
-> u32 {
    transition n > 0 { true -> self.count(n - 1, seen) false -> n }
}";

#[test]
fn subordinate_site_prefix_may_store_into_a_slot_carrying_no_premise_role() {
    assert_eq!(admitted(&typed_source(SUBORDINATE)).len(), 1);
}

#[test]
fn subordinate_site_prefix_store_into_a_slot_carrying_a_premise_role_rejects() {
    // `scratch` now carries the ranked entry `remaining`; storing into it
    // breaks the copy the site's telescope aliases onto the subject.
    let copied = SUBORDINATE.replace("hold(remaining, seen)", "hold(remaining, remaining)");
    assert!(admitted(&typed_source(&copied)).is_empty());
    let subject = SUBORDINATE
        .replace(
            "hold(pending: u32 [0..=9], mut scratch",
            "hold(mut pending: u32 [0..=9], mut scratch",
        )
        .replace("scratch = pending;", "pending = pending;");
    assert!(admitted(&typed_source(&subject)).is_empty());
}
