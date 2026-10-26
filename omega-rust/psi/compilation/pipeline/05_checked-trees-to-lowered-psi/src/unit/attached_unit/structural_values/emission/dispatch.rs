//! A structural selection lowers as a chain of equality tests over its
//! scalar subject. Each covered arm's value is established in its own block
//! and joins one continuation block, whose parameters carry the selected
//! result place and the owners that stay live after the selection.

use super::super::super::{
    CheckedScalarExpressionRole, LoweringError, OperationKind, PlaceId, ScalarType, unsupported,
};
use super::{Emission, ValueContinuation};
use crate::emission::operation_emission::integer::LoweredIntegerComparisonKind;

impl Emission<'_, '_, '_> {
    /// Lower one `Dispatch` node authored at `expression`.
    pub(super) fn dispatch(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        value: checked_trees::CheckedStructuralValueHandle,
        expression: checked_trees::expression::ExpressionHandle,
        subject: checked_trees::CheckedDispatchSubject,
        arms: arena::HandleSpan<checked_trees::CheckedStructuralDispatchArm>,
        continuation: Option<&mut ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let completed_prefix = self.evaluation.completed_operand_scope.len();
        let result =
            self.dispatch_value(identities, value, expression, subject, arms, continuation);
        self.evaluation
            .completed_operand_scope
            .truncate(completed_prefix);
        result
    }

    fn dispatch_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        value: checked_trees::CheckedStructuralValueHandle,
        expression: checked_trees::expression::ExpressionHandle,
        subject: checked_trees::CheckedDispatchSubject,
        arms: arena::HandleSpan<checked_trees::CheckedStructuralDispatchArm>,
        continuation: Option<&mut ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let arms = self
            .checked
            .facts
            .values
            .structural_values
            .dispatch_arms
            .span(arms)
            .ok_or(LoweringError::Unsupported(
                "structural dispatch arm span is stale",
            ))?
            .to_vec();
        let source_count = self.values.len();
        match subject {
            checked_trees::CheckedDispatchSubject::Scalar(subject) => {
                let value = self.scalar(
                    identities,
                    CheckedScalarExpressionRole::StructuralValueSubject { expression },
                    subject,
                    source_count,
                )?;
                self.values.push(value);
            }
            checked_trees::CheckedDispatchSubject::Structural { value, alias, .. } => {
                let request = crate::scalar_lowering::branch_states::structural_values::ValueRequest::prepare(identities, self.checked, value, self.structural_types)?;
                self.environment.produce_value(
                    identities,
                    value,
                    request.structural_type,
                    self.evaluation,
                    self.values,
                    self.next_value,
                    self.next_block,
                    self.next_edge,
                    self.calls,
                    self.operations,
                )?;
                self.evaluation
                    .bind_completed_value_alias(value, alias, self.operations)?;
                // The actual producer above dominates every predicate and
                // selected arm. Carry that dependency into their fresh scalar
                // expansions, not every historical entry in computed_values.
                self.evaluation.completed_operand_scope.push(value);
            }
        }
        let mut owned_continuation;
        let is_root = continuation.is_none();
        let continuation = if let Some(continuation) = continuation {
            continuation
        } else {
            owned_continuation = self.root_value_continuation(
                identities,
                value,
                self.values[..source_count].to_vec(),
            )?;
            owned_continuation.claim_destinations =
                crate::emission::claim_bindings::selections::reserve(
                    self.checked,
                    self.machine,
                    self.state,
                    self.statement,
                    expression,
                    0,
                    &mut self.calls.claims,
                )?;
            &mut owned_continuation
        };
        let route_prefix = self.selection_route.len();
        for (position, arm) in arms.iter().enumerate() {
            let mut fallback = None;
            if let checked_trees::CheckedScalarDispatchPattern::Predicate(predicate) = arm.pattern {
                let predicate = self.scalar(
                    identities,
                    CheckedScalarExpressionRole::StructuralValuePattern {
                        source_arm: arm.source_arm,
                    },
                    predicate,
                    source_count,
                )?;
                if predicate.scalar_type != ScalarType::Boolean {
                    return unsupported("structural dispatch predicate is not Boolean");
                }
                if position + 1 < arms.len() {
                    self.boolean_value_split(predicate.id, &mut fallback)?;
                }
            }
            if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) = arm.pattern {
                let pattern = self.scalar(
                    identities,
                    CheckedScalarExpressionRole::StructuralValuePattern {
                        source_arm: arm.source_arm,
                    },
                    pattern,
                    source_count,
                )?;
                let subject = self.values[source_count];
                if pattern.value_type() != subject.value_type() || arm.equality_use.is_valid() {
                    return unsupported(
                        "structural dispatch comparison requires matching builtin scalar operands",
                    );
                }
                // Replay requires coverage. A final value pattern is
                // therefore the last literal Boolean alternative, not
                // permission to default to an arbitrary integer arm.
                if position + 1 < arms.len() {
                    let kind = if subject.scalar_type == ScalarType::Boolean {
                        OperationKind::BooleanEqual {
                            left: subject.id,
                            right: pattern.id,
                        }
                    } else if matches!(subject.scalar_type, ScalarType::Integer { .. }) {
                        LoweredIntegerComparisonKind::Equal.operation(subject.id, pattern.id)
                    } else {
                        return unsupported(
                            "structural dispatch needs selected floating comparison lowering",
                        );
                    };
                    self.boolean_case_split(kind, &mut fallback)?;
                }
            }
            self.values.truncate(source_count);
            // Births are attached to exact completed constructor nodes by
            // complete_construction; this route only chooses storage aliases.
            if position + 1 < arms.len() {
                self.selection_route.push(
                    crate::emission::claim_bindings::selections::SelectedDecision {
                        expression,
                        decision: position,
                        selected: true,
                    },
                );
            }
            let emitted = self.value(identities, arm.value, Some(&mut *continuation));
            self.selection_route.truncate(route_prefix + position);
            if position + 1 < arms.len() {
                self.selection_route.push(
                    crate::emission::claim_bindings::selections::SelectedDecision {
                        expression,
                        decision: position,
                        selected: false,
                    },
                );
            }
            emitted?;
            self.restore_fallback(fallback)?;
        }
        self.selection_route.truncate(route_prefix);
        if is_root {
            self.record_root_continuation(continuation)?;
        }
        Ok(continuation.place)
    }
}
