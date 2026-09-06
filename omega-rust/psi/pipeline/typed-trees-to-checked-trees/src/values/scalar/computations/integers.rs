//! Integer policy stays attached to each operand until its operation is selected.

use super::*;

pub(super) struct IntegerOperand {
    pub(super) value: CheckedScalarExpression,
    domain: ArithmeticDomain,
    computation: CheckedScalarComputationHandle,
}

impl Builder<'_, '_> {
    pub(super) fn materialize_integer(
        &mut self,
        operand: IntegerOperand,
    ) -> Option<CheckedScalarComputationHandle> {
        if operand.computation.is_valid() {
            Some(operand.computation)
        } else {
            Some(self.insert(
                scalar_expression_type(&operand.value)?,
                CheckedScalarComputationKind::Value(operand.value),
            ))
        }
    }

    fn integer_application(
        &mut self,
        expression: CheckedScalarExpression,
        domain: ArithmeticDomain,
        operands: impl IntoIterator<Item = CheckedScalarComputationHandle>,
    ) -> Option<IntegerOperand> {
        let primitive_type = scalar_expression_type(&expression)?;
        let operands = self.plans.operands.insert_many(operands);
        let computation = self.insert(
            primitive_type,
            CheckedScalarComputationKind::Apply {
                expression,
                operands,
            },
        );
        Some(IntegerOperand {
            value: parameter(0, primitive_type),
            domain,
            computation,
        })
    }

