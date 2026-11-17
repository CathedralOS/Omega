//! A match hands its selected owned value to one common continuation.
//! The ordinary structural successor bindings transfer that custody once;
//! selecting an arm grants no copy permission to its result.

use super::build_purpose::judge_build;
use super::*;

const SOURCE: &str = r#"
data Choice [copy] { case Left; case Right; }
data Item { value: u32; }
machine choose(choice: Choice) -> Item {
    match choice {
        Choice::Left -> Item { value: 7 }
        Choice::Right -> Item { value: 11 }
    }
}
machine consume(item: Item) { }
machine build_entry() {
    let selected: Item = choose(Choice::Left {});
    consume(selected);
}
"#;

#[test]
fn an_owned_match_result_reaches_a_consuming_call() {
    let verdict = judge_build(SOURCE);
    let Ok(RouteVerdict::Accepted(accepted)) = verdict else {
        panic!("the selected owned result transfers into its continuation: {verdict:?}");
    };
    accepted.publish_invocation().unwrap();
}

#[test]
fn an_owned_match_result_cannot_be_consumed_twice() {
    let source = SOURCE.replace(
        "consume(selected);",
        "consume(selected); consume(selected);",
    );
    let verdict = judge_build(&source);
    let Ok(RouteVerdict::Rejected { unestablished, .. }) = verdict else {
        panic!("a second consumption must fail ownership checking: {verdict:?}");
    };
    assert!(unestablished.iter().any(|failure| failure.detail
        == "owned storage is used after its value was transferred or consumed"));
}

#[test]
fn an_ended_arm_supplies_no_owned_join_value() {
    let source = SOURCE
        .replace("machine choose(choice: Choice) -> Item {", "machine stop() -> Item crashes Trap { crash Trap; }\nmachine choose(choice: Choice) -> Item crashes Trap {")
        .replace("Choice::Right -> Item { value: 11 }", "Choice::Right -> stop()")
        .replace("machine build_entry() {", "machine build_entry() crashes Trap {");
    let verdict = judge_build(&source);
    let Ok(RouteVerdict::Accepted(accepted)) = verdict else {
        panic!("an ended arm creates no incoming owner: {verdict:?}");
    };
    accepted.publish_invocation().unwrap();
}
