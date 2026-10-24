//! Match owns one saved subject and evaluates only reached patterns. Selected
//! equality consumes those values, not the source operand expressions retained
//! for custody: reinterpreting the expressions would duplicate their effects.
//! A settled compiler execution child authorizes IEEE semantics; a declaration,
//! readable operator name or source classification alone does not.
use crate::interpreter::evaluator::{
    ArithmeticDomain, BinaryOperator, EvalResult, Evaluator, ExpressionHandle, Frame, Halt,
    PrimitiveType, Value, unsupported,
};
use arena::Handle;
use checked_trees::CheckedOperatorOccurrence;
use typed_trees::expression::{MatchPattern, TableMatchArm, TableMatchExpression};

impl Evaluator<'_> {
    pub(in crate::interpreter::evaluator) fn with_selected_match<ResultValue>(
        &mut self,
        expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        frame: &mut Frame,
        evaluate: impl FnOnce(&mut Self, ExpressionHandle, &mut Frame) -> EvalResult<ResultValue>,
    ) -> EvalResult<ResultValue> {
        let outer_captures = frame.match_subjects.len();
        let result = self
            .select_match_arm(expression, dispatch, frame)
            .and_then(|selected| evaluate(self, selected, frame));
        frame.match_subjects.truncate(outer_captures);
        result
    }

    pub(in crate::interpreter::evaluator) fn select_match_arm(
        &mut self,
        expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        frame: &mut Frame,
    ) -> EvalResult<ExpressionHandle> {
        let destination = self
            .expression_scalar_type(dispatch.subject, frame)
            .map(|(primitive, _)| primitive);
        let subject = if dispatch.capture.is_valid() {
            let cell = self.eval_read_cell(dispatch.subject, frame)?;
            let reference = self
                .program
                .machine_by_symbol(frame.machine_symbol)
                .and_then(|machine| {
                    self.program
                        .state_in_machine(machine, frame.state_symbol)
                        .and_then(|state| {
                            validation::expression_result_type_reference(
                                self.program,
                                machine,
                                state,
                                dispatch.subject,
                            )
                        })
                });
            let cell = if let Some(reference) = reference
                && self.program.type_multiplicity(reference)
                    == language_semantics::Multiplicity::Unrestricted
            {
                // A copied subject freezes its carrier before any guard can
                // replace the source. Deep value capture preserves reference
                // descriptors, not snapshots of their borrowed pointees.
                let value = cell.borrow().clone();
                let captured = self.capture_value_operand(value, reference)?;
                self.allocate_cell(captured)?
            } else {
                cell
            };
            frame.match_subjects.push((dispatch.capture, cell.clone()));
            let value = cell.borrow().clone();
            self.observe_reference_value(value)?
        } else {
            self.eval_expression_with_destination(dispatch.subject, destination, frame)?
        };
        for (ordinal, arm) in self
            .program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .enumerate()
        {
            let matches = match arm.pattern {
                MatchPattern::Destructure(pattern) => {
                    // The normalized predicate retains selected equality and
                    // ordinary short-circuit effects. Reconstructing it from
                    // raw fields here would bypass those language operations.
                    self.eval_expression(pattern.condition, frame)?
                        .as_bool()
                        .ok_or_else(|| {
                            Halt::Unsupported(
                                "structural pattern predicate is not Boolean".to_owned(),
                            )
                        })?
                }
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
                        || subject.is_float()
                        || pattern.is_float()
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
        // Product selections arrive from its execution owner. Build has its
        // separately admitted comparison executions; they are never a fallback
        // for an unbound Product call.
        let product_executions = self.selected_product_float_comparisons.unwrap_or(&[]);
        let build_executions = facts
            .selected_float_comparisons
            .iter()
            .filter(|_| self.selected_product_float_comparisons.is_none())
            .map(|(_, execution)| execution);
        let mut executions = product_executions
            .iter()
            .chain(build_executions)
            .filter(|execution| execution.operator_use() == operator_use);
        let execution = executions.next().ok_or_else(missing)?;
        if executions.next().is_some() {
            return Err(missing());
        }
        let primitive = execution
            .validated_primitive(self.program, facts)
            .ok_or_else(missing)?;
        if destination.is_some_and(|destination| destination != primitive) {
            return Err(missing());
        }
        if !subject.is_float() || !pattern.is_float() {
            return Err(missing());
        }
        self.eval_float_binary(
            BinaryOperator::Equal,
            subject,
            pattern,
            Some((primitive, ArithmeticDomain::Exact)),
            None,
        )
    }
}