    pub(super) fn integer_operand(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<IntegerOperand> {
        if let Some((value, domain)) = lower_scalar_expression(
            self.program,
            self.operators,
            expression,
            self.parameters,
            self.parameter_types,
            self.locals,
            self.exact_integer_casts,
        ) {
            return Some(IntegerOperand {
                value,
                domain,
                computation: CheckedScalarComputationHandle::invalid(),
            });
        }
        match self.program.expression_table.expression(expression).clone() {
            ExpressionNode::Call(call) => {
                // The resolved callee owns both the result carrier and its policy.
                // A destination carrier is not evidence for either one.
                let state = self.program.machines().iter().find_map(|machine| {
                    self.program
                        .machine_states(machine)
                        .first()
                        .filter(|state| state.symbol == call.target_symbol)
                })?;
                let primitive_type = self.program.primitive_type_reference(state.return_type)?;
                if !is_integer(primitive_type) {
                    return None;
                }
                let domain = self
                    .program
                    .arithmetic_domain_for_type_reference(state.return_type);
                let computation = self.expression(expression, primitive_type)?;
                Some(IntegerOperand {
                    value: parameter(0, primitive_type),
                    domain,
                    computation,
                })
            }
            ExpressionNode::Binary(binary) if operator_is_builtin(self.operators, expression) => {
                let (mut left, mut right) = self.integer_operands(&binary)?;
                let (value, domain) = construct_integer_binary(
                    binary.operator,
                    left.value,
                    left.domain,
                    right.value,
                    right.domain,
                )?;
                let CheckedScalarExpression::IntegerBinary {
                    kind,
                    primitive_type,
                    left: landed_left,
                    right: landed_right,
                } = value
                else {
                    return None;
                };
                // The shared constructor lands anonymous literals before either
                // operand is captured. It never changes their evaluation order.
                left.value = *landed_left;
                right.value = *landed_right;
                let right_type = scalar_expression_type(&right.value)?;
                let left = self.materialize_integer(left)?;
                let right = self.materialize_integer(right)?;
                self.integer_application(
                    CheckedScalarExpression::IntegerBinary {
                        kind,
                        primitive_type,
                        left: Box::new(parameter(0, primitive_type)),
                        right: Box::new(parameter(1, right_type)),
                    },
                    domain,
                    [left, right],
                )
            }
            ExpressionNode::Unary(unary)
                if unary.operator == UnaryOperator::BitwiseNot
                    && operator_is_builtin(self.operators, expression) =>
            {
                let operand = self.integer_operand(unary.operand)?;
                let primitive_type = scalar_expression_type(&operand.value)?;
                let (value, domain) =
                    construct_integer_bitwise_not(parameter(0, primitive_type), operand.domain)?;
                let operand = self.materialize_integer(operand)?;
                self.integer_application(value, domain, [operand])
            }
            ExpressionNode::Cast(cast) => {
                let operand = self.integer_operand(cast.value)?;
                let (value, domain) = construct_integer_cast(
                    self.program,
                    expression,
                    operand.value.clone(),
                    self.exact_integer_casts,
                )?;
                if matches!(
                    value,
                    CheckedScalarExpression::IntegerWiden { .. }
                        | CheckedScalarExpression::IntegerExactCast { .. }
                ) {
                    let source_type = scalar_expression_type(&operand.value)?;
                    let (template, _) = construct_integer_cast(
                        self.program,
                        expression,
                        parameter(0, source_type),
                        self.exact_integer_casts,
                    )?;
                    let operand = self.materialize_integer(operand)?;
                    self.integer_application(template, domain, [operand])
                } else {
                    // Same-carrier qualification changes later policy, not payload.
                    Some(IntegerOperand {
                        value,
                        domain,
                        ..operand
                    })
                }
            }
            _ => None,
        }
    }

    fn integer_operands(
        &mut self,
        binary: &typed_trees::expression::TableBinaryExpression,
    ) -> Option<(IntegerOperand, IntegerOperand)> {
        let mut left = self.integer_operand(binary.left);
        let mut right = self.integer_operand(binary.right);
        // Computations are still visited in authored order. Only a wholly
        // anonymous subtree can be replaced by a landed value, so a call or
        // other already-typed computation is never discarded or reevaluated.
        let land = |expression, destination| {
            land_anonymous_scalar_expression(self.program, self.operators, expression, destination)
                .map(|value| IntegerOperand {
                    value,
                    domain: ArithmeticDomain::Exact,
                    computation: CheckedScalarComputationHandle::invalid(),
                })
        };
        if let Some(destination) = right
            .as_ref()
            .and_then(|operand| scalar_expression_type(&operand.value))
            && let Some(operand) = land(binary.left, destination)
        {
            left = Some(operand);
        }
        if let Some(destination) = left
            .as_ref()
            .and_then(|operand| scalar_expression_type(&operand.value))
            && let Some(operand) = land(binary.right, destination)
        {
            right = Some(operand);
        }
        Some((left?, right?))
    }

    pub(super) fn integer_comparison(
        &mut self,
        binary: &typed_trees::expression::TableBinaryExpression,
    ) -> Option<CheckedScalarComputationHandle> {
        let (mut left, mut right) = self.integer_operands(binary)?;
        let comparison = construct_integer_comparison(binary.operator, left.value, right.value)?;
        let comparison = match comparison {
            CheckedBooleanExpression::Not(value) => *value,
            value => value,
        };
        let CheckedBooleanExpression::IntegerComparison {
            left: landed_left,
            right: landed_right,
            ..
        } = comparison
        else {
            return None;
        };
        // Greater-than is represented by reversing the comparison's *values*,
        // never by reversing the authored computations that produce them.
        (left.value, right.value) = if matches!(
            binary.operator,
            BinaryOperator::Greater | BinaryOperator::GreaterOrEqual
        ) {
            (*landed_right, *landed_left)
        } else {
            (*landed_left, *landed_right)
        };
        let primitive_type = scalar_expression_type(&left.value)?;
        let template = construct_integer_comparison(
            binary.operator,
            parameter(0, primitive_type),
            parameter(1, primitive_type),
        )?;
        let left = self.materialize_integer(left)?;
        let right = self.materialize_integer(right)?;
        let operands = self.plans.operands.insert_many([left, right]);
        Some(self.insert(
            PrimitiveType::Bool,
            CheckedScalarComputationKind::Apply {
                expression: CheckedScalarExpression::Boolean(Box::new(template)),
                operands,
            },
        ))
    }
}

fn parameter(position: usize, primitive_type: PrimitiveType) -> CheckedScalarExpression {
    CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    }
}
