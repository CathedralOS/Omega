//! A match hands its selected owned value to one common continuation.
//! The ordinary structural successor bindings transfer that custody once;
//! selecting an arm grants no copy permission to its result.

use super::build_purpose::judge_build;
use super::*;

#[test]
fn an_owned_match_result_reaches_a_consuming_call() {
    let source = r#"
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
    let verdict = judge_build(source);
    let Ok(RouteVerdict::Accepted(accepted)) = verdict else {
        panic!("the selected owned result transfers into its continuation: {verdict:?}");
    };
    accepted.publish_invocation().unwrap();
}
