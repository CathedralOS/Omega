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
