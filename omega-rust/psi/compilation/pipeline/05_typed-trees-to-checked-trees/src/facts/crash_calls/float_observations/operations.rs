//! Normal results of exact sealed IEEE operations over completed operands.
//! Selection, operand custody and result policy remain independent evidence.

use super::{FloatObservations, extend_values};
use checked_trees::{
    CheckedArithmeticPolicyAdapter, CheckedValueOrigin, FlowOperandFact, FlowStateFact,
};
use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
use typed_trees::expression::{BinaryOperator, ExpressionHandle};

impl FloatObservations<'_> {
    pub(super) fn operation(
        &self,
        state: &FlowStateFact,
        capture: &FlowOperandFact,
        expression: ExpressionHandle,
    ) -> Option<Vec<FloatMeaning>> {
        let mut selection = None;
        let mut ambiguous = false;
        self.operators.for_each_resolved_expression_selection(
            expression,
            |origin, selected, adapter, use_fact| {
                if selection.is_some() {
                    ambiguous = true;
                } else {
                    selection = Some((origin, selected, adapter, use_fact));
                }
            },
        );
        let (origin, selected, adapter, use_fact) = selection?;
        if ambiguous
            || !matches!(origin, CheckedValueOrigin::StateStatement {
                machine_symbol, state_symbol, statement_index, ..
            } if machine_symbol == state.machine_symbol
                && state_symbol == capture.state_symbol
                && statement_index == capture.statement_index)
        {
            return None;
        }
        let operator = typed_trees::operator::declaration_by_symbol(self.program, selected)?;
        if !typed_trees::operator::is_sealed_toolchain_float_operator(self.program, operator) {
            return None;
        }
        let (operation, format) =
            typed_trees::operator::primitive_float_binary_semantics(self.program, operator)?;
        if !matches!(
            operation,
            BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
        ) {
            return None;
        }
        let parameters = self.program.operator_parameters(operator);
        let operands = if let Some(use_fact) = use_fact {
            use_fact.operands(self.program)?
        } else {
            crate::facts::operator_crashes::named_call_operands(
                self.program,
                self.program.expression_table.call_node(expression)?,
                parameters,
            )?
        };
        if parameters.len() != 2 || operands.len() != 2 {
            return None;
        }
        let invocation = crate::checks::InvocationContexts::for_selected_expression_in_flow(
            self.program,
            self.operators,
            self.flow,
            self.semantic,
            expression,
            selected,
            &operands,
        );
        let constraints = invocation.entry_constraints()?;
        let epoch = invocation.operand_at(1)?;
        let read = |position: usize| {
            self.capture_values(
                state,
                invocation.operand_at(position)?,
                parameters[position].type_reference,
                constraints,
                epoch,
            )
        };
        let (left_values, right_values) = (read(0)?, read(1)?);
        let format = match format {
            numerics::literals::FloatFormat::F32 => FloatFormat::BINARY32,
            numerics::literals::FloatFormat::F64 => FloatFormat::BINARY64,
        };
        let mut results = Vec::new();
        for left in &left_values {
            for right in &right_values {
                let result = match operation {
                    BinaryOperator::Add => FloatSemantics::add(format, left, right),
                    BinaryOperator::Subtract => FloatSemantics::subtract(format, left, right),
                    BinaryOperator::Multiply => FloatSemantics::multiply(format, left, right),
                    BinaryOperator::Divide => FloatSemantics::divide(format, left, right),
                    _ => return None,
                };
                let result = match adapter {
                    CheckedArithmeticPolicyAdapter::None => result,
                    CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { format: adapted }
                        if adapted == format =>
                    {
                        FloatSemantics::apply_trapping_policy(result).ok()?
                    }
                    CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
                        format: adapted,
                    } if adapted == format => {
                        if operation == BinaryOperator::Divide {
                            FloatSemantics::apply_saturating_divide_policy(
                                format, left, right, result,
                            )
                        } else {
                            FloatSemantics::apply_saturating_policy(format, &[left, right], result)
                        }
                    }
                    _ => return None,
                };
                // No equality-class representative becomes an exact operand.
                // Every signed-zero possibility and captured alternative must
                // support the final observation independently.
                extend_values(&mut results, [result]);
            }
        }
        (!results.is_empty()).then_some(results)
    }
}
