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
        let field = field.id;
        let mut operands = Vec::new();
        for (ordinal, operand) in access.event.operands().into_iter().enumerate() {
            let role = CheckedScalarExpressionRole::AtomicOperand {
                operand_ordinal: u32::try_from(ordinal).map_err(|_| {
                    LoweringError::Unsupported("atomic event operand ordinal overflow")
                })?,
            };
            let value = self.evaluation.source_value(
                identities,
                self.checked,
                self.machine,
                self.state,
                access.statement_index,
                role,
                operand,
                self.source_value_count,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
                self.calls,
            )?;
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
        };
        if !event.ordering_is_legal() {
            return unsupported("atomic event orderings are not source-legal");
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
            },
        });
        Ok(())
    }
}
