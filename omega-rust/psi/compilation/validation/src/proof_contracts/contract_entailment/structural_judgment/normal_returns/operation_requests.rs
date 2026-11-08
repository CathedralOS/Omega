//! Sparse membership for one ordered observation pass. This index selects
//! outputs and blocks folding over them; it supplies no execution or proof fact.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

type ExpressionKey = (u32, u32);

#[derive(Default)]
pub(super) struct OperationRequests {
    requested: BTreeSet<ExpressionKey>,
    containing: BTreeSet<ExpressionKey>,
}

fn key(expression: ExpressionHandle) -> ExpressionKey {
    (expression.arena_index(), expression.generation())
}

impl OperationRequests {
    pub(super) fn prepare(
        program: &dyn crate::CheckingInput,
        roots: &[ExpressionHandle],
        operations: &[ExpressionHandle],
    ) -> Self {
        let requested = operations.iter().copied().map(key).collect::<BTreeSet<_>>();
        if requested.is_empty() {
            return Self::default();
        }
        // The same VALUES edges own expression_contains. Saved subjects stay
        // leaves, and full generation coordinates keep unrelated slots apart.
        let mut parents = BTreeMap::<ExpressionKey, Vec<ExpressionKey>>::new();
        let mut visited = BTreeSet::new();
        let mut pending = roots.to_vec();
        while let Some(expression) = pending.pop() {
            if !expression.is_valid() || !visited.insert(key(expression)) {
                continue;
            }
            program.subtree_children(
                program.expression_table.expression(expression),
                typed_trees::SubtreeEdges::VALUES,
                |child| {
                    if child.is_valid() {
                        parents.entry(key(child)).or_default().push(key(expression));
                        pending.push(child);
                    }
                    false
                },
            );
        }
        let mut containing = BTreeSet::new();
        let mut pending = requested.iter().copied().collect::<Vec<_>>();
        while let Some(expression) = pending.pop() {
            if containing.insert(expression)
                && let Some(owners) = parents.get(&expression)
            {
                pending.extend(owners);
            }
        }
        Self {
            requested,
            containing,
        }
    }

    pub(super) fn contains(&self, expression: ExpressionHandle) -> bool {
        self.requested.contains(&key(expression))
    }

    pub(super) fn contains_in_subtree(&self, expression: ExpressionHandle) -> bool {
        self.containing.contains(&key(expression))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use typed_trees::expression::{BinaryOperator, TableBinaryExpression, TableSavedSubject};

    #[test]
    fn operation_request_index_matches_value_containment() {
        let mut program = typed_trees::TypedTrees::default();
        let left = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let right = program
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        let capture = program.expression_table.insert_match_subject(left);
        let saved = program
            .expression_table
            .insert(ExpressionNode::SavedSubject(TableSavedSubject {
                capture,
                component: Default::default(),
                pattern_head: ExpressionHandle::invalid(),
            }));
        let joined =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: BinaryOperator::And,
                    right,
                }));
        let repeated =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: joined,
                    operator: BinaryOperator::Or,
                    right: joined,
                }));
        let roots = [repeated, saved];
        let nodes = [left, right, saved, joined, repeated];
        for requested in [
            &[][..],
            &[left][..],
            &[saved, right][..],
            &[joined, joined][..],
        ] {
            let index = OperationRequests::prepare(&program, &roots, requested);
            for expression in nodes {
                assert_eq!(index.contains(expression), requested.contains(&expression));
                assert_eq!(
                    index.contains_in_subtree(expression),
                    requested.iter().any(|target| {
                        crate::expression_owners::expression_contains(&program, expression, *target)
                    })
                );
            }
            let other_generation =
                ExpressionHandle::from_parts(left.arena_index(), left.generation() + 1);
            assert!(!index.contains(other_generation));
            assert!(!index.contains_in_subtree(other_generation));
        }
    }
}
