//! A dispatch retains one subject computation and its covered authored prefix.

use super::*;
use checked_trees::{CheckedScalarDispatchArm, CheckedScalarDispatchPattern};
use typed_trees::expression::{MatchPattern, TableMatchExpression};

impl Builder<'_, '_> {
    pub(super) fn dispatch(
        &mut self,
        source_expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        result_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        if result_type != PrimitiveType::Bool && !is_integer(result_type) {
            return None;
        }
        let subject = if let Some(subject_type) =
            validation::match_subject_primitive_type(self.program, dispatch)
        {
            if subject_type != PrimitiveType::Bool && !is_integer(subject_type) {
                return None;
            }
            self.expression(dispatch.subject, subject_type)?
        } else if let Some(operand) = self.integer_operand(dispatch.subject)
            && scalar_expression_type(&operand.value).is_some_and(is_integer)
        {
            self.materialize_integer(operand)?
        } else {
            self.expression(dispatch.subject, PrimitiveType::Bool)?
        };
        let subject_type = self.plans.nodes.get(subject).primitive_type;
        let authored = self.program.expression_table.match_arms(dispatch.arms);
        let mut arms = Vec::new();
        let mut boolean_coverage = [false; 2];
        let mut covered = false;
        for (ordinal, arm) in authored.iter().enumerate() {
            let pattern = match arm.pattern {
                MatchPattern::Wildcard => {
                    covered = true;
                    CheckedScalarDispatchPattern::Wildcard
                }
                MatchPattern::Value(expression) => {
                    if subject_type == PrimitiveType::Bool
                        && let ExpressionNode::Boolean(value) =
                            self.program.expression_table.expression(expression)
                    {
                        boolean_coverage[usize::from(*value)] = true;
                        covered = boolean_coverage.iter().all(|value| *value);
                    }
                    CheckedScalarDispatchPattern::Value(self.expression(expression, subject_type)?)
                }
            };
            let value = self.expression(arm.value, result_type)?;
            let source_arm = arena::Handle::from_parts(
                dispatch
                    .arms
                    .start()
                    .arena_index()
                    .checked_add(u32::try_from(ordinal).ok()?)?,
                dispatch.arms.start().generation(),
            );
            arms.push(CheckedScalarDispatchArm {
                source_arm,
                pattern,
                value,
            });
            if covered {
                break;
            }
        }
        if !covered {
            return None;
        }
        let arms = self.plans.dispatch_arms.insert_many(arms);
        Some(self.insert(
            result_type,
            CheckedScalarComputationKind::Dispatch {
                source_expression,
                subject,
                arms,
            },
        ))
    }
}
