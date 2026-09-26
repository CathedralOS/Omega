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

const COPIED_PAYLOAD: &str = "data Main {}
machine Main::count(&mut self, remaining: u32 [0..=9], extra: u32)
terminates by remaining in 0..=9;
-> u32 {
    transition remaining > 0 { true -> hold(remaining, extra, extra) false -> remaining }
    state hold(pending: u32 [0..=9], a: u32, mut b: u32) {
        transition a < b { true -> self.step(pending) false -> pending }
    }
}
machine Main::step(&mut self, n: u32 [0..=9])
terminates by n in 0..=9;
-> u32 {
    transition n > 0 { true -> self.count(n, 0) false -> n }
}";

#[test]
fn subordinate_site_store_into_a_slot_sharing_its_role_rejects() {
    // `a` and `b` are bare-forward copies of `extra`, so the site query holds
    // them equal and the `a < b` arm is dead: the never-decreasing cycle is
    // admitted only through that vacuity, which is sound while both copies
    // still denote the arrival value.
    assert_eq!(admitted(&typed_source(COPIED_PAYLOAD)).len(), 1);
    // A store into one copy makes the arm live at runtime (`b = 5` with
    // `a = 0` calls `step(pending)` forever); the copy is not a premise
    // carrier, but its equality is still an installed hypothesis.
    let written = COPIED_PAYLOAD.replace(
        "        transition a < b",
        "        b = 5;\n        transition a < b",
    );
    assert_ne!(written, COPIED_PAYLOAD);
    assert!(admitted(&typed_source(&written)).is_empty());
    // Three bare-forward copies of `extra` keep every claim, so a scratch
    // slot that is itself a copy shares the role and its store rejects...
    let scratch = COPIED_PAYLOAD
        .replace(
            "hold(remaining, extra, extra)",
            "hold(remaining, extra, extra, extra)",
        )
        .replace("a: u32, mut b: u32)", "a: u32, b: u32, mut scratch: u32)")
        .replace(
            "        transition a < b",
            "        scratch = 0;\n        transition a < b",
        );
    assert_ne!(scratch, COPIED_PAYLOAD);
    assert!(admitted(&typed_source(&scratch)).is_empty());
    // ...while a distinct input gives it a unique non-premise role and the
    // same store is admitted beside the untouched copies.
    let distinct = scratch
        .replace(
            "remaining: u32 [0..=9], extra: u32)",
            "remaining: u32 [0..=9], extra: u32, other: u32)",
        )
        .replace(
            "hold(remaining, extra, extra, extra)",
            "hold(remaining, extra, extra, other)",
        )
        .replace("self.count(n, 0)", "self.count(n, 0, 0)");
    assert_ne!(distinct, scratch);
    assert_eq!(admitted(&typed_source(&distinct)).len(), 1);
}

const MIXED: &str = "data Main {}
machine Main::outer(&mut self, cap: u64, remaining: u64)
requires remaining <= cap;
terminates by remaining in 0..=cap;
-> u64 {
    transition remaining > 0 { true -> self.inner(remaining, cap) false -> remaining }
}
machine Main::inner(&mut self, n: u64, mut limit: u64)
terminates by n;
-> u64 {
    transition n > 0 && n <= limit { true -> self.outer(limit, n - 1) false -> n }
}";

#[test]
fn mixed_range_component_rejects_stores_into_any_role_carrying_slot() {
    // `limit` is not one of `inner`'s premises (no range, no requires fact,
    // no constraint), but endpoint conservation reads its arrival atom to
    // pin `outer`'s ceiling through the unranged member.
    assert_eq!(admitted(&typed_source(MIXED)).len(), 1);
    let written = MIXED.replace(
        "    transition n > 0 && n <= limit",
        "    limit = 100;\n    transition n > 0 && n <= limit",
    );
    assert_ne!(written, MIXED);
    assert!(admitted(&typed_source(&written)).is_empty());
}
