//! A structural selection lowers as a chain of equality tests over its
//! scalar subject. Each covered arm's value is established in its own block
//! and joins one continuation block, whose parameters carry the selected
//! result place and the owners that stay live after the selection.

use super::super::super::super::block_id;
use super::super::super::{
    CheckedScalarExpressionRole, LoweringError, Operation, OperationKind, PlaceId, ScalarType,
    Terminator, allocate_dense, unsupported, value_id,
};
use super::{Emission, ValueContinuation};
use crate::emission::operation_emission::buffer::scalar_result_declaration;
use crate::emission::operation_emission::integer::LoweredIntegerComparisonKind;

impl Emission<'_, '_, '_> {
    /// Lower one `Dispatch` node authored at `expression`.
    pub(super) fn dispatch(
        &mut self,
        expression: checked_trees::expression::ExpressionHandle,
        subject: checked_trees::CheckedScalarComputationHandle,
        arms: arena::HandleSpan<checked_trees::CheckedStructuralDispatchArm>,
        continuation: Option<&ValueContinuation>,
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
        let subject = self.scalar(
            CheckedScalarExpressionRole::StructuralValueSubject { expression },
            subject,
            source_count,
        )?;
        self.values.push(subject);
        let owned_continuation;
        let is_root = continuation.is_none();
        let continuation = if let Some(continuation) = continuation {
            continuation
        } else {
            let first_candidate = self.first_candidate();
            let remaining_owners = self
                .owners
                .iter()
                .enumerate()
                .filter(|(position, _)| Some(*position) != first_candidate)
                .map(|(_, owner)| owner.clone())
                .collect::<Vec<_>>();
            owned_continuation = self.joined_value_continuation(
                self.values[..source_count].to_vec(),
                remaining_owners,
            )?;
            &owned_continuation
        };
        for (position, arm) in arms.iter().enumerate() {
            let mut fallback = None;
            if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) = arm.pattern {
                let pattern = self.scalar(
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
                    let condition = value_id(allocate_dense(self.next_value)?);
                    let id = self.operations.allocate();
                    self.operations.push(Operation {
                        static_reach_binding: None,
                        suspension_crossing: None,
                        id,
                        result: scalar_result_declaration(condition, ScalarType::Boolean),
                        kind,
                    });
                    let selected = block_id(allocate_dense(self.next_block)?);
                    let next = block_id(allocate_dense(self.next_block)?);
                    let when_true = self.edge(selected, Vec::new(), Vec::new())?;
                    let when_false = self.edge(next, Vec::new(), Vec::new())?;
                    self.finish(Terminator::Conditional {
                        condition,
                        when_true,
                        when_false,
                    });
                    fallback = Some((next, self.values.clone()));
                    self.start(selected);
                }
            }
            self.values.truncate(source_count);
            self.value(arm.value, Some(continuation))?;
            self.restore_fallback(fallback);
        }
        if is_root {
            self.record_root_continuation(continuation);
        }
        Ok(continuation.place)
    }
}
