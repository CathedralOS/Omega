//! Match owns one saved subject and evaluates only reached patterns. Selected
//! equality consumes those values, not the source operand expressions retained
//! for custody: reinterpreting the expressions would duplicate their effects.
//! A settled compiler execution child authorizes IEEE semantics; a declaration,
//! readable operator name or source classification alone does not.

use super::*;
use arena::Handle;
use checked_trees::CheckedOperatorOccurrence;
use typed_trees::expression::{MatchPattern, TableMatchArm, TableMatchExpression};

impl Evaluator<'_> {
    pub(in super::super) fn select_match_arm(
        &mut self,
        expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        frame: &Frame,
    ) -> EvalResult<ExpressionHandle> {
        let destination = self
            .expression_scalar_type(dispatch.subject, frame)
            .map(|(primitive, _)| primitive);
        let subject =
            self.eval_expression_with_destination(dispatch.subject, destination, frame)?;
        for (ordinal, arm) in self
            .program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .enumerate()
        {
            let matches = match arm.pattern {
                MatchPattern::Wildcard => true,
                MatchPattern::Value(pattern) => {
                    let pattern =
                        self.eval_expression_with_destination(pattern, destination, frame)?;
                    let arm_index = u32::try_from(ordinal)
                        .ok()
                        .and_then(|ordinal| {
                            dispatch.arms.start().arena_index().checked_add(ordinal)
                        })
                        .ok_or_else(|| {
                            Halt::Unsupported("Match arm identity overflowed".to_owned())
                        })?;
                    let source_arm =
                        Handle::from_parts(arm_index, dispatch.arms.start().generation());
                    let selected_float = self.operator_facts.is_some_and(|facts| {
                        facts.uses.iter().any(|(handle, selected)| {
                            selected.expression == expression
                                && selected.origin.machine_symbol() == Some(frame.machine_symbol)
                                && selected.occurrence
                                    == CheckedOperatorOccurrence::MatchEquality { source_arm }
                                && facts
                                    .selected_float_comparison(self.program, handle)
                                    .is_some()
                        })
                    });
                    // Projection metadata may be absent even when evaluation
                    // produces a float. Conversely, malformed runtime values
                    // cannot erase the source's selected floating comparison.
                    let value = if selected_float
                        || matches!(destination, Some(PrimitiveType::F32 | PrimitiveType::F64))
                        || matches!(subject, Value::Float(_))
                        || matches!(pattern, Value::Float(_))
                    {
                        self.eval_selected_match_equality(
                            expression,
                            source_arm,
                            &subject,
                            &pattern,
                            destination,
                            frame,
                        )?
                    } else {
                        self.eval_binary(
                            BinaryOperator::Equal,
                            subject.clone(),
                            pattern,
                            false,
                            None,
                            None,
                        )?
                    };
                    value.as_bool().ok_or_else(|| {
                        Halt::Unsupported("match equality did not produce a Boolean".to_owned())
                    })?
                }
            };
            if matches {
                return Ok(arm.value);
            }
        }
        unsupported("checked match has no selected arm; exhaustive dispatch evidence is required")
    }

    fn eval_selected_match_equality(
        &self,
        expression: ExpressionHandle,
        source_arm: Handle<TableMatchArm>,
        subject: &Value,
        pattern: &Value,
        destination: Option<PrimitiveType>,
        frame: &Frame,
    ) -> EvalResult<Value> {
        let missing = || {
            Halt::Unsupported(
                "selected floating Match equality has no interpreter execution custody".to_owned(),
            )
        };
        let facts = self.operator_facts.ok_or_else(missing)?;
        let mut uses = facts.uses.iter().filter(|(_, selected)| {
            selected.expression == expression
                && selected.origin.machine_symbol() == Some(frame.machine_symbol)
                && selected.occurrence == CheckedOperatorOccurrence::MatchEquality { source_arm }
        });
        let (operator_use, _) = uses.next().ok_or_else(missing)?;
        if uses.next().is_some() {
            return Err(missing());
        }
        let mut executions = facts
            .selected_float_comparisons
            .iter()
            .filter(|(_, execution)| execution.operator_use() == operator_use);
        let (_, execution) = executions.next().ok_or_else(missing)?;
        if executions.next().is_some() {
            return Err(missing());
        }
        let primitive = execution
            .validated_primitive(self.program, facts)
            .ok_or_else(missing)?;
        if destination.is_some_and(|destination| destination != primitive) {
            return Err(missing());
        }
        let (Value::Float(subject), Value::Float(pattern)) = (subject, pattern) else {
            return Err(missing());
        };
        self.eval_float_binary(
            BinaryOperator::Equal,
            *subject,
            *pattern,
            Some((primitive, ArithmeticDomain::Exact)),
            None,
        )
    }
}
