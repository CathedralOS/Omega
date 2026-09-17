//! Diverging copies of rank inputs at call-component boundaries: a slot that
//! claimed an entry role through a computed arrival denotes a derived value,
//! not the invocation rank. The telescope's dependency claim is consumed at a
//! call site only when every arrival into the slot preserves the bound the
//! member's order reads -- a bare forward of a carrier still holding the
//! role, a descent step in the direction the subject's order decreases, or a
//! ranged member's proved-equal copy anchored by a bare sibling. Anything
//! else leaves the role unconsumed rather than laundering an increase.

use super::{admitted, typed_source};

// `count(5) -> hold(6) -> step(5) -> count(5)` preserves the rank forever:
// `carried` claims `remaining` through `remaining + 1`, so `step(carried - 1)`
// would read a strict edge against a value the invocation never carried.
const INFLATED_UNRANGED: &str = "data Main {}
machine Main::count(&mut self, remaining: u32)
terminates by remaining;
-> u64 {
    transition { _ -> hold(remaining + 1) }
    state hold(carried: u32) {
        transition carried > 0 {
            true -> self.step(carried - 1)
            false -> 0
        }
    }
}
machine Main::step(&mut self, n: u32)
terminates by n;
-> u64 {
    transition n > 0 {
        true -> self.count(n)
        false -> 0
    }
}";

// The same divergence through a ranged member: `remaining < 9` keeps
// `remaining + 1` inside the declared range, and membership alone never
// proved `carried == remaining` -- the carrier's value exceeds the
// invocation rank the role names.
const INFLATED_RANGED: &str = "data Main {}
machine Main::count(&mut self, remaining: u32 [0..=9])
terminates by remaining in 0..=9;
-> u64 {
    transition remaining < 9 {
        true -> hold(remaining + 1)
        false -> 0
    }
    state hold(carried: u32 [0..=9]) {
        transition carried > 0 {
            true -> self.step(carried - 1)
            false -> 0
        }
    }
}
machine Main::step(&mut self, n: u32 [0..=9])
terminates by n in 0..=9;
-> u64 {
    transition n > 0 {
        true -> self.count(n)
        false -> 0
    }
}";

// Two slots claiming `remaining` through `remaining + 1` are proved equal to
// each other by the ranged arrival judgment, but never to the bare
// `remaining` carrier -- no sibling anchors the copy, so neither transports.
const UNANCHORED_COPIES: &str = "data Main {}
machine Main::count(&mut self, remaining: u32 [0..=9])
terminates by remaining in 0..=9;
-> u64 {
    transition remaining < 9 {
        true -> pair(remaining + 1, remaining + 1)
        false -> 0
    }
    state pair(left: u32 [0..=9], right: u32 [0..=9]) {
        transition left > 0 {
            true -> self.step(right - 1)
            false -> left
        }
    }
}
machine Main::step(&mut self, n: u32 [0..=9])
terminates by n in 0..=9;
-> u64 {
    transition n > 0 {
        true -> self.count(n)
        false -> 0
    }
}";

// A subordinate slot re-arrived through `pending - 1` keeps the role under a
// descending subject: the carrier stays bounded by the invocation rank, so
// `step(pending)` is a genuine weak edge and the strict `count(n - 1)` hop
// completes the cycle's descent.
const DESCENDED_CARRIER: &str = "data Main {}
machine Main::count(&mut self, remaining: u32 [0..=9])
terminates by remaining in 0..=9;
-> u64 {
    transition remaining > 0 {
        true -> hold(remaining)
        false -> remaining
    }
    state hold(pending: u32 [0..=9]) {
        transition pending > 1 {
            true -> hold(pending - 1)
            false -> self.step(pending)
        }
    }
}
machine Main::step(&mut self, n: u32 [0..=9])
terminates by n in 0..=9;
-> u64 {
    transition n > 0 {
        true -> self.count(n - 1)
        false -> n
    }
}";

// `right` arrives as the computed copy `remaining + 0`: the ranged arrival
// judgment proved it equal to `left`'s bare forward of the required
// `remaining` carrier, so the anchored copy keeps the role and transports.
const PROVED_COPY: &str = "data Main {}
machine Main::count(&mut self, remaining: u32 [0..=9])
terminates by remaining in 0..=9;
-> u64 {
    transition { _ -> pair(remaining, remaining + 0) }
    state pair(left: u32 [0..=9], right: u32 [0..=9]) {
        transition left > 0 {
            true -> self.step(right)
            false -> left
        }
    }
}
machine Main::step(&mut self, n: u32 [0..=9])
terminates by n in 0..=9;
-> u64 {
    transition n > 0 {
        true -> self.count(n - 1)
        false -> n
    }
}";

// `carried` arrives as the computed copy `remaining + 0`: on this unranged
// member no ranged arrival judgment proved the copy equal to a bare anchor,
// so the slot keeps no role and `step(carried)` cannot transport -- the same
// contract `rank_range_call_computed_copy_unproven` states for the corpus.
const ZERO_COPY: &str = "data Main {}
machine Main::count(&mut self, remaining: u32 [0..=9])
terminates by remaining;
-> u64 {
    transition { _ -> hold(remaining + 0) }
    state hold(carried: u32 [0..=9]) {
        transition carried > 0 {
            true -> self.step(carried)
            false -> carried
        }
    }
}
machine Main::step(&mut self, n: u32 [0..=9])
terminates by n in 0..=9;
-> u64 {
    transition n > 0 {
        true -> self.count(n - 1)
        false -> n
    }
}";

#[test]
fn an_inflated_unranged_carrier_does_not_transport_the_role() {
    assert!(admitted(&typed_source(INFLATED_UNRANGED)).is_empty());
}

#[test]
fn an_inflated_ranged_carrier_does_not_transport_the_role() {
    assert!(admitted(&typed_source(INFLATED_RANGED)).is_empty());
}

#[test]
fn unanchored_computed_copies_share_no_entry_role() {
    assert!(admitted(&typed_source(UNANCHORED_COPIES)).is_empty());
}

#[test]
fn a_descended_subject_carrier_keeps_its_role() {
    assert_eq!(admitted(&typed_source(DESCENDED_CARRIER)).len(), 1);
}

#[test]
fn a_proved_computed_copy_keeps_the_role_through_its_bare_anchor() {
    assert_eq!(admitted(&typed_source(PROVED_COPY)).len(), 1);
}

#[test]
fn an_unproved_computed_copy_keeps_no_role() {
    assert!(admitted(&typed_source(ZERO_COPY)).is_empty());
}
