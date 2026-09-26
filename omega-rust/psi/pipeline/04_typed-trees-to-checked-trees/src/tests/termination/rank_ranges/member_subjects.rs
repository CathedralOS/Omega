//! A builtin scalar rank subject may read projected storage directly:
//! `bag.count` names the exact unsigned coordinate rooted at the `bag` record
//! formal, of whatever unsigned width the declaration gives the leaf. The
//! produced-rank arithmetic then judges membership, pinning, and strict
//! descent on that coordinate exactly as a bare formal -- a respelled copy,
//! a moved endpoint, a prefix write, or a signed leaf each fails on its own
//! terms.
use crate::tests::front_end::typed_program;

const MEMBER: &str = r#"
data Bag { count: u64 [0..=9]; }

machine walk(bag: Bag)
terminates by bag.count -> Nat::Descending in 0..=9;
-> u64 {
    transition { _ -> step(bag) }
    state step(current: Bag) {
        transition current.count > 0 {
            true -> step(Bag { count: current.count - 1 })
            false -> current.count
        }
    }
}
"#;

fn reject_range(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed_program(source))
        .expect_err("the authored member-subject range must be proved");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
    );
}

#[test]
fn member_scalar_subject_keeps_endpoint_pinning_and_membership_obligations() {
    reject_range(&MEMBER.replace("in 0..=9", "in 1..=9"));
    reject_range(&MEMBER.replace("in 0..=9", "in 0..8"));
    // A `.len`-style respelling or the current state's member copy is not the
    // pinned entry endpoint either; the authored ceiling is `9`, not a
    // spelling of the moved coordinate.
    reject_range(&MEMBER.replace("in 0..=9", "in 0..=current.count"));
    // The entry membership needs the declared field bound: without it the
    // produced coordinate cannot be placed inside the authored range.
    let unbounded = MEMBER.replace("u64 [0..=9]", "u64");
    reject_range(&unbounded);
    // A signed leaf has no natural rank coordinate at all.
    reject_range(&MEMBER.replace("u64 [0..=9]", "i64"));
}
