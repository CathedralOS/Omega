//! Intrinsic enum equality observes both operands before comparing their tags.
//!
//! Payload-free sums need no new runtime equality opcode: their complete case
//! roster is a Boolean encoding of the tag. Apply evaluates every left
//! observation before every right observation, then compares the captured
//! Booleans. Short-circuiting the final conjunction cannot skip an operand's
//! bounds check. Only established parameter places are admitted here; a call
//! or effectful selector cannot be replayed once for every declared case.

use super::{
    Builder, CheckedBooleanExpression, CheckedScalarComputationHandle,
    CheckedScalarComputationKind, CheckedScalarExpression, ExpressionHandle, ExpressionNode,
    PrimitiveType, operator_is_builtin,
};
use checked_trees::CheckedScalarComputationStructuralArgument;
use typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
use typed_trees::expression::BinaryOperator;

impl Builder<'_, '_> {
    pub(super) fn enum_equality(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<CheckedScalarComputationHandle> {
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) || !operator_is_builtin(self.operators, expression)
        {
            return None;
        }
        let unequal = binary.operator == BinaryOperator::NotEqual;
        let (left_type, left) = self.parameter_case_subject(binary.left, self.statement_index)?;
        let (right_type, right) =
            self.parameter_case_subject(binary.right, self.statement_index)?;
        let left_owner =
            crate::values::scalar::structural_fields::structural_data(self.program, left_type)?;
        let right_owner =
            crate::values::scalar::structural_fields::structural_data(self.program, right_type)?;
        if left_owner.symbol != right_owner.symbol || left.type_identity != right.type_identity {
            return None;
        }
        let members = self.program.data_members(left_owner);
        if DataDefinition::shape_kind_from_members(members) != DataShapeKind::Enum {
            return None;
        }
        let cases = members
            .iter()
            .map(|member| {
                let DataMember::Variant(variant) = member else {
                    return None;
                };
                self.program
                    .data_payload_fields(variant)
                    .is_empty()
                    .then_some(variant.symbol)
            })
            .collect::<Option<Vec<_>>>()?;
        if cases.is_empty() {
            return None;
        }
        let mut observations = Vec::with_capacity(cases.len().checked_mul(2)?);
        for subject in [left, right] {
            for case in &cases {
                observations.push(self.insert(
                    PrimitiveType::Bool,
                    CheckedScalarComputationKind::CaseMembership {
                        source_expression: expression,
                        subject: CheckedScalarComputationStructuralArgument::Place(subject.clone()),
                        case: *case,
                    },
                ));
            }
        }
        let comparisons = (0..cases.len()).map(|position| CheckedBooleanExpression::Equal {
            left: Box::new(CheckedBooleanExpression::Parameter { position }),
            right: Box::new(CheckedBooleanExpression::Parameter {
                position: cases.len() + position,
            }),
        });
        let mut template = comparisons.reduce(|left, right| CheckedBooleanExpression::And {
            left: Box::new(left),
            right: Box::new(right),
        })?;
        if unequal {
            template = CheckedBooleanExpression::Not(Box::new(template));
        }
        let operands = self.plans.operands.insert_many(observations);
        Some(self.insert(
            PrimitiveType::Bool,
            CheckedScalarComputationKind::Apply {
                source_expression: expression,
                expression: CheckedScalarExpression::Boolean(Box::new(template)),
                operands,
            },
        ))
    }
}
