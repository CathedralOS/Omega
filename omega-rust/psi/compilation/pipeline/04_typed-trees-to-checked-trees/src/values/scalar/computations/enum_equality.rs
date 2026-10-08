//! Intrinsic enum equality observes both operands before comparing their tags.
//!
//! Payload-free sums need no new runtime equality opcode: their complete case
//! roster is a Boolean encoding of the tag. The comparison is a chain, one
//! Apply per declared case, each folding that case's two memberships into the
//! running conjunction. A single Apply over every membership kept all of them
//! live at once; a 50-case enum then forced hundreds of spill steps. Every
//! observation is still evaluated, so no bounds check is skipped, and each
//! operand's first membership comes before any later one, so an operand that
//! traps does so at the same observation as an all-left-then-right order:
//! memberships are pure reads whose only effect is that check. A payloadless
//! constructor contributes known tag bits, not a temporary owner per
//! observation; compared with a place, it reduces the equality to that place's
//! membership in the one known case. Calls and effectful selectors cannot be
//! replayed once for every declared case.

use super::{
    Builder, CheckedBooleanExpression, CheckedScalarComputationHandle,
    CheckedScalarComputationKind, CheckedScalarExpression, ExpressionHandle, PrimitiveType,
    operator_is_builtin,
};
use checked_trees::CheckedScalarComputationStructuralArgument;
use typed_trees::data::{DataDefinition, DataShapeKind};
use typed_trees::expression::BinaryOperator;

impl Builder<'_, '_> {
    pub(super) fn enum_equality(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<CheckedScalarComputationHandle> {
        let Some(binary) = self.program.expression_table.binary_node(expression) else {
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
        let (left_type, left) = self.enum_operand(binary.left)?;
        let (right_type, right) = self.enum_operand(binary.right)?;
        let left_owner =
            crate::values::scalar::structural_fields::structural_data(self.program, left_type)?;
        let right_owner =
            crate::values::scalar::structural_fields::structural_data(self.program, right_type)?;
        if left_owner.symbol != right_owner.symbol
            || self.program.normalized_type_identity(left_type)
                != self.program.normalized_type_identity(right_type)
        {
            return None;
        }
        let members = self.program.data_members(left_owner);
        if DataDefinition::shape_kind_from_members(members) != DataShapeKind::Enum {
            return None;
        }
        let cases = members
            .iter()
            .map(|member| {
                let variant = member.as_variant()?;
                self.program
                    .data_payload_fields(variant)
                    .is_empty()
                    .then_some(variant.symbol)
            })
            .collect::<Option<Vec<_>>>()?;
        if cases.is_empty() {
            return None;
        }
        // Equality with one known payload-free case is that case's
        // membership: a single tag observation of the other operand answers
        // it. Dispatch coverage reads memberships, so this keeps an
        // exhaustive arm set over a projected field (`transition item.kind`)
        // provably closed without a `_` arm.
        let known_case = match (&left, &right) {
            (subject, CheckedScalarComputationStructuralArgument::Case(constructor))
            | (CheckedScalarComputationStructuralArgument::Case(constructor), subject)
                if !matches!(subject, CheckedScalarComputationStructuralArgument::Case(_)) =>
            {
                Some((subject.clone(), constructor.case))
            }
            _ => None,
        };
        if let Some((subject, case)) = known_case {
            let membership = self.insert(
                PrimitiveType::Bool,
                CheckedScalarComputationKind::CaseMembership {
                    source_expression: expression,
                    subject,
                    case,
                },
            );
            if !unequal {
                return Some(membership);
            }
            let operands = self.plans.operands.insert_many([membership]);
            return Some(self.insert(
                PrimitiveType::Bool,
                CheckedScalarComputationKind::Apply {
                    source_expression: expression,
                    expression: CheckedScalarExpression::Boolean(Box::new(
                        CheckedBooleanExpression::Not(Box::new(
                            CheckedBooleanExpression::Parameter { position: 0 },
                        )),
                    )),
                    operands,
                },
            ));
        }
        let mut chain = None;
        for case in &cases {
            let left_membership = self.membership_of(&left, *case, expression);
            let right_membership = self.membership_of(&right, *case, expression);
            // The running conjunction leads each link's operands, so the
            // links evaluate in declared case order, left before right.
            let same = |left, right| CheckedBooleanExpression::Equal {
                left: Box::new(CheckedBooleanExpression::Parameter { position: left }),
                right: Box::new(CheckedBooleanExpression::Parameter { position: right }),
            };
            let (template, operands) = match chain {
                None => (same(0, 1), vec![left_membership, right_membership]),
                Some(previous) => (
                    CheckedBooleanExpression::And {
                        left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
                        right: Box::new(same(1, 2)),
                    },
                    vec![previous, left_membership, right_membership],
                ),
            };
            let operands = self.plans.operands.insert_many(operands);
            chain = Some(self.insert(
                PrimitiveType::Bool,
                CheckedScalarComputationKind::Apply {
                    source_expression: expression,
                    expression: CheckedScalarExpression::Boolean(Box::new(template)),
                    operands,
                },
            ));
        }
        let equal = chain?;
        if !unequal {
            return Some(equal);
        }
        let operands = self.plans.operands.insert_many([equal]);
        Some(self.insert(
            PrimitiveType::Bool,
            CheckedScalarComputationKind::Apply {
                source_expression: expression,
                expression: CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::Not(Box::new(CheckedBooleanExpression::Parameter {
                        position: 0,
                    })),
                )),
                operands,
            },
        ))
    }

    /// One operand's membership in `case`: a constant for a payloadless
    /// constructor, a tag observation for a place.
    fn membership_of(
        &mut self,
        subject: &CheckedScalarComputationStructuralArgument,
        case: symbols::SymbolHandle,
        expression: ExpressionHandle,
    ) -> CheckedScalarComputationHandle {
        match subject {
            CheckedScalarComputationStructuralArgument::Case(constructor) => {
                self.boolean(constructor.case == case)
            }
            _ => self.insert(
                PrimitiveType::Bool,
                CheckedScalarComputationKind::CaseMembership {
                    source_expression: expression,
                    subject: subject.clone(),
                    case,
                },
            ),
        }
    }

    fn enum_operand(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<(
        typed_trees::types::TypeReferenceHandle,
        CheckedScalarComputationStructuralArgument,
    )> {
        if let Some((reference, place)) =
            self.readable_case_subject(expression, self.statement_index)
        {
            return Some((
                reference,
                CheckedScalarComputationStructuralArgument::Place(place),
            ));
        }
        let constructor = self.case_construction(expression)?;
        // A payloadless constructor supplies a constant tag without evaluating
        // payloads. Calls and constructor payload evaluations are not tag constants.
        if !constructor.fields.is_empty() {
            return None;
        }
        Some((
            constructor.type_reference,
            CheckedScalarComputationStructuralArgument::Case(constructor),
        ))
    }
}
