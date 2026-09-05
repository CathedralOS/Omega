use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::ContractExpressionEvaluator;

impl ContractExpressionEvaluator<'_, '_> {
    /// The strongest inexpensive interval known for an integer expression at
    /// this call site. Contract parameters first rebind to the caller's actual
    /// argument; an Exact declared range on that argument is a store-enforced
    /// invariant and may therefore discharge a callee comparison even when the
    /// argument's initializer is not constant-foldable.
    pub(super) fn integer_bounds(&self, expression: ExpressionHandle) -> Option<(i64, i64)> {
        if let Some(value) = self.integer_value(expression) {
            return Some((value, value));
        }

        let ExpressionNode::Name(path) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        let name = self
            .program
            .expression_table
            .name_path_members(path.members)
            .last()
            .map(|name| name.as_str());
        let argument = self.argument_for_parameter(path.head_symbol, path.symbol, name)?;

        if let Some(value) = self.integer_value(argument) {
            return Some((value, value));
        }
        crate::checks::ranges::expression_enforced_declared_range(
            self.program,
            self.caller_machine,
            self.caller_state,
            argument,
        )
    }

    pub(super) fn integer_value(&self, expression: ExpressionHandle) -> Option<i64> {
        // Re-entering an expression whose evaluation is still in progress
        // means call-site following looped (recursive machine); stand down
        // with unknown rather than overflow the stack.
        Self::guarding_cycles(&self.active_evaluations, expression, || {
            self.integer_value_of_resolved(expression)
        })
    }

    fn integer_value_of_resolved(&self, expression: ExpressionHandle) -> Option<i64> {
        let expression = self.resolved_expression(expression).unwrap_or(expression);
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => {
                let left = self.integer_value(binary.left)?;
                let right = self.integer_value(binary.right)?;
                match binary.operator {
                    BinaryOperator::Add => left.checked_add(right),
                    BinaryOperator::Divide => {
                        (right != 0).then(|| left.checked_div(right)).flatten()
                    }
                    BinaryOperator::Modulo => {
                        (right != 0).then(|| left.checked_rem(right)).flatten()
                    }
                    BinaryOperator::Multiply => left.checked_mul(right),
                    BinaryOperator::Subtract => left.checked_sub(right),
                    BinaryOperator::And
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::Equal
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Or
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight => None,
                }
            }
            ExpressionNode::Integer(value) => value.value_i64(),
            ExpressionNode::Member(member) if member.member.as_str() == "len" => self
                .collection_length(member.receiver)
                .and_then(|length| i64::try_from(length).ok()),
            ExpressionNode::Borrow(inner) => self.integer_value(inner.target),
            _ => None,
        }
    }
}
