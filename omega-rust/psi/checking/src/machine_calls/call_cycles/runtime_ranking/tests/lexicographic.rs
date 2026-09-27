//! Lexicographic members issue component calls from subordinate states: the
//! spelled carrier formal telescopes to its discovered entry role, so the
//! rebuilt literal still reads the carried rank's components. A carrier that
//! arrived through a computed record keeps no role -- the site cannot read a
//! subject equality no judgment established.

use super::{admitted, typed_source};

// `carried` arrives through a computed literal that refills the component
// `scan_b` decrements, so the cycle `scan_a -> hold -> scan_b -> scan_a`
// preserves the rank forever. Lineage to `progress` is not equality:
// `carried` keeps no role and the site cannot spell the authored subject.
const STATE_CALL_INFLATED: &str = "data Progress { outer: u64; inner: u64; }
measure Progress::Steps lexicographic { outer, inner }
data Main {}
machine Main::scan_a(&mut self, progress: Progress) -> u64
terminates by progress -> Progress::Steps;
{
    transition progress.inner > 0 {
        true -> hold(Progress { outer: progress.outer, inner: progress.inner + 1 })
        false -> 0
    }
    state hold(carried: Progress) {
        transition carried.inner > 0 {
            true -> self.scan_b(carried)
            false -> 0
        }
    }
}
machine Main::scan_b(&mut self, remaining: Progress) -> u64
terminates by remaining -> Progress::Steps;
{
    transition remaining.inner > 0 {
        true -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - 1 })
        false -> 0
    }
}";

// A constant tree that folds to zero is still no decrement: the folded
// amount fails the strict-step check exactly like a literal `0`.
const STATE_CALL_ZERO_STEP: &str = "data Progress { outer: u64; inner: u64; }
measure Progress::Steps lexicographic { outer, inner }
data Main {}
machine Main::scan_a(&mut self, progress: Progress) -> u64
terminates by progress -> Progress::Steps;
{
    transition progress.inner > 1 {
        true -> self.scan_b(progress)
        false -> 0
    }
}
machine Main::scan_b(&mut self, remaining: Progress) -> u64
terminates by remaining -> Progress::Steps;
{
    transition remaining.inner > 1 {
        true -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - (1 - 1) })
        false -> 0
    }
}";

#[test]
fn a_computed_record_claimant_does_not_carry_the_subject() {
    let program = typed_source(STATE_CALL_INFLATED);
    assert!(admitted(&program).is_empty());
}

#[test]
fn a_constant_tree_folding_to_zero_is_no_decrement() {
    let program = typed_source(STATE_CALL_ZERO_STEP);
    assert!(admitted(&program).is_empty());
}
