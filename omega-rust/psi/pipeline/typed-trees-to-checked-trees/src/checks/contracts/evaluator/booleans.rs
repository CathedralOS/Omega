use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};

use super::{BooleanExpressionOwner, ContractExpressionEvaluator};

impl ContractExpressionEvaluator<'_, '_> {
    pub(super) fn boolean_value(&self, expression: ExpressionHandle) -> Option<bool> {
        self.boolean_value_in_owner(expression, BooleanExpressionOwner::Contract)
    }

    fn boolean_value_in_owner(
        &self,
        expression: ExpressionHandle,
        owner: BooleanExpressionOwner,
    ) -> Option<bool> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(*value),
            ExpressionNode::Binary(binary) => match binary.operator {
                BinaryOperator::And => Some(
                    self.boolean_value_in_owner(binary.left, owner)?
                        && self.boolean_value_in_owner(binary.right, owner)?,
                ),
                BinaryOperator::Or => Some(
                    self.boolean_value_in_owner(binary.left, owner)?
                        || self.boolean_value_in_owner(binary.right, owner)?,
                ),
                BinaryOperator::Equal => {
                    if let Some(value) = self.checked_boolean_comparison_value(expression, owner) {
                        Some(value)
                    } else if let (Some(left), Some(right)) = (
                        self.integer_value(binary.left),
                        self.integer_value(binary.right),
                    ) {
                        Some(left == right)
                    } else {
                        let (left_min, left_max) = self.integer_bounds(binary.left)?;
                        let (right_min, right_max) = self.integer_bounds(binary.right)?;
                        (left_min == left_max && right_min == right_max && left_min == right_min)
                            .then_some(true)
                    }
                }
                BinaryOperator::Greater => {
                    if let (Some(left), Some(right)) = (
                        self.integer_value(binary.left),
                        self.integer_value(binary.right),
                    ) {
                        Some(left > right)
                    } else {
                        let (left_min, _) = self.integer_bounds(binary.left)?;
                        let (_, right_max) = self.integer_bounds(binary.right)?;
                        (left_min > right_max).then_some(true)
                    }
                }
                BinaryOperator::GreaterOrEqual => {
                    if let (Some(left), Some(right)) = (
                        self.integer_value(binary.left),
                        self.integer_value(binary.right),
                    ) {
                        Some(left >= right)
                    } else {
                        let (left_min, _) = self.integer_bounds(binary.left)?;
                        let (_, right_max) = self.integer_bounds(binary.right)?;
                        (left_min >= right_max).then_some(true)
                    }
                }
                BinaryOperator::Less => {
                    if let (Some(left), Some(right)) = (
                        self.integer_value(binary.left),
                        self.integer_value(binary.right),
                    ) {
                        Some(left < right)
                    } else {
                        let (_, left_max) = self.integer_bounds(binary.left)?;
                        let (right_min, _) = self.integer_bounds(binary.right)?;
                        (left_max < right_min).then_some(true)
                    }
                }
                BinaryOperator::LessOrEqual => {
                    if let (Some(left), Some(right)) = (
                        self.integer_value(binary.left),
                        self.integer_value(binary.right),
                    ) {
                        Some(left <= right)
                    } else {
                        let (_, left_max) = self.integer_bounds(binary.left)?;
                        let (right_min, _) = self.integer_bounds(binary.right)?;
                        (left_max <= right_min).then_some(true)
                    }
                }
                BinaryOperator::NotEqual => {
                    if let Some(value) = self.checked_boolean_comparison_value(expression, owner) {
                        Some(value)
                    } else if let (Some(left), Some(right)) = (
                        self.integer_value(binary.left),
                        self.integer_value(binary.right),
                    ) {
                        Some(left != right)
                    } else {
                        let (left_min, left_max) = self.integer_bounds(binary.left)?;
                        let (right_min, right_max) = self.integer_bounds(binary.right)?;
                        (left_max < right_min || right_max < left_min).then_some(true)
                    }
                }
                BinaryOperator::Add
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
                | BinaryOperator::Divide
                | BinaryOperator::Modulo
                | BinaryOperator::Multiply
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::Subtract => None,
            },
            ExpressionNode::Name(_) => {
                if self.operators.is_some() {
                    let (resolved, resolved_owner) =
                        self.resolved_boolean_expression(expression, owner)?;
                    return self.boolean_value_in_owner(resolved, resolved_owner);
                }
                let resolved = self.resolved_expression(expression)?;
                (resolved != expression)
                    .then(|| self.boolean_value(resolved))
                    .flatten()
            }
            ExpressionNode::Borrow(inner) => self.boolean_value_in_owner(inner.target, owner),
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                Some(!self.boolean_value_in_owner(unary.operand, owner)?)
            }
            _ => None,
        }
    }

    fn checked_boolean_comparison_value(
        &self,
        expression: ExpressionHandle,
        owner: BooleanExpressionOwner,
    ) -> Option<bool> {
        Self::guarding_cycles(&self.active_evaluations, expression, || {
            self.checked_boolean_comparison_value_without_cycle(expression, owner)
        })
    }

    fn checked_boolean_comparison_value_without_cycle(
        &self,
        expression: ExpressionHandle,
        owner: BooleanExpressionOwner,
    ) -> Option<bool> {
        let operators = self.operators?;
        if !crate::checks::contracts::prover::has_builtin_operators(
            self.program,
            operators,
            expression,
        ) || !self.builtin_boolean_meaning(expression, owner)
        {
            return None;
        }
        if let Some(value) = crate::checks::contracts::prover::closed_boolean_value(
            self.program,
            operators,
            expression,
        ) {
            return Some(value);
        }
        let value = crate::checks::contracts::prover::evaluate_scalar(
            self.program,
            expression,
            &mut |leaf| {
                let (resolved, resolved_owner) = self.resolved_boolean_expression(leaf, owner)?;
                self.boolean_value_in_owner(resolved, resolved_owner)
                    .map(crate::checks::contracts::prover::ScalarValue::Boolean)
            },
        )?;
        let crate::checks::contracts::prover::ScalarValue::Boolean(value) = value else {
            return None;
        };
        Some(value)
    }

    fn builtin_boolean_meaning(
        &self,
        expression: ExpressionHandle,
        owner: BooleanExpressionOwner,
    ) -> bool {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(_) => true,
            ExpressionNode::Name(path) => {
                path.symbol.is_valid()
                    && path.head_symbol == path.symbol
                    && self
                        .program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        == 1
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                self.builtin_boolean_meaning(unary.operand, owner)
            }
            ExpressionNode::Binary(binary) => {
                let selected_builtin = match binary.operator {
                    BinaryOperator::And | BinaryOperator::Or => true,
                    BinaryOperator::Equal | BinaryOperator::NotEqual => {
                        let Some(left_type) = self.boolean_operand_type(binary.left, owner) else {
                            return false;
                        };
                        let Some(right_type) = self.boolean_operand_type(binary.right, owner)
                        else {
                            return false;
                        };
                        let owner_symbol = match owner {
                            BooleanExpressionOwner::Caller => self.caller_machine.symbol,
                            BooleanExpressionOwner::Contract => {
                                let Some(target_machine) =
                                    self.program.machines().iter().find(|machine| {
                                        self.program
                                            .machine_states(machine)
                                            .iter()
                                            .any(|state| state.symbol == self.target_symbol)
                                    })
                                else {
                                    return false;
                                };
                                target_machine.symbol
                            }
                        };
                        typed_trees::operator::has_builtin_spelled_expression_meaning(
                            self.program,
                            owner_symbol,
                            expression,
                            if binary.operator == BinaryOperator::Equal {
                                language_core::OperatorSpelling::Equal
                            } else {
                                language_core::OperatorSpelling::NotEqual
                            },
                            &[Some(left_type), Some(right_type)],
                        )
                    }
                    _ => false,
                };
                selected_builtin
                    && self.builtin_boolean_meaning(binary.left, owner)
                    && self.builtin_boolean_meaning(binary.right, owner)
            }
            _ => false,
        }
    }

    fn boolean_operand_type(
        &self,
        expression: ExpressionHandle,
        owner: BooleanExpressionOwner,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        use typed_trees::types::{PrimitiveType, TypeReferenceNode};
        let reference = match self.program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                if !path.symbol.is_valid()
                    || path.head_symbol != path.symbol
                    || self
                        .program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        != 1
                {
                    return None;
                }
                let parameters = match owner {
                    BooleanExpressionOwner::Contract => self.target_parameters,
                    BooleanExpressionOwner::Caller => {
                        self.program.state_parameters(self.caller_state)
                    }
                };
                parameters
                    .iter()
                    .find(|parameter| parameter.symbol == path.symbol)
                    .map(|parameter| parameter.type_reference)
                    .or_else(|| match owner {
                        BooleanExpressionOwner::Contract => None,
                        BooleanExpressionOwner::Caller => self
                            .program
                            .statement_table
                            .statements(self.caller_state.statement_nodes)
                            .iter()
                            .take(self.statement_index)
                            .find_map(|statement| {
                                let typed_trees::statement::StatementNode::LocalData(local) =
                                    statement
                                else {
                                    return None;
                                };
                                (local.symbol == path.symbol).then_some(local.type_reference)
                            }),
                    })?
            }
            ExpressionNode::Boolean(_) | ExpressionNode::Unary(_) | ExpressionNode::Binary(_) => {
                // Literals and builtin Boolean results have plain Bool type,
                // not a parameter's qualified domain. Reuse the invocation's
                // retained Bool carrier rather than a fabricated type handle.
                let mut reference = self
                    .target_parameters
                    .iter()
                    .chain(self.program.state_parameters(self.caller_state))
                    .find(|parameter| {
                        self.program
                            .primitive_type_reference(parameter.type_reference)
                            == Some(PrimitiveType::Bool)
                    })?
                    .type_reference;
                while let TypeReferenceNode::Constrained { base_type, .. } =
                    self.program.type_reference_table.type_reference(reference)
                {
                    reference = *base_type;
                }
                reference
            }
            _ => return None,
        };
        (self.program.primitive_type_reference(reference) == Some(PrimitiveType::Bool))
            .then_some(reference)
    }
}
