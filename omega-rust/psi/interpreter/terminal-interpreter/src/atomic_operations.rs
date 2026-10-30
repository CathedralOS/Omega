//! Serial execution of normalized atomic access events.
//!
//! The interpreter runs one activation at a time, so an atomic event is one
//! uninterrupted observe-then-replace step on the same resolved scalar field:
//! the observed prior is exactly the resident the replacement consumed, never
//! a second read. The field resolves through the same structural-argument
//! resolution other observations use, retaining the current local or arrival
//! home and every readable stored-reference crossing. Orderings constrain concurrent observation only; a serial run
//! has no second participant, so they select no different behavior here.
//! Direct members include a mixed case's common fields: their cells live in
//! the member namespace regardless of the active payload or physical tag size.

use semantic_vocabulary::{IntegerValue, PlaceId, StructuralFieldId};
use terminal_psi::{
    AtomicAccessEvent, AtomicReadModifyWrite, Operation, OperationResult, StructuralAccess,
    StructuralPathSegment,
};

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::values::TerminalScalarValue;

impl TerminalExecution<'_> {
    pub(crate) fn execute_atomic_access(
        &mut self,
        operation: &Operation,
        place: PlaceId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        event: AtomicAccessEvent,
    ) -> Result<(), TerminalInterpretError> {
        let machine = self.program.verified_machine(self.current_machine)?;
        let root = self.structural_values.get(&place).ok_or_else(malformed)?;
        let access = terminal_semantics::arrival_parameter(machine, place)
            .filter(|parameter| parameter.structural_type == root.structural_type)
            .map(|parameter| parameter.access)
            .or_else(|| {
                let (producer, structural_type) = machine.producing_operation(place)?;
                let result = producer.result.structural()?;
                (result.place == place
                    && result.structural_type == structural_type
                    && structural_type == root.structural_type)
                    .then_some(StructuralAccess::Owned)
            })
            .ok_or_else(malformed)?;
        if access == StructuralAccess::WriteOnlyBorrow || !event.ordering_is_legal() {
            return Err(malformed());
        }
        // A final Case qualifies a payload field of the sum; it does not
        // designate another structural value. Resolve the actual record/sum
        // through each current descriptor before selecting that namespace.
        let (prefix, selected_case) = match path.split_last() {
            Some((StructuralPathSegment::Case(case), prefix)) => (prefix, Some(*case)),
            _ => (path, None),
        };
        let (parent, view) =
            self.resolve_live_view_projection(place, prefix, StructuralAccess::SharedBorrow)?;
        let declared_parent = match view {
            None => parent.structural_type,
            Some(crate::reference::AggregateViewTip::Aggregate { view, backing })
                if backing == parent.structural_type =>
            {
                view.structural_type
            }
            Some(_) => return Err(malformed()),
        };
        let namespace = selected_case
            .into_iter()
            .map(StructuralPathSegment::Case)
            .collect::<Vec<_>>();
        let types = self.program.structural_types();
        let (canonical, declaration) = terminal_semantics::static_field_projection(
            types.values(),
            declared_parent,
            &namespace,
            field,
        )
        .ok_or_else(malformed)?;
        let scalar_type = declaration
            .field_type
            .atomic_scalar_type()
            .ok_or_else(malformed)?;
        let cell = self.observed_field_cell_in(&parent, view, &canonical, field)?;
        let operand = |value| {
            self.values
                .get(&value)
                .copied()
                .filter(|operand: &TerminalScalarValue| operand.scalar_type() == scalar_type)
                .ok_or(TerminalInterpretError::VerifiedValueMissing(value))
        };
        let prior = if event.observes_resident() {
            let prior = self
                .structural_scalar_fields
                .get(&cell)
                .map(crate::scalar_storage::ScalarCell::read)
                .transpose()?
                .ok_or(TerminalInterpretError::StructuralScalarFieldMissing {
                    source: place,
                    field,
                })?;
            if prior.scalar_type() != scalar_type {
                return Err(malformed());
            }
            Some(prior)
        } else {
            None
        };
        let replacement = match event {
            AtomicAccessEvent::Load { .. } => None,
            AtomicAccessEvent::Store { value, .. } | AtomicAccessEvent::Swap { value, .. } => {
                Some(operand(value)?)
            }
            AtomicAccessEvent::ReadModifyWrite {
                operation: arithmetic,
                operand: value,
                ..
            } => Some(
                read_modify_write(arithmetic, prior.ok_or_else(malformed)?, operand(value)?)
                    .ok_or_else(malformed)?,
            ),
            AtomicAccessEvent::CompareExchange {
                expected,
                replacement,
                ..
            }
            | AtomicAccessEvent::CompareExchangeOnce {
                expected,
                replacement,
                ..
            } => {
                // Representation equality of the resident against the
                // expected value, never a user-defined equality.
                let matched = prior.ok_or_else(malformed)? == operand(expected)?;
                matched.then(|| operand(replacement)).transpose()?
            }
        };
        match (prior, &operation.result) {
            (Some(prior), OperationResult::Structural(result))
                if matches!(event, AtomicAccessEvent::CompareExchangeOnce { .. }) =>
            {
                if !result.is_unrestricted_plain()
                    || !result.qualification_establishments.is_empty()
                {
                    return Err(malformed());
                }
                let types = self.program.structural_types();
                let cases = terminal_psi::types::atomic_once_outcome_cases(
                    types.get(&result.structural_type).ok_or_else(malformed)?,
                    scalar_type,
                )
                .ok_or_else(malformed)?;
                // The serial engine has no interfering activation or failed
                // conditional-store attempt. It never invents Uncommitted.
                let index = usize::from(replacement.is_some());
                let result_case = cases[index].id;
                let fields = if index == 0 {
                    vec![(
                        vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                            cases[0].fields[0].id,
                        )],
                        prior,
                    )]
                } else {
                    Vec::new()
                };
                self.publish_scalar_aggregate(
                    result,
                    crate::values::TerminalScalarAggregateValue {
                        structural_type: result.structural_type,
                        result_case: Some(result_case),
                        fields,
                    },
                )?;
            }
            (Some(prior), OperationResult::Scalar(result)) if result.scalar_type == scalar_type => {
                if matches!(event, AtomicAccessEvent::CompareExchangeOnce { .. }) {
                    return Err(malformed());
                }
                self.values.insert(result.id, prior)?;
            }
            (None, OperationResult::Unit) => {}
            _ => return Err(malformed()),
        }
        if let Some(replacement) = replacement {
            if !terminal_scalar_belongs_to_type(replacement) {
                return Err(malformed());
            }
            self.invalidate_stored_provenance(&cell.parent);
            self.structural_scalar_fields.insert(cell, replacement)?;
        }
        Ok(())
    }
}

/// `prior op operand` at the field's integer width, wrapping like the
/// instruction does.
fn read_modify_write(
    operation: AtomicReadModifyWrite,
    prior: TerminalScalarValue,
    operand: TerminalScalarValue,
) -> Option<TerminalScalarValue> {
    let (
        TerminalScalarValue::Integer {
            scalar_type,
            value: prior,
        },
        TerminalScalarValue::Integer {
            scalar_type: operand_type,
            value: operand,
        },
    ) = (prior, operand)
    else {
        return None;
    };
    if scalar_type != operand_type || scalar_type.is_address() {
        return None;
    }
    let value: IntegerValue = match operation {
        AtomicReadModifyWrite::FetchAdd => scalar_type.wrapping_add(prior, operand),
        AtomicReadModifyWrite::FetchSub => scalar_type.wrapping_sub(prior, operand),
        AtomicReadModifyWrite::FetchAnd => scalar_type.bitwise_and(prior, operand),
        AtomicReadModifyWrite::FetchOr => scalar_type.bitwise_or(prior, operand),
        AtomicReadModifyWrite::FetchXor => scalar_type.bitwise_xor(prior, operand),
    }?;
    Some(TerminalScalarValue::Integer { scalar_type, value })
}
