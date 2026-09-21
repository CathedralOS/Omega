//! Lexicographic members issue component calls from subordinate states: the
//! spelled carrier formal telescopes to its discovered entry role, so the
//! rebuilt literal still reads the carried rank's components. A carrier that
//! arrived through a computed record keeps no role -- the site cannot read a
//! subject equality no judgment established.

use super::{admitted, typed_source};

// `hold -> scan_b` transports `carried` to its discovered entry role
// `progress`, so the rebuilt literal still reads the carried rank's
// components and the guarded decrement is the strict edge.
const STATE_CALL: &str = "data Progress { outer: u64; inner: u64; }
measure Progress::Steps lexicographic { outer, inner }
data Main {}
machine Main::scan_a(&mut self, progress: Progress) -> u64
terminates by progress -> Progress::Steps;
{
    transition progress.inner > 1 {
        true -> hold(progress)
        false -> 0
    }
    state hold(carried: Progress) {
        transition carried.inner > 0 {
            true -> self.scan_b(Progress { outer: carried.outer, inner: carried.inner - 1 })
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

// The step amount is the closed integer its spelling evaluates to: a
// constant arithmetic tree carries the same exact identity as the literal it
// folds to, so `inner - (1 + 1)` is the guarded decrement the cycle needs.
const STATE_CALL_CONSTANT_STEP: &str = "data Progress { outer: u64; inner: u64; }
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
        true -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - (1 + 1) })
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

// Folding the step does not relax the lower-bound proof: a guard that only
// bounds the component by 1 cannot carry a step of 2, constant or literal.
const STATE_CALL_UNDERRANGED_STEP: &str = "data Progress { outer: u64; inner: u64; }
measure Progress::Steps lexicographic { outer, inner }
data Main {}
machine Main::scan_a(&mut self, progress: Progress) -> u64
terminates by progress -> Progress::Steps;
{
    transition progress.inner > 0 {
        true -> self.scan_b(progress)
        false -> 0
    }
}
machine Main::scan_b(&mut self, remaining: Progress) -> u64
terminates by remaining -> Progress::Steps;
{
    transition remaining.inner > 0 {
        true -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - (0 + 2) })
        false -> 0
    }
}";

#[test]
fn a_lexicographic_member_calls_from_a_subordinate_state() {
    let program = typed_source(STATE_CALL);
    assert_eq!(admitted(&program).len(), 1);
}

#[test]
fn a_computed_record_claimant_does_not_carry_the_subject() {
    let program = typed_source(STATE_CALL_INFLATED);
    assert!(admitted(&program).is_empty());
}

#[test]
fn a_component_may_step_by_a_closed_arithmetic_tree() {
    let program = typed_source(STATE_CALL_CONSTANT_STEP);
    assert_eq!(admitted(&program).len(), 1);
}

#[test]
fn a_constant_tree_folding_to_zero_is_no_decrement() {
    let program = typed_source(STATE_CALL_ZERO_STEP);
    assert!(admitted(&program).is_empty());
}

#[test]
fn a_folded_step_still_needs_its_lower_bound() {
    let program = typed_source(STATE_CALL_UNDERRANGED_STEP);
    assert!(admitted(&program).is_empty());
}
