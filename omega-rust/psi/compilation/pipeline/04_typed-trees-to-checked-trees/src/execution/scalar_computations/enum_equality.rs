//! Intrinsic enum equality completes each operand once, left before right.
//!
//! Payload-free sums compare their complete case roster with ordinary Boolean
//! operations. The general path owns one explicit mixed evaluation prefix:
//! copyable operands complete through the ordinary value producer, while a
//! non-copy place contributes scalar tag observations without copying its owner.
//! Readable subjects already contain only places and captured immutable selectors.
//! The generated comparison then reads those snapshots without replaying calls,
//! observing a replaced source, or granting case membership to either payload.
//!
//! One Apply per case keeps the comparison's running conjunction short-lived.
//! Copyable enums retain two value homes rather than every tag Boolean at once;
//! non-copy carriers retain their tag snapshots through the comparison. A known
//! payload-free constructor remains the existing single-membership optimization.
//! Only the whole comparison owns its source normal-completion occurrence.

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
        let (left_type, mut left, mut left_receiver) = self.enum_operand(binary.left)?;
        let (right_type, mut right, mut right_receiver) = self.enum_operand(binary.right)?;
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
                if !matches!(subject, CheckedScalarComputationStructuralArgument::Case(_)) =>
            {
                Some((subject.clone(), left_receiver, constructor.case))
            }
            (CheckedScalarComputationStructuralArgument::Case(constructor), subject)
                if !matches!(subject, CheckedScalarComputationStructuralArgument::Case(_)) =>
            {
                Some((subject.clone(), right_receiver, constructor.case))
            }
            _ => None,
        };
        if let Some((subject, receiver_value, case)) = known_case {
            let membership = self.publish_case_membership(
                expression,
                if unequal {
                    ExpressionHandle::invalid()
                } else {
                    expression
                },
                receiver_value,
                subject,
                case,
            );
            if !unequal {
                return Some(membership);
            }
            return Some(self.boolean_negation(membership, expression));
        }
        let mut evaluation = Vec::new();
        self.capture_enum_operand(binary.left, left_type, &mut left, &mut left_receiver)?;
        self.capture_enum_operand(binary.right, right_type, &mut right, &mut right_receiver)?;
        let generated = ExpressionHandle::invalid();
        let mut memberships = Vec::new();
        for (subject, receiver) in [(&left, left_receiver), (&right, right_receiver)] {
            if receiver.is_valid() {
                evaluation.push(checked_trees::CheckedScalarEvaluationOperand::Structural(
                    receiver,
                ));
            }
            let snapshot_tags = !receiver.is_valid()
                && matches!(
                    subject,
                    CheckedScalarComputationStructuralArgument::Place(_)
                );
            let mut observed = Vec::new();
            for case in &cases {
                let membership = self.membership_of(subject, receiver, *case, generated);
                // A non-copy place still supplies scalar tag observations. No
                // whole-value copy or ownership of that carrier is introduced.
                if snapshot_tags {
                    evaluation.push(checked_trees::CheckedScalarEvaluationOperand::Scalar(
                        membership,
                    ));
                }
                observed.push(membership);
            }
            memberships.push(observed);
        }
        let mut chain = None;
        for (ordinal, _) in cases.iter().enumerate() {
            let left_membership = memberships[0][ordinal];
            let right_membership = memberships[1][ordinal];
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
                    source_expression: generated,
                    expression: CheckedScalarExpression::Boolean(Box::new(template)),
                    operands,
                },
            ));
        }
        let equal = chain?;
        let operand = if unequal {
            self.boolean_negation(equal, generated)
        } else {
            equal
        };
        let operands = self.plans.evaluation_operands.insert_many(evaluation);
        Some(self.insert(
            PrimitiveType::Bool,
            CheckedScalarComputationKind::WithOperands {
                source_expression: expression,
                operands,
                operand,
            },
        ))
    }

    /// Ordinary readable copies use the same value producer as constructor
    /// children and call operands. Retaining the actual leaf once is essential
    /// even when its root was already resident before this comparison.
    fn capture_enum_operand(
        &mut self,
        expression: ExpressionHandle,
        reference: typed_trees::types::TypeReferenceHandle,
        subject: &mut CheckedScalarComputationStructuralArgument,
        receiver: &mut checked_trees::CheckedStructuralValueHandle,
    ) -> Option<()> {
        if receiver.is_valid()
            || matches!(subject, CheckedScalarComputationStructuralArgument::Case(_))
        {
            return Some(());
        }
        if self.program.type_multiplicity(reference)
            != language_semantics::Multiplicity::Unrestricted
        {
            // Borrowing a tag does not license copying its affine carrier.
            return Some(());
        }
        let value = self.retain_copied_place_value(expression, reference)?;
        *receiver = value;
        *subject = CheckedScalarComputationStructuralArgument::Place(
            checked_trees::CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                source:
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        expression,
                    },
                path: Vec::new(),
                type_identity: self.program.normalized_type_identity(reference),
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            },
        );
        Some(())
    }

    /// One operand's membership in `case`: a constant for a payloadless
    /// constructor, a tag observation for a place.
    fn membership_of(
        &mut self,
        subject: &CheckedScalarComputationStructuralArgument,
        receiver_value: checked_trees::CheckedStructuralValueHandle,
        case: symbols::SymbolHandle,
        expression: ExpressionHandle,
    ) -> CheckedScalarComputationHandle {
        match subject {
            CheckedScalarComputationStructuralArgument::Case(constructor) => {
                self.boolean(constructor.case == case)
            }
            _ => self.publish_case_membership(
                expression,
                expression,
                receiver_value,
                subject.clone(),
                case,
            ),
        }
    }

    /// A completed value supplies the tag just as an established place does.
    /// Selection and evaluation remain on the ordinary structural-value graph;
    /// observing its tag grants neither payload membership nor ownership.
    pub(super) fn computed_case_membership(
        &mut self,
        source_expression: ExpressionHandle,
        expression: ExpressionHandle,
        case: symbols::SymbolHandle,
    ) -> Option<CheckedScalarComputationHandle> {
        let (returned, subject, receiver_value) = self.completed_case_subject(expression)?;
        let owner =
            crate::values::scalar::structural_fields::structural_data(self.program, returned)?;
        if !self.program.data_members(owner).iter().any(|member| {
            member
                .as_variant()
                .is_some_and(|variant| variant.symbol == case)
        }) {
            return None;
        }
        Some(self.publish_case_membership(
            source_expression,
            source_expression,
            receiver_value,
            subject,
            case,
        ))
    }

    fn completed_case_subject(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<(
        typed_trees::types::TypeReferenceHandle,
        CheckedScalarComputationStructuralArgument,
        checked_trees::CheckedStructuralValueHandle,
    )> {
        let reference = validation::expression_type_reference_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )?;
        // Observation borrows the completed owner. Its usage multiplicity does
        // not decide whether that tag can be read. The contextual source-death
        // owner checks the actual remaining claims and nominal disposition;
        // emission requires that exact completion before an owned receiver dies.
        let receiver = self.retain_structural_argument(expression, reference)?;
        Some((reference, CheckedScalarComputationStructuralArgument::Place(
            checked_trees::CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression },
                path: Vec::new(),
                type_identity: self.program.normalized_type_identity(reference),
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            }
        ), receiver))
    }

    fn enum_operand(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<(
        typed_trees::types::TypeReferenceHandle,
        CheckedScalarComputationStructuralArgument,
        checked_trees::CheckedStructuralValueHandle,
    )> {
        if let Some((reference, place)) =
            self.readable_case_subject(expression, self.statement_index)
        {
            return Some((
                reference,
                CheckedScalarComputationStructuralArgument::Place(place),
                checked_trees::CheckedStructuralValueHandle::invalid(),
            ));
        }
        if let Some(constructor) = self.case_construction(expression)
            && constructor.fields.is_empty()
        {
            return Some((
                constructor.type_reference,
                CheckedScalarComputationStructuralArgument::Case(constructor),
                checked_trees::CheckedStructuralValueHandle::invalid(),
            ));
        }
        self.completed_case_subject(expression)
    }
}
