//! A structural selection lowers as a chain of equality tests over its
//! scalar subject. Each covered arm's value is established in its own block
//! and joins one continuation block, whose parameters carry the selected
//! result place and the owners that stay live after the selection.

use super::super::super::super::{StructuralAccess, StructuralParameterDeclaration, block_id};
use super::super::super::{
    CheckedScalarExpressionRole, LoweringError, Operation, OperationKind, OperationResult, PlaceId,
    ScalarType, StructuralPlaceDeclaration, StructuralPlaceKind, Terminator, ValueDeclaration,
    allocate_dense, argument_evaluation, place_id, unsupported, value_id,
};
use super::{Emission, ValueContinuation};
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
            let join = block_id(allocate_dense(self.next_block)?);
            let joined_values = self.values[..source_count]
                .iter()
                .map(|value| {
                    Ok(ValueDeclaration {
                        id: value_id(allocate_dense(self.next_value)?),
                        ..*value
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let mut structural_parameters = Vec::new();
            let first_candidate = self.first_candidate();
            let mut remaining_owners = self
                .owners
                .iter()
                .enumerate()
                .filter(|(position, _)| Some(*position) != first_candidate)
                .map(|(_, owner)| owner.clone())
                .collect::<Vec<_>>();
            let mut pass_through = Vec::new();
            let mut residuals = Vec::new();
            for position in 0..remaining_owners.len() + 1 {
                let (structural_type, multiplicity) = remaining_owners
                    .get(position)
                    .map_or((self.structural_type, self.multiplicity), |owner| {
                        (owner.value.structural_type, owner.value.multiplicity)
                    });
                let is_result_slot = position == remaining_owners.len();
                let owner = remaining_owners.get_mut(position);
                let position = u32::try_from(position).map_err(|_| {
                    LoweringError::Unsupported("owned selection parameter count exceeds u32")
                })?;
                let place = place_id(allocate_dense(self.next_place)?);
                if let Some(owner) = owner {
                    if self
                        .sources
                        .iter()
                        .any(|source| source.place == owner.value.place)
                    {
                        owner.symbol = symbols::SymbolHandle::invalid();
                        residuals.push(place);
                    } else {
                        pass_through.push((owner.value.place, place));
                    }
                    owner.value.place = place;
                }
                self.temporary_places.push(StructuralPlaceDeclaration {
                    id: place,
                    kind: StructuralPlaceKind::BlockParameter {
                        block: join,
                        position,
                    },
                });
                structural_parameters.push(StructuralParameterDeclaration {
                    place,
                    position,
                    is_self: false,
                    structural_type,
                    multiplicity,
                    // Owner slots keep owned custody; the result slot
                    // carries the result's own access (`SharedBorrow`
                    // for a `&T` branch join, `Owned` otherwise).
                    access: if is_result_slot {
                        self.result_access
                    } else {
                        StructuralAccess::Owned
                    },
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                });
            }
            let place = structural_parameters
                .last()
                .ok_or(LoweringError::Unsupported(
                    "structural continuation has no result",
                ))?
                .place;
            owned_continuation = ValueContinuation {
                block: join,
                parameters: joined_values,
                structural_parameters,
                place,
                remaining_owners,
                pass_through,
                residuals,
            };
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
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: condition,
                            scalar_type: ScalarType::Boolean,
                            qualifications: Default::default(),
                        }),
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
            if let Some((next, values)) = fallback {
                self.start(next);
                *self.values = values;
            }
        }
        if is_root {
            self.start(continuation.block);
            *self.values = continuation.parameters.clone();
            self.evaluation.parameters = continuation.parameters.clone();
            self.evaluation.block_structural_parameters =
                continuation.structural_parameters.clone();
            if !self.sources.is_empty() {
                self.evaluation
                    .selection_cleanups
                    .push(argument_evaluation::SelectionCleanup {
                        selected: continuation.place,
                        sources: self
                            .sources
                            .iter()
                            .rev()
                            .map(|source| source.place)
                            .collect(),
                        remaining: continuation.residuals.iter().rev().copied().collect(),
                        pass_through: continuation.pass_through.clone(),
                        next_operation: self.operations.next_identity,
                    });
                self.evaluation.structural_value_owners = continuation.remaining_owners.clone();
                self.evaluation.structural_locals.retain(|(_, argument)| {
                    !self
                        .sources
                        .iter()
                        .any(|source| source.place == argument.place)
                });
                for (_, argument) in &mut self.evaluation.structural_locals {
                    if let Some((_, target)) = continuation
                        .pass_through
                        .iter()
                        .find(|(source, _)| *source == argument.place)
                    {
                        argument.place = *target;
                    }
                }
                self.rebind_local_cases()?;
            }
        }
        Ok(continuation.place)
    }
}
