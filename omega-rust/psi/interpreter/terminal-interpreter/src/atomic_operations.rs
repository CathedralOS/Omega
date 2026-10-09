//! Serial execution of normalized atomic access events.
//!
//! The interpreter runs one activation at a time, so an atomic event is one
//! uninterrupted observe-then-replace step on the same resolved scalar field:
//! the observed prior is exactly the resident the replacement consumed, never
//! a second read. The field resolves through the same structural-argument
//! resolution a `StructuralScalarFieldStore` uses, under the root parameter's
//! own access. Orderings constrain concurrent observation only; a serial run
//! has no second participant, so they select no different behavior here.
//! Direct members include a mixed case's common fields: their cells live in
//! the member namespace regardless of the active payload or physical tag size.

use semantic_vocabulary::{IntegerValue, PlaceId, StructuralFieldId};
use terminal_psi::{
    AtomicAccessEvent, AtomicReadModifyWrite, Operation, OperationResult, StructuralAccess,
    StructuralMultiplicity, StructuralPathSegment,
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
        machine
            .structural_parameter(place)
            .filter(|parameter| {
                let readable = matches!(
                    parameter.access,
                    StructuralAccess::Owned
                        | StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                );
                let writable = matches!(
                    parameter.access,
                    StructuralAccess::Owned
                        | StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                );
                (!event.observes_resident() || readable)
                    && (!event.modifies_resident() || writable)
                    && matches!(
                        parameter.multiplicity,
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    )
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            })
            .ok_or_else(malformed)?;
        let root = self.structural_values.get(&place).ok_or_else(malformed)?;
        let (canonical, declaration) = terminal_semantics::static_field_projection(
            self.program.structural_types().values(),
            root.structural_type,
            path,
            field,
        )
        .ok_or_else(malformed)?;
        let scalar_type = declaration
            .field_type
            .atomic_scalar_type()
            .ok_or_else(malformed)?;
        // Preserve the original overlay cell, checking each live discriminator
        // before erasing Case from runtime storage geometry.
        super::structural_inputs::case_membership::validate_case_projection(
            &self.program.structural_types(),
            &self.structural_values,
            &self.structural_cases,
            place,
            path,
        )?;
        let cell = self.observed_field_cell(place, &canonical, field)?;
        let operand = |value| {
            self.values
                .get(&value)
                .copied()
                .filter(|operand: &TerminalScalarValue| operand.scalar_type() == scalar_type)
                .ok_or(TerminalInterpretError::VerifiedValueMissing(value))
        };
        let prior = if event.observes_resident() {
            let prior = self.structural_scalar_fields.get(&cell).copied().ok_or(
                TerminalInterpretError::StructuralScalarFieldMissing {
                    source: place,
                    field,
                },
            )?;
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
            } => {
                // Representation equality of the resident against the
                // expected value, never a user-defined equality.
                let matched = prior.ok_or_else(malformed)? == operand(expected)?;
                matched.then(|| operand(replacement)).transpose()?
            }
        };
        match (prior, &operation.result) {
            (Some(prior), OperationResult::Scalar(result)) if result.scalar_type == scalar_type => {
                self.values.insert(result.id, prior);
            }
            (None, OperationResult::Unit) => {}
            _ => return Err(malformed()),
        }
        if let Some(replacement) = replacement {
            if !terminal_scalar_belongs_to_type(replacement) {
                return Err(malformed());
            }
            self.structural_scalar_fields.insert(cell, replacement);
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
