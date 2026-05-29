use omega_checked_trees::{FlowCallFact, FlowStateFact};
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::state::State;

mod resolution;

pub(super) fn call_site_proves_boolean_contract_expression(
    program: &omega_typed_trees::TypedTrees,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    call_site: &crate::CallSite<'_>,
    target_state: &State,
    expression: ExpressionHandle,
) -> bool {
    let Some(caller_state) =
        crate::find_state_in_machine(program, state_flow.machine_symbol, state_flow.state_symbol)
    else {
        return false;
    };

    ContractExpressionEvaluator {
        program,
        caller_state,
        statement_index: call_flow.statement_index,
        call_site,
        target_state,
    }
    .boolean_value(expression)
    .unwrap_or(false)
}

pub(super) struct ContractExpressionEvaluator<'program, 'call> {
    program: &'program omega_typed_trees::TypedTrees,
    caller_state: &'program State,
    statement_index: usize,
    call_site: &'call crate::CallSite<'program>,
    target_state: &'program State,
}

impl<'program, 'call> ContractExpressionEvaluator<'program, 'call> {
    fn boolean_value(&self, expression: ExpressionHandle) -> Option<bool> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(*value),
            ExpressionNode::Binary(binary) => match binary.operator {
                BinaryOperator::And => {
                    Some(self.boolean_value(binary.left)? && self.boolean_value(binary.right)?)
                }
                BinaryOperator::Or => {
                    Some(self.boolean_value(binary.left)? || self.boolean_value(binary.right)?)
                }
                BinaryOperator::Equal => {
                    Some(self.integer_value(binary.left)? == self.integer_value(binary.right)?)
                }
                BinaryOperator::Greater => {
                    Some(self.integer_value(binary.left)? > self.integer_value(binary.right)?)
                }
                BinaryOperator::GreaterOrEqual => {
                    Some(self.integer_value(binary.left)? >= self.integer_value(binary.right)?)
                }
                BinaryOperator::Less => {
                    Some(self.integer_value(binary.left)? < self.integer_value(binary.right)?)
                }
                BinaryOperator::LessOrEqual => {
                    Some(self.integer_value(binary.left)? <= self.integer_value(binary.right)?)
                }
                BinaryOperator::NotEqual => {
                    Some(self.integer_value(binary.left)? != self.integer_value(binary.right)?)
                }
                BinaryOperator::Add
                | BinaryOperator::Divide
                | BinaryOperator::Modulo
                | BinaryOperator::Multiply
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::Subtract => None,
            },
            ExpressionNode::Mutable(inner) => self.boolean_value(*inner),
            _ => None,
        }
    }

    fn integer_value(&self, expression: ExpressionHandle) -> Option<i64> {
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
            ExpressionNode::Integer(value) => Some(*value),
            ExpressionNode::Member(member) if member.member.as_str() == "len" => self
                .collection_length(member.receiver)
                .and_then(|length| i64::try_from(length).ok()),
            ExpressionNode::Mutable(inner) => self.integer_value(*inner),
            _ => None,
        }
    }

    fn collection_length(&self, expression: ExpressionHandle) -> Option<usize> {
        let expression = self.resolved_expression(expression).unwrap_or(expression);
        match self.program.expression_table.expression(expression) {
            ExpressionNode::ArrayLiteral(values) => Some(values.count() as usize),
            ExpressionNode::Mutable(inner) => self.collection_length(*inner),
            _ => None,
        }
    }
}
