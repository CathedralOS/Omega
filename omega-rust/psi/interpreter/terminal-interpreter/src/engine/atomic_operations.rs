//! Serial execution of normalized atomic access events.
//!
//! The interpreter runs one activation at a time, so an atomic event is one
//! uninterrupted observe-then-replace step on the same resolved atomic cell:
//! the observed prior is exactly the resident the replacement consumed, never
//! a second read. The endpoint resolves through the same structural-argument
//! resolution other observations use, retaining the current local or arrival
//! home and every readable stored-reference crossing. Orderings constrain concurrent observation only; a serial run
//! has no second participant, so they select no different behavior here.
//! Direct members include a mixed case's common fields: their cells live in
//! the member namespace regardless of the active payload or physical tag size.

use semantic_vocabulary::{IntegerValue, PlaceId};
use terminal_psi::{
    AtomicAccessEvent, AtomicReadModifyWrite, Operation, OperationResult, StructuralAccess,
    StructuralPathSegment,
};

use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::values::TerminalScalarValue;
use crate::engine::execution::TerminalExecution;
use crate::storage::primitive_storage::{PrimitiveStorageWrites, scalar_field_projection};

impl TerminalExecution<'_> {
    pub(crate) fn execute_atomic_access(
        &mut self,
        operation: &Operation,
        place: PlaceId,
        path: &[StructuralPathSegment],
        event: AtomicAccessEvent,
    ) -> Result<(), TerminalInterpretError> {
        let machine = self.program.verified_machine(self.current_machine)?;
        let root = self.structural_value(place)?;
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
        // Resolve reference crossings before choosing the final scalar cell.
        // A Case belongs to its field namespace, rather than a new value home.
        let (prefix, endpoint) = scalar_field_projection(path).unwrap_or((path, &[]));
        let (parent, view) =
            self.resolve_live_view_projection(place, prefix, StructuralAccess::SharedBorrow)?;
        let (storage, scalar_type) = self.atomic_cell_storage(&parent, view, endpoint)?;
        let operand = |value| {
            self.values
                .get(&value)
                .copied()
                .filter(|operand: &TerminalScalarValue| operand.scalar_type() == scalar_type)
                .ok_or(TerminalInterpretError::VerifiedValueMissing(value))
        };
        let prior = if event.observes_resident() {
            let prior = self.read_primitive_storage(
                place,
                storage.clone(),
                scalar_type,
                scalar_type,
                false,
            )?;
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
        let proposed = match event {
            AtomicAccessEvent::CompareExchangeOnce { replacement, .. } => {
                Some(operand(replacement)?)
            }
            _ => None,
        };
        let mut writes = PrimitiveStorageWrites::default();
        if let Some(value) = replacement {
            writes.stage(self, place, storage, scalar_type, value)?;
        }
        match (prior, &operation.result) {
            (Some(prior), OperationResult::Structural(result))
                if matches!(event, AtomicAccessEvent::CompareExchangeOnce { .. }) =>
            {
                let custody = event.single_attempt_custody().ok_or_else(malformed)?;
                if !terminal_psi::types::atomic_once_result_is_plain(result, custody) {
                    return Err(malformed());
                }
                let types = self.structural_types();
                let cases = terminal_psi::types::atomic_once_outcome_cases(
                    types.declaration(&result.structural_type)?,
                    scalar_type,
                    custody,
                )
                .ok_or_else(malformed)?;
                // The serial engine has no interfering activation or failed
                // conditional-store attempt. It never invents Uncommitted.
                let index = usize::from(replacement.is_some());
                let result_case = cases[index].id;
                let fields = match custody.outcome_identity.cases()[index].payload {
                    None => Vec::new(),
                    Some(payload) => {
                        use language_core::atomic::AtomicCompareExchangeOutcomePayload;
                        let value = match payload {
                            AtomicCompareExchangeOutcomePayload::Observed
                            | AtomicCompareExchangeOutcomePayload::Displaced => prior,
                            AtomicCompareExchangeOutcomePayload::Proposed => {
                                proposed.ok_or_else(malformed)?
                            }
                        };
                        vec![(
                            vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                                cases[index].fields[0].id,
                            )],
                            value,
                        )]
                    }
                };
                self.publish_scalar_aggregate(
                    result,
                    crate::boundary::values::TerminalScalarAggregateValue {
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
        writes.commit(self)?;
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
