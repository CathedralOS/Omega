//! Concatenation snapshots immutable input values before publishing fresh owned
//! backing. Reexecuting the operation creates a new activation-local value;
//! previous shared observations retain their original bytes.
use crate::byte_sequences::{ByteSequenceView, binding::ByteSequenceBinding};
use crate::errors::TerminalInterpretError;
use crate::execution::{OperationFlow, TerminalExecution};
use crate::values::{TerminalScalarValue, TerminalStructuralValue};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_psi::control_flow::ByteSequenceOperand;
use terminal_psi::{ByteSequenceCarrier, Operation, OperationKind, StructuralTypeShape};

impl TerminalExecution {
    pub(crate) fn execute_byte_sequence_concatenation(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } = &operation.kind
        else {
            return Err(invalid());
        };
        let result = operation.result.structural().ok_or_else(invalid)?;
        let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) =
            self.program
                .structural_types
                .get(&result.structural_type)
                .ok_or_else(invalid)?
                .shape
        else {
            return Err(invalid());
        };
        let left = self.concatenation_operand(left)?;
        let right = self.concatenation_operand(right)?;
        let length = left.len().checked_add(right.len()).ok_or_else(invalid)?;
        if u64::try_from(length).map_err(|_| invalid())? > capacity {
            return Err(invalid());
        }
        let mut bytes = Vec::with_capacity(length);
        bytes.extend_from_slice(left.bytes());
        bytes.extend_from_slice(right.bytes());
        let value = TerminalStructuralValue {
            opaque_identity: self.local_structural_identities.allocate()?,
            structural_type: result.structural_type,
            qualifications: result.qualifications.clone(),
            path: Vec::new(),
        };
        self.byte_sequence_values.insert(
            result.place,
            ByteSequenceBinding::Immutable(ByteSequenceView::new(bytes)),
        );
        self.structural_values.insert(result.place, value);
        Ok(OperationFlow::Advance)
    }

    fn concatenation_operand(
        &self,
        operand: &ByteSequenceOperand,
    ) -> Result<ByteSequenceView, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let bytes = match operand {
            ByteSequenceOperand::Whole { source, .. } => self
                .byte_sequence_values
                .get(source)
                .ok_or_else(invalid)?
                .immutable()?
                .clone(),
            ByteSequenceOperand::Field { source, field, .. } => {
                let (key, _) = self.resolve_structural_byte_sequence_field(
                    source.place,
                    &source.path,
                    *field,
                    false,
                )?;
                self.structural_byte_sequence_fields
                    .get(&key)
                    .ok_or_else(invalid)?
                    .clone()
            }
        };
        let expected = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?;
        if !matches!(self.values.get(&operand.length()), Some(TerminalScalarValue::Integer {
            scalar_type, value: IntegerValue::Unsigned(length),
        }) if *scalar_type == expected && *length == bytes.len() as u128)
        {
            return Err(invalid());
        }
        Ok(bytes)
    }
}
