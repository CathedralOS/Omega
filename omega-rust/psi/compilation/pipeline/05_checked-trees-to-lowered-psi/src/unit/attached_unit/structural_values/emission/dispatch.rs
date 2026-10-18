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
        expression: checked_trees::expression::ExpressionHandle,
        subject: checked_trees::CheckedScalarComputationHandle,
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
        let subject = self.scalar(
            identities,
            CheckedScalarExpressionRole::StructuralValueSubject { expression },
            subject,
            source_count,
        )?;
        self.values.push(subject);
        let mut owned_continuation;
        let is_root = continuation.is_none();
        let continuation = if let Some(continuation) = continuation {
            continuation
        } else {
            owned_continuation =
                self.root_value_continuation(self.values[..source_count].to_vec())?;
            &mut owned_continuation
        };
        for (position, arm) in arms.iter().enumerate() {
            let mut fallback = None;
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
            self.value(identities, arm.value, Some(&mut *continuation))?;
            self.restore_fallback(fallback);
        }
        if is_root {
            self.record_root_continuation(continuation);
        }
        Ok(continuation.place)
    }
}
