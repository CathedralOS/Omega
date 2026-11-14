//! Return-field promises bind the exact returned storage, then remain facts
//! about that value after the call. A later write changes only its own field.

use super::build_purpose::judge_build;
use super::*;

const SOURCE: &str = r#"
data Pair [copy] { first: u32; second: u32; }
machine pair(input: u32) -> Pair
ensures result.first == input, result.second == 7
{ Pair { first: input, second: 7 } }
machine seven(value: u32) requires value == 7 { }
machine build_entry() {
    let mut value: Pair = pair(3);
    value.first = 9;
    seven(value.second);
}
"#;

#[test]
fn returned_field_guarantees_follow_the_call_and_preserve_disjoint_writes() {
    let result = judge_build(SOURCE).unwrap();
    let RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    accepted.publish_invocation().unwrap();
}

#[test]
fn a_written_result_field_does_not_keep_the_calls_old_guarantee() {
    let changed = SOURCE.replace("value.first = 9", "value.second = 9");
    let result = judge_build(&changed).unwrap();
    let RouteVerdict::Rejected { unproved, .. } = result else {
        panic!("{result:?}")
    };
    assert!(
        unproved
            .iter()
            .any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))
    );
}

#[test]
fn the_callee_must_prove_the_fields_it_actually_returns() {
    let changed = SOURCE.replace("second: 7 }", "second: 8 }");
    let result = judge_build(&changed).unwrap();
    let RouteVerdict::Rejected { unproved, .. } = result else {
        panic!("{result:?}")
    };
    assert!(
        unproved
            .iter()
            .any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))
    );
}

const TWO_CALLS: &str = r#"
data Pair [copy] { first: u32; second: u32; }
machine pair(input: u32) -> Pair
ensures result.first == input, result.second == 7
{ Pair { first: input, second: 7 } }
machine three(value: u32) requires value == 3 { }
machine build_entry() {
    let original: Pair = pair(3);
    let other: Pair = pair(5);
    three(original.first);
}
"#;

#[test]
fn a_result_field_promise_belongs_to_its_actual_call() {
    let result = judge_build(TWO_CALLS).unwrap();
    let RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    accepted.publish_invocation().unwrap();

    let other_call = TWO_CALLS.replace("three(original.first)", "three(other.first)");
    let result = judge_build(&other_call).unwrap();
    let RouteVerdict::Rejected { unproved, .. } = result else {
        panic!("{result:?}")
    };
    assert!(
        unproved
            .iter()
            .any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))
    );
}

#[test]
fn borrowed_completion_observations_cannot_reuse_input_snapshots() {
    use super::build_purpose::{frontier_of, package, roster, symbol};

    // The authored promise is true of the actual result. A receiving producer
    // must not replace it with an entry fact about a borrowed input: the Pair
    // is changed before return. The borrowed view is unchanged, but its
    // completion observation is still unsupported, not an entry snapshot.
    const SOURCE: &str = r#"
data Pair [copy] { first: u32; second: u32; }
machine build_entry(INPUT, bytes: &[u8]) -> Pair
requires p.first == 0, bytes.len > 0
ensures result.first == 0
{
    p.first = 9;
    Pair { first: 0, second: 7 }
}
"#;

    fn terms(proposition: &Proposition) -> [&ScalarTerm; 2] {
        match proposition {
            Proposition::Equal(left, right) | Proposition::LessThan(left, right) => [left, right],
            other => panic!("the source spells a scalar equality and a length bound: {other:?}"),
        }
    }

    for (input, extent, accepted) in [
        ("mut p: Pair", false, true),
        ("p: &mut Pair", false, false),
        ("p: &mut Pair", true, false),
    ] {
        let typed = frontier_of(
            "machine product_entry() { }",
            &SOURCE.replace("INPUT", input),
        );
        let entry = symbol(&typed, "build_entry");
        let elaborate = || {
            typed_trees_to_terminal_psi::elaborate_for(
                &typed,
                roster(&[entry], &[Some(package())], source::DependencyScope::Build),
                terminal_codec::TerminalPublication::Invocation,
            )
            .unwrap()
        };
        let baseline = judge(Ok(elaborate())).unwrap();
        assert!(
            matches!(baseline, RouteVerdict::Accepted(_)),
            "{baseline:?}"
        );

        let mut received = elaborate();
        let machine = &mut received.module.machines[0];
        assert_eq!(machine.contract.ensures.len(), 1);
        assert_eq!(machine.contract.requires.len(), 2);
        let [pair, bytes] = machine.structural_parameters.as_slice() else {
            panic!("the actual source has exactly two structural inputs")
        };
        assert_eq!(
            pair.access,
            if accepted {
                terminal_psi::StructuralAccess::Owned
            } else {
                terminal_psi::StructuralAccess::MutableBorrow
            }
        );
        assert_eq!(bytes.access, terminal_psi::StructuralAccess::SharedBorrow);
        let result_path = terms(&machine.contract.ensures[0].proposition)
            .into_iter()
            .find_map(|term| match term {
                ScalarTerm::IntegerField { root, path, .. }
                    if machine.result.as_structural().unwrap().place == *root =>
                {
                    Some(path)
                }
                _ => None,
            })
            .expect("the authored promise projects the actual result's first field");
        let mut premises = machine.contract.requires.iter().filter(|clause| {
            terms(clause).into_iter().any(|term| match term {
                ScalarTerm::ViewExtent { root, path, .. } if extent => {
                    *root == bytes.place && path.is_empty()
                }
                ScalarTerm::IntegerField { root, path, .. } if !extent => {
                    *root == pair.place && path == result_path
                }
                _ => false,
            })
        });
        let premise = premises
            .next()
            .expect("the exact source observation")
            .clone();
        assert!(premises.next().is_none(), "the observation is unique");
        // Keep the real declaration, formal roster, return and obligation
        // identities. Only the received claim is different.
        machine.contract.ensures[0].proposition = premise;
        let verdict = judge(Ok(received));
        if accepted {
            assert!(
                matches!(verdict, Ok(RouteVerdict::Accepted(_))),
                "an owned input still names its original value: {verdict:?}"
            );
        } else {
            let refusal = verdict.unwrap_err();
            assert_eq!(refusal.reason, RefusalReason::Unsupported, "{refusal:?}");
            assert_eq!(
                refusal.detail,
                "borrowed completion storage has no post-call observation"
            );
        }
    }
}
