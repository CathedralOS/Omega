//! Emit one checked atomic event as one Terminal `AtomicAccess`.
//!
//! The event borrows its containing record through the ordinary structural
//! source/path join, retaining the actual home and reference crossings. Its
//! operands are the carrier's own `AtomicOperand` rows,
//! evaluated in event order in the ordinary scalar evaluator. The operation
//! then carries the whole event: the arithmetic model the source carrier
//! spells is never emitted as a separate read, arithmetic and store, so the
//! observed prior is the one the event itself defines.
//!
//! The observed prior is the next dense scalar value, like an established
//! scalar local; the Terminal verifier independently reconstructs the leaf,
//! the root's authority, the orderings and every type.

use super::OperationFrame;
use crate::unit::{
    CheckedScalarExpressionRole, LoweringError, Operation, OperationKind, OperationResult,
    ValueDeclaration, allocate_dense, terminal_scalar_type, unsupported,
};
use checked_trees::{CheckedAtomicAccessPlan, CheckedAtomicEvent, CheckedAtomicReadModifyWrite};
use terminal_psi::{AtomicAccessEvent, AtomicReadModifyWrite};

impl OperationFrame<'_, '_> {
    pub(super) fn atomic_fence(
        &mut self,
        ordering: language_core::atomic::FenceOrdering,
    ) -> Result<(), LoweringError> {
        let id = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Unit,
            kind: OperationKind::AtomicFence { ordering },
        });
        Ok(())
    }

    pub(super) fn atomic_access(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        access: &CheckedAtomicAccessPlan,
    ) -> Result<(), LoweringError> {
        let scalar_type = terminal_scalar_type(access.primitive_type)?;
        // Borrow the current published home through the same join used by
        // ordinary observations. A stored descriptor remains in the path;
        // neither its initializer nor a same-typed parameter supplies it.
        let (_, state) = crate::declarations::state_declaration(self.checked, self.state)?;
        let bindings = self
            .evaluation
            .completed_bindings(self.operations, std::iter::empty())
            .with_structural_observations(self.structural_types.declarations());
        let receiver = bindings.shared_structural_argument(
            identities,
            self.checked,
            &access.receiver,
            self.checked.state_parameters(state),
        )?;
        let carrier =
            bindings
                .structural_argument_type(&receiver)
                .ok_or(LoweringError::Unsupported(
                    "atomic event lost its receiver type",
                ))?;
        let place = receiver.place;
        let mut path = receiver.path;
        let field_path = access
            .field_case
            .iter()
            .cloned()
            .map(checked_trees::CheckedUnitStructuralPathSegment::Case)
            .collect::<Vec<_>>();
        let (suffix, field) =
            crate::emission::structural_scalar_store::lower_structural_field_path(
                identities,
                carrier,
                &field_path,
                &access.field_identity,
                self.structural_types.declarations(),
            )?;
        path.extend(suffix);
        let resident_bounds = match field.field_type {
            terminal_psi::StructuralFieldType::AtomicBoundedInteger(bounds) => Some(bounds),
            _ => None,
        };
        let field = field.id;
        let mut operands = Vec::new();
        for (ordinal, operand) in access.event.operands().into_iter().enumerate() {
            let role = CheckedScalarExpressionRole::AtomicOperand {
                operand_ordinal: u32::try_from(ordinal).map_err(|_| {
                    LoweringError::Unsupported("atomic event operand ordinal overflow")
                })?,
            };
            let value = {
                let (mut environment, emission) = self.reborrow().split();
                emission.evaluation.source_value(
                    identities,
                    environment.checked,
                    environment.machine,
                    environment.state,
                    access.statement_index,
                    role,
                    operand,
                    emission.source_value_count,
                    emission.values,
                    emission.next_value,
                    emission.next_block,
                    emission.next_edge,
                    emission.operations,
                    emission.calls,
                    &mut environment,
                )
            }?;
            operands.push(value.id);
        }
        let operand = |ordinal: usize| {
            operands
                .get(ordinal)
                .copied()
                .ok_or(LoweringError::Unsupported(
                    "atomic event lost an evaluated operand",
                ))
        };
        let event = match &access.event {
            CheckedAtomicEvent::Load { ordering } => AtomicAccessEvent::Load {
                ordering: *ordering,
            },
            CheckedAtomicEvent::Store { ordering, .. } => AtomicAccessEvent::Store {
                value: operand(0)?,
                ordering: *ordering,
            },
            CheckedAtomicEvent::ReadModifyWrite {
                operation,
                ordering,
                ..
            } => AtomicAccessEvent::ReadModifyWrite {
                operation: match operation {
                    CheckedAtomicReadModifyWrite::FetchAdd => AtomicReadModifyWrite::FetchAdd,
                    CheckedAtomicReadModifyWrite::FetchSub => AtomicReadModifyWrite::FetchSub,
                    CheckedAtomicReadModifyWrite::FetchAnd => AtomicReadModifyWrite::FetchAnd,
                    CheckedAtomicReadModifyWrite::FetchOr => AtomicReadModifyWrite::FetchOr,
                    CheckedAtomicReadModifyWrite::FetchXor => AtomicReadModifyWrite::FetchXor,
                },
                operand: operand(0)?,
                ordering: *ordering,
            },
            CheckedAtomicEvent::Swap { ordering, .. } => AtomicAccessEvent::Swap {
                value: operand(0)?,
                ordering: *ordering,
            },
            CheckedAtomicEvent::CompareExchange {
                success, failure, ..
            } => AtomicAccessEvent::CompareExchange {
                expected: operand(0)?,
                replacement: operand(1)?,
                success: *success,
                failure: *failure,
            },
            CheckedAtomicEvent::CompareExchangeOnce {
                success, failure, ..
            } => AtomicAccessEvent::CompareExchangeOnce {
                expected: operand(0)?,
                replacement: operand(1)?,
                success: *success,
                failure: *failure,
            },
        };
        if !event.ordering_is_legal() {
            return unsupported("atomic event orderings are not source-legal");
        }
        let resident_obligation = if event.modifies_resident() && resident_bounds.is_some() {
            if event.replacement().is_none() {
                return unsupported(
                    "bounded fetch requires universal atomic resident preservation",
                );
            }
            if !crate::emission::structural_scalar_store::atomic_source_range_decided(
                self.checked,
                access.primitive_type,
                access.resident_range_decision,
                resident_bounds,
            )? {
                return unsupported(
                    "atomic resident establishment lost its exact checked range decision",
                );
            }
            let obligation = crate::unit::obligation_id(allocate_dense(
                &mut self.calls.next_obligation_identity,
            )?);
            self.operations.source_checked_obligations.push(obligation);
            Some(obligation)
        } else {
            None
        };
        if matches!(event, AtomicAccessEvent::CompareExchangeOnce { .. }) {
            let result = access
                .structural_result
                .as_ref()
                .ok_or(LoweringError::Unsupported(
                    "single-attempt event lost its structural result",
                ))?;
            if access.result.is_some() {
                return unsupported("single-attempt event cannot bind a scalar prior");
            }
            self.results
                .require_next(result, "atomic outcome result is not dense")?;
            let structural_type =
                crate::unit::lookup_type_id(self.type_ids, &result.type_identity)?;
            let (produced_place, producer) = super::allocate_structural_result(
                self.operations,
                self.next_place,
                structural_type,
                crate::terminal_identities::structural_multiplicity(result.multiplicity),
                Vec::new(),
                OperationKind::AtomicAccess {
                    place,
                    path,
                    field,
                    event,
                    resident_obligation,
                },
            )?;
            let produced = self
                .operations
                .operations
                .last()
                .and_then(|row| row.result.as_structural())
                .cloned()
                .ok_or(LoweringError::Unsupported(
                    "single-attempt event did not establish its outcome",
                ))?;
            return self.register_structural_result(
                super::operation_result_declaration(produced_place, producer, structural_type),
                result,
                produced,
            );
        }
        if access.structural_result.is_some() {
            return unsupported("scalar atomic event has a structural outcome");
        }
        let result = match (&access.result, event.observes_resident()) {
            (Some(_), true) => {
                let value = ValueDeclaration {
                    qualifications: Default::default(),
                    id: crate::unit::value_id(allocate_dense(self.next_value)?),
                    scalar_type,
                };
                self.evaluation
                    .scalar_bindings
                    .append(scalar_type, self.values.len());
                self.values.push(value);
                OperationResult::Scalar(value)
            }
            (None, false) => OperationResult::Unit,
            _ => return unsupported("atomic event result disagrees with its event"),
        };
        let id = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result,
            kind: OperationKind::AtomicAccess {
                place,
                path,
                field,
                event,
                resident_obligation,
            },
        });
        Ok(())
    }
}
