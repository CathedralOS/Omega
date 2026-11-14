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
    // is changed before return. Its current field cannot reuse the entry
    // fact. The borrowed view's extent still has no completion observation.
    const SOURCE: &str = r#"
data Pair [copy] { first: u32; second: u32; }
machine build_entry(INPUT, bytes: &[u8]) -> Pair
requires p.first == 0, bytes.len > 0
ensures result.first == 0
{
    let original: u32 = p.first;
    p.first = 9;
    Pair { first: original, second: 7 }
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
                    if machine.result.structural().unwrap().place == *root =>
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
        } else if !extent {
            let Ok(RouteVerdict::Rejected { unproved, .. }) = verdict else {
                panic!("the changed borrowed field must reject its old entry fact: {verdict:?}")
            };
            let [failure] = unproved.as_slice() else {
                panic!("one changed guarantee: {unproved:?}")
            };
            assert!(matches!(
                failure.kind,
                ObligationKind::NormalGuarantee { clause: 0, .. }
            ));
            assert_eq!(failure.cause, UnprovedCause::NoDerivation);
        } else {
            let Ok(RouteVerdict::FallBack(refusal)) = verdict else {
                panic!("unsupported borrowed completion must remain refused: {verdict:?}")
            };
            assert_eq!(refusal.reason, RefusalReason::Unsupported, "{refusal:?}");
            assert_eq!(
                refusal.detail,
                "borrowed completion storage has no post-call observation"
            );
        }
    }
}

#[test]
fn borrowed_field_guarantees_connect_entry_and_completion_at_each_call() {
    // The helper must prove the update it promises. Each later call observes
    // the preceding call's actual completion, rather than reusing entry zero.
    const SOURCE: &str = r#"
data Pair [copy] { first: u32; second: u32; }
machine step(p: &mut Pair)
requires p.first < 32
ensures embed(p.first) == embed(old(p.first)) + 1
{ p.first = p.first + 1; }
machine build_entry() -> u32
ensures result == 2
{
    let mut value: Pair = Pair { first: 0, second: 9 };
    step(&mut value);
    step(&mut value);
    value.first
}
"#;
    let prepare = |text: &str| {
        super::calling_policies::prepared(
            super::build_purpose::frontier_of(text, "machine unused_build_body() { }"),
            &["build_entry"],
            terminal_codec::TerminalPublication::Portable,
        )
    };
    let owners = |input: &terminal_check_tables::compilation::TerminalCompilation| {
        let terminal_check_tables::compilation::TerminalCompilation::Prepared(input) = input else {
            panic!("the source must prepare")
        };
        let root = &input.closure.as_ref().unwrap().module.machines[0];
        let calls: Vec<_> = root
            .operations()
            .filter_map(|operation| terminal_psi::direct_call_parts(&operation.kind))
            .collect();
        let [first, second] = calls.as_slice() else {
            panic!("the actual caller has two step calls")
        };
        assert_eq!(first.callee, second.callee);
        (root.id, first.callee)
    };
    let input = prepare(SOURCE);
    owners(&input);
    let verdicts = crate::judge_compilation(input);
    let [Ok(RouteVerdict::Accepted(accepted))] = verdicts.as_slice() else {
        panic!("{verdicts:?}")
    };
    accepted.publish().unwrap();

    for (changed, helper) in [
        (
            SOURCE.replace("embed(old(p.first)) + 1", "embed(old(p.first)) + 2"),
            true,
        ),
        (
            SOURCE.replace("ensures result == 2", "ensures result == 3"),
            false,
        ),
    ] {
        let input = prepare(&changed);
        let (root, step) = owners(&input);
        let verdicts = crate::judge_compilation(input);
        let [
            Ok(RouteVerdict::Rejected {
                unproved,
                formations,
                violations,
                unestablished,
                conflicts,
                exceeded,
            }),
        ] = verdicts.as_slice()
        else {
            panic!("{verdicts:?}")
        };
        assert!(
            formations.is_empty()
                && violations.is_empty()
                && unestablished.is_empty()
                && conflicts.is_empty()
                && exceeded.is_empty(),
            "{verdicts:?}"
        );
        let expected = if helper { step } else { root };
        assert!(
            unproved.iter().any(|row| row.machine == expected
                && row.cause == UnprovedCause::NoDerivation
                && matches!(row.kind, ObligationKind::NormalGuarantee { clause: 0, .. })),
            "{unproved:?}"
        );
        if !helper {
            assert_eq!(
                unproved.len(),
                1,
                "only the caller's false result promise: {unproved:?}"
            );
        }
    }
}

