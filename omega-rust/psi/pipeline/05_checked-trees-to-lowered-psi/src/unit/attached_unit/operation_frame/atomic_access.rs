//! Emit one checked atomic event as one Terminal `AtomicAccess`.
//!
//! The event's place resolves exactly as a scalar field store's does — a
//! structural parameter, its static carrier path, and the field — and its
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
use terminal_psi::{
    AtomicAccessEvent, AtomicReadModifyWrite, StructuralAccess, StructuralFieldType,
};

impl OperationFrame<'_, '_> {
    pub(super) fn atomic_access(
        &mut self,
        access: &CheckedAtomicAccessPlan,
    ) -> Result<(), LoweringError> {
        let scalar_type = terminal_scalar_type(access.primitive_type)?;
        // Rejoin the plan to its authored carrier before emitting anything.
        let parameter = self.parameters.get(access.parameter_index as usize).ok_or(
            LoweringError::Unsupported("atomic event parameter is absent"),
        )?;
        let (_, state) = crate::expression_preparation::source_custody::authored_state(
            self.checked,
            self.state,
        )?;
        let authored_destination = self
            .checked
            .state_parameters(state)
            .get(parameter.position as usize)
            .map(|parameter| parameter.symbol)
            .ok_or(LoweringError::Unsupported(
                "atomic event parameter has no authored binding",
            ))?;
        crate::emission::atomic_sources::validate(
            self.checked,
            self.machine,
            self.state,
            authored_destination,
            access,
        )?;
        let authorized = if access.event.modifies_resident() {
            parameter.access == StructuralAccess::MutableBorrow
        } else {
            matches!(
                parameter.access,
                StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
            )
        };
        if !authorized || !parameter.qualifications.is_empty() {
            return unsupported("atomic event parameter lost its access authority");
        }
        let place = parameter.place;
        let (path, field) = crate::emission::structural_scalar_store::lower_structural_field_path(
            parameter.structural_type,
            &access.carrier_path,
            &access.field_identity,
            self.structural_types.declarations(),
        )?;
        if field.field_type != StructuralFieldType::Scalar(scalar_type) {
            return unsupported("atomic event field differs from its checked carrier");
        }
        let field = field.id;
        let mut operands = Vec::new();
        for (ordinal, operand) in access.event.operands().into_iter().enumerate() {
            let role = CheckedScalarExpressionRole::AtomicOperand {
                operand_ordinal: u32::try_from(ordinal).map_err(|_| {
                    LoweringError::Unsupported("atomic event operand ordinal overflow")
                })?,
            };
            let value = self.evaluation.source_value(
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
            if value.scalar_type != scalar_type || !value.qualifications.is_empty() {
                return unsupported("atomic event operand differs from its leaf carrier");
            }
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
            (Some(result), true) => {
                if result.primitive_type != access.primitive_type
                    || (self.evaluation.scalar_bindings.is_none()
                        && usize::try_from(result.binding_ordinal)
                            .ok()
                            .and_then(|ordinal| ordinal.checked_add(self.scalar_parameter_count))
                            != Some(self.values.len()))
                {
                    return unsupported("atomic event prior binding drifted from source order");
                }
                let value = ValueDeclaration {
                    qualifications: Default::default(),
                    id: crate::unit::value_id(allocate_dense(self.next_value)?),
                    scalar_type,
                };
                if let Some(bindings) = self.evaluation.scalar_bindings.as_mut() {
                    bindings.append(
                        checked_trees::CheckedScalarBindingDestination::Immutable,
                        scalar_type,
                        self.values.len(),
                    )?;
                }
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
