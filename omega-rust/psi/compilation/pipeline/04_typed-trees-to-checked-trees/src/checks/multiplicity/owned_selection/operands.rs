//! First-match reachability shared by flow capture and custody consumers.
use arena::{Handle, HandleSpan};
use typed_trees::expression::{ExpressionNode, MatchPattern, TableMatchArm};

/// Ordered first-match reachability. The source type checker still visits all
/// authored arms. Only literal equality and complete Boolean/wildcard coverage
/// remove ownership alternatives here; no predicate theorem is invented.
pub(in crate::checks) fn reachable_arms(
    program: &dyn ::validation::CheckingInput,
    arms: HandleSpan<TableMatchArm>,
) -> Vec<(Handle<TableMatchArm>, &TableMatchArm)> {
    let mut selected = Vec::new();
    let mut patterns = Vec::new();
    let mut booleans = [false; 2];
    for (ordinal, arm) in program.expression_table.match_arms(arms).iter().enumerate() {
        let mut covered = false;
        match arm.pattern {
            MatchPattern::Destructure(_) => {}
            MatchPattern::Wildcard => covered = true,
            MatchPattern::Value(pattern) => {
                let node = program.expression_table.expression(pattern);
                let duplicate = patterns.iter().any(|previous| {
                    match (program.expression_table.expression(*previous), node) {
                        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => {
                            left == right
                        }
                        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left
                            .value_bignum()
                            .zip(right.value_bignum())
                            .is_some_and(|(left, right)| left == right),
                        _ => false,
                    }
                });
                if duplicate {
                    continue;
                }
                patterns.push(pattern);
                if let ExpressionNode::Boolean(value) = node {
                    booleans[usize::from(*value)] = true;
                    covered = booleans == [true, true];
                }
            }
        }
        let handle = arms.handle_at(ordinal as u32);
        selected.push((handle, arm));
        if covered {
            break;
        }
    }
    selected
}
