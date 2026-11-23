//! What resolution and typing hand stage 04 about a `match` without a
//! subject: the typed match has no subject and no subject capture, and each
//! arm but `_` is a condition over values that already exist, not a pattern
//! to compare with a subject. The program is `GUARDED` in
//! `05_prepared-typed-trees-to-lowered-program/tests/support/dispatch_arms.rs`;
//! stage 05's side of the handoff is that stage's `tests/subjectless_match.rs`.
//!
//! The rule: `wiki/spec/language/patterns.md`, "In a dispatch without a
//! subject, an arm condition such as `ready -> 1` reads the existing value of
//! `ready`".

#[path = "../../05_prepared-typed-trees-to-lowered-program/tests/support/dispatch_arms.rs"]
mod program;

use source_compiler::representations::source_state::expression::{
    BinaryOperator, ExpressionNode, MatchPattern,
};

#[test]
fn a_match_without_a_subject_has_condition_arms_over_the_parameters() {
    let trees = program::typed_source(program::GUARDED);
    let matches = program::subjectless_matches(&trees);
    assert_eq!(matches.len(), 2, "`larger` and `gap` each hold one");
    let (_, larger) = matches[0];
    assert!(!larger.capture.is_valid(), "nothing captures a subject");

    let arms = trees.expression_table.match_arms(larger.arms);
    let [first, last] = arms else {
        panic!("two arms: {arms:?}")
    };
    assert!(matches!(last.pattern, MatchPattern::Wildcard));
    let MatchPattern::Guard(condition) = first.pattern else {
        panic!("the first arm is a condition: {:?}", first.pattern)
    };

    // The condition is `a > b` over the machine's own parameters.
    let ExpressionNode::Binary(condition) = trees.expression_table.expression(condition) else {
        panic!("the condition is a comparison")
    };
    assert_eq!(condition.operator, BinaryOperator::Greater);
    let name = |operand| match trees.expression_table.expression(operand) {
        ExpressionNode::Name(name) if name.symbol.is_valid() => {
            trees.symbols.name(name.symbol).to_owned()
        }
        other => panic!("an operand is a resolved name: {other:?}"),
    };
    assert_eq!([name(condition.left), name(condition.right)], ["a", "b"]);
}
