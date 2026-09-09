//! Validate and replace one operation-owned immutable byte descriptor.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, StructuralPlaceKind, ValueId};
use terminal_psi::{
    ByteSequenceCarrier, Operation, OperationKind, StructuralMultiplicity, StructuralTypeShape,
};

use super::{
    TerminalExecution, TerminalInterpretError, TerminalScalarValue, TerminalStructuralValue,
};

#[cfg(test)]
mod tests;

impl TerminalExecution {
    /// Called only after the dispatch loop has charged this execution. Prepare
    /// the complete view before replacement, including when source and result
    /// storage alias. No failing validation may discard a previous descriptor.
    pub(super) fn execute_byte_sequence_subslice(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let OperationKind::ByteSequenceSubslice {
            source,
            start,
            end,
            length,
            ..
        } = &operation.kind
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let result = operation
            .result
            .structural()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let count_type = IntegerType::new(IntegerSign::Unsigned, 64)
            .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
        let count = |operand: &ValueId| -> Result<usize, TerminalInterpretError> {
            let value = self
                .values
                .get(operand)
                .ok_or(TerminalInterpretError::VerifiedValueMissing(*operand))?;
            let TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(value),
            } = value
            else {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            };
            if *scalar_type != count_type {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            let value = u64::try_from(*value)
                .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
            usize::try_from(value).map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)
        };
        let start = count(start)?;
        let end = count(end)?;
        let length = count(length)?;
        let source_value = self.structural_values.get(source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(*source),
        )?;
        let bytes = self
            .byte_sequence_values
            .get(source)
            .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                *source,
            ))?
            .immutable()?;
        let machine = self.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        // Canonical verification owns unique producer/result IDs. Retain the
        // exact operation declaration here so reuse cannot overwrite a parameter
        // or another producer's storage, even in malformed private runtime state.
        if !machine.structural_places.iter().any(|place| {
            place.id == result.place
                && place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type: result.structural_type,
                    }
        }) || !matches!(
            self.structural_types.get(&result.structural_type),
            Some(declaration)
                if declaration.shape
                    == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
        ) || bytes.len() != length
            || source_value.structural_type != result.structural_type
            || !source_value.path.is_empty()
            || !source_value.qualifications.is_empty()
            || result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(*source) || claim.place == Some(result.place))
            || self
                .live_affine_frontier
                .iter()
                .any(|entry| entry.place == *source || entry.place == result.place)
            || self.scalar_case_values.contains_key(source)
            || self.scalar_case_values.contains_key(&result.place)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let destination = TerminalStructuralValue {
            opaque_identity: result.place.get(),
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        match (
            self.structural_values.get(&result.place),
            self.byte_sequence_values.get(&result.place),
        ) {
            (None, None) => {}
            (Some(previous), Some(super::ByteSequenceBinding::Immutable(_)))
                if *previous == destination => {}
            _ => return Err(TerminalInterpretError::VerifiedOperationMalformed),
        }
        // subslice snapshots the checked window and clones its Arc, not bytes.
        // Other aliases keep their windows when this result is rebound.
        let view = bytes
            .subslice(start, end)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        self.byte_sequence_values
            .insert(result.place, super::ByteSequenceBinding::Immutable(view));
        self.structural_values.insert(result.place, destination);
        Ok(())
    }
}