#[test]
fn owned_field_guarantees_keep_the_input_separate_from_working_storage() {
    const SOURCE: &str = r#"
data Pair [copy] { first: u32; second: u32; }
machine build_entry(mut p: Pair) -> Pair
requires p.first == 0
ensures p.first == old(p.first), result.first == 9
{ p.first = 9; p }
"#;
    // Keep the actual test-package source identity for an independently
    // received artifact; an invocation-only module is not made portable.
    let prepared = super::calling_policies::prepared(
        super::build_purpose::frontier_of(SOURCE, "machine unused_build_body() { }"),
        &["build_entry"],
        terminal_codec::TerminalPublication::Portable,
    );
    let verdicts = crate::judge_compilation(prepared);
    let [Ok(RouteVerdict::Accepted(accepted))] = verdicts.as_slice() else {
        panic!("{verdicts:?}")
    };
    accepted.publish().unwrap();
    let bytes = terminal_codec::encode_module(&accepted.elaboration.module).unwrap();
    let received = terminal_codec::decode_module(&bytes).unwrap();
    assert_eq!(received, accepted.elaboration.module);

    let mut forged = received.clone();
    let machine = &mut forged.machines[0];
    let result_root = machine.result.structural().unwrap().place;
    let result_term = machine
        .contract
        .ensures
        .iter_mut()
        .find_map(|clause| {
            let Proposition::Equal(left, right) = &mut clause.proposition else {
                return None;
            };
            [left, right].into_iter().find(|term| {
                matches!(
                    term,
                    ScalarTerm::IntegerField { root, .. } if *root == result_root
                )
            })
        })
        .expect("the actual source promises a field of its returned Pair");
    let ScalarTerm::IntegerField { version, .. } = result_term else {
        unreachable!()
    };
    *version = semantic_vocabulary::ContentPlaceVersion::Entry;
    let forged = terminal_codec::encode_module(&forged).unwrap();
    let received = terminal_codec::decode_module(&forged);
    assert!(
        matches!(received.as_ref(),
            Err(terminal_codec::CodecError::MalformedProposition(
                semantic_vocabulary::PropositionError::EntryNonInputStructuralPlace(root))) if *root == result_root),
        "entry observation of result {result_root:?}: {:?}",
        received.as_ref().map(|_| ())
    );

    let mut old = bytes;
    old[8..10].copy_from_slice(&264u16.to_le_bytes());
    assert!(matches!(
        terminal_codec::decode_module(&old),
        Err(terminal_codec::CodecError::UnsupportedFormatMarker(264))
    ));

    let changed = SOURCE.replace("p.first == old(p.first)", "p.first == 9");
    let verdict = judge_build(&changed).unwrap();
    let RouteVerdict::Rejected {
        unproved,
        formations,
        violations,
        unestablished,
        conflicts,
        exceeded,
    } = verdict
    else {
        panic!("{verdict:?}")
    };
    assert!(
        formations.is_empty()
            && violations.is_empty()
            && unestablished.is_empty()
            && conflicts.is_empty()
            && exceeded.is_empty()
    );
    let [failure] = unproved.as_slice() else {
        panic!("one false owned-input guarantee: {unproved:?}")
    };
    assert!(matches!(
        failure.kind,
        ObligationKind::NormalGuarantee { clause: 0, .. }
    ));
    assert_eq!(failure.cause, UnprovedCause::NoDerivation);
}
