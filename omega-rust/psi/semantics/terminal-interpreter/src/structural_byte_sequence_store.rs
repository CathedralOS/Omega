//! Non-observing bounded byte replacement with invocation-independent backing.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, StructuralFieldId};
use terminal_psi::{
    ByteSequenceCarrier, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralArgument, StructuralFieldType, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeShape,
};

use super::{
    StructuralByteSequenceRuntimeField, StructuralRuntimePlace, TerminalExecution,
    TerminalInterpretError, TerminalScalarValue, resolve_structural_arguments,
};

impl TerminalExecution {
    /// Observe the current live bytes at an original structural referent and
    /// carrier path. Unused capacity is not part of the returned sequence.
    pub fn structural_byte_sequence_field(
        &self,
        opaque_identity: u64,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
    ) -> Option<&[u8]> {
        self.structural_byte_sequence_fields
            .get(&StructuralByteSequenceRuntimeField {
                parent: StructuralRuntimePlace {
                    opaque_identity,
                    path: path.to_vec(),
                },
                field,
            })
            .map(|view| view.bytes())
    }

    /// Dispatch charges fuel first. Validate and snapshot the complete source
    /// before replacing either live bytes or length; no failure partially writes.
    pub(super) fn execute_structural_byte_sequence_field_store(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            length,
            ..
        } = &operation.kind
        else {
            return Err(invalid());
        };
        if operation.result != OperationResult::Unit || destination == source {
            return Err(invalid());
        }
        let machine = self
            .machines
            .get(&self.current_machine)
            .ok_or_else(invalid)?;
        let parameter = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == *destination)
            .ok_or_else(invalid)?;
        if !matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        ) || !matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ) || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || !terminal_psi::is_bounded_structural_scalar_store_path(path)
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(*destination) || claim.place == Some(*source))
        {
            return Err(invalid());
        }
        let parent = resolve_structural_arguments(
            &self.structural_types,
            &self.structural_values,
            &[StructuralArgument {
                place: *destination,
                path: path.clone(),
                access: parameter.access,
            }],
        )?
        .pop()
        .ok_or_else(invalid)?;
        let declaration = self
            .structural_types
            .get(&parent.structural_type)
            .ok_or_else(invalid)?;
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return Err(invalid());
        };
        let field_declaration = fields
            .iter()
            .find(|candidate| candidate.id == *field && !candidate.relevance.is_erased())
            .ok_or_else(invalid)?;
        let StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) =
            field_declaration.field_type
        else {
            return Err(invalid());
        };
        let TerminalScalarValue::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(length),
        } = self.values.get(length).ok_or_else(invalid)?
        else {
            return Err(invalid());
        };
        if *scalar_type != IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?
            || *length > u128::from(capacity)
        {
            return Err(invalid());
        }
        let source_value = self.structural_values.get(source).ok_or_else(invalid)?;
        if !source_value.path.is_empty()
            || !source_value.qualifications.is_empty()
            || !matches!(self.structural_types.get(&source_value.structural_type), Some(declaration)
                if declaration.shape == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView))
        {
            return Err(invalid());
        }
        let bytes = self.byte_sequence_values.get(source).ok_or_else(invalid)?;
        if bytes.len() as u128 != *length {
            return Err(invalid());
        }
        // The backing is immutable. Sharing it copies the semantic byte value;
        // subsequent field replacement cannot modify this source or other views.
        self.structural_byte_sequence_fields.insert(
            StructuralByteSequenceRuntimeField {
                parent: StructuralRuntimePlace::from(&parent),
                field: *field,
            },
            bytes.clone(),
        );
        Ok(())
    }
}
