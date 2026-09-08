//! Explicit initialized fixed-array inputs and their fieldless original backing.

use super::*;
use terminal_psi::{ByteSequenceCarrier, StructuralFieldType};

/// Initialized contents of a true fixed u8 array reachable from an entry input.
/// The path is relative to the dense structural argument. Exactly the declared
/// positive number of bytes is required; missing contents are never initialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStructuralByteArrayValue {
    pub argument_index: u32,
    pub path: Vec<StructuralPathSegment>,
    pub bytes: Vec<u8>,
}

pub(super) fn byte_array_length(
    types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    array: StructuralTypeId,
) -> Option<u64> {
    let StructuralTypeShape::FixedArray { element, length } = types.get(&array)?.shape else {
        return None;
    };
    let StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(element)) =
        types.get(&element)?.shape
    else {
        return None;
    };
    (length != 0
        && !element.is_address()
        && element.sign() == semantic_vocabulary::IntegerSign::Unsigned
        && element.bits() == 8)
        .then_some(length)
}

fn array_path_type(
    types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in path {
        let StructuralPathSegment::Field(identity) = segment else {
            return None;
        };
        let StructuralTypeShape::Record { fields } = &types.get(&structural_type)?.shape else {
            return None;
        };
        let field = fields
            .iter()
            .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
        let StructuralFieldType::Structural(next) = field.field_type else {
            return None;
        };
        structural_type = next;
    }
    Some(structural_type)
}

impl TerminalExecution {
    /// Verify an artifact and install explicitly initialized fixed-array contents.
    /// This grants neither missing contents nor source-owned array construction.
    pub fn start_artifact_with_structural_arguments_and_byte_arrays(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[TerminalStructuralValue],
        byte_arrays: &[TerminalStructuralByteArrayValue],
    ) -> Result<Self, TerminalArtifactInterpretError> {
        let mut execution = Self::start_artifact_with_structural_arguments(
            semantic_bytes,
            proof_bytes,
            profile,
            scalar_arguments,
            structural_arguments,
        )?;
        execution
            .bind_byte_arrays(byte_arrays)
            .map_err(TerminalArtifactInterpretError::Execution)?;
        Ok(execution)
    }

    pub(super) fn bind_byte_arrays(
        &mut self,
        arguments: &[TerminalStructuralByteArrayValue],
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let machine = self
            .machines
            .get(&self.current_machine)
            .ok_or_else(invalid)?;
        let mut storage = BTreeMap::new();
        for argument in arguments {
            if argument
                .path
                .iter()
                .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
            {
                return Err(invalid());
            }
            let parameter = machine
                .structural_parameters
                .get(argument.argument_index as usize)
                .ok_or_else(invalid)?;
            if parameter.access != StructuralAccess::MutableBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || self
                    .live_claims
                    .values()
                    .any(|claim| claim.place == Some(parameter.place))
            {
                return Err(invalid());
            }
            let root = self
                .structural_values
                .get(&parameter.place)
                .ok_or_else(invalid)?;
            if !root.qualifications.is_empty() || root.structural_type != parameter.structural_type
            {
                return Err(invalid());
            }
            let array_type =
                array_path_type(&self.structural_types, root.structural_type, &argument.path)
                    .ok_or_else(invalid)?;
            let length =
                byte_array_length(&self.structural_types, array_type).ok_or_else(invalid)?;
            if argument.bytes.len() as u128 != u128::from(length) {
                return Err(invalid());
            }
            let mut array = StructuralRuntimePlace::from(root);
            array.path.extend_from_slice(&argument.path);
            if self.structural_byte_arrays.contains_key(&array)
                || storage
                    .insert(array, ByteSequenceView::new(argument.bytes.clone()))
                    .is_some()
            {
                return Err(invalid());
            }
        }
        self.structural_byte_arrays.extend(storage);
        Ok(())
    }

    /// Observe the current contents of an explicitly initialized raw fixed array.
    /// No field identity, capacity, or mutable live-length header is involved.
    pub fn structural_byte_array(
        &self,
        opaque_identity: u64,
        path: &[StructuralPathSegment],
    ) -> Option<&[u8]> {
        self.structural_byte_arrays
            .get(&StructuralRuntimePlace {
                opaque_identity,
                path: path.to_vec(),
            })
            .map(ByteSequenceView::bytes)
    }

    pub(super) fn prepare_array_view_argument(
        &self,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Result<Option<ByteSequenceBinding>, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        if parameter.access != StructuralAccess::MutableBorrow
            || !self
                .structural_types
                .get(&parameter.structural_type)
                .is_some_and(|declaration| {
                    declaration.shape
                        == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
                })
        {
            return Ok(None);
        }
        if parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || argument.access != StructuralAccess::MutableBorrow
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(argument.place))
        {
            return Err(invalid());
        }
        if argument.path.is_empty()
            && let Some(binding @ ByteSequenceBinding::MutableArray { .. }) =
                self.byte_sequence_values.get(&argument.place)
        {
            let value = self
                .structural_values
                .get(&argument.place)
                .ok_or_else(invalid)?;
            if value.structural_type != parameter.structural_type {
                return Err(invalid());
            }
            binding.validate_mutable_referent(&self.structural_types, value)?;
            return Ok(Some(binding.clone()));
        }
        // A genuine array projection resolves to its own structural type. The
        // bounded byte-field presentation remains owned by its separate path.
        let Some(root) = self.structural_values.get(&argument.place) else {
            return Err(invalid());
        };
        let Some(array_type) =
            array_path_type(&self.structural_types, root.structural_type, &argument.path)
        else {
            return Ok(None);
        };
        let Some(length) = byte_array_length(&self.structural_types, array_type) else {
            return Ok(None);
        };
        if argument
            .path
            .iter()
            .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
        {
            return Err(invalid());
        }
        let source = self
            .machines
            .get(&self.current_machine)
            .ok_or_else(invalid)?
            .structural_parameters
            .iter()
            .chain(
                self.blocks
                    .values()
                    .flat_map(|block| &block.structural_parameters),
            )
            .find(|source| source.place == argument.place)
            .ok_or_else(invalid)?;
        if source.access != StructuralAccess::MutableBorrow
            || source.multiplicity != StructuralMultiplicity::Unrestricted
            || source.structural_type != root.structural_type
            || !source.qualifications.is_empty()
            || !source.projected_qualifications.is_empty()
        {
            return Err(invalid());
        }
        let mut referent = resolve_structural_arguments(
            &self.structural_types,
            &self.structural_values,
            std::slice::from_ref(argument),
        )?
        .pop()
        .ok_or_else(invalid)?;
        if !root.qualifications.is_empty() || !referent.qualifications.is_empty() {
            return Err(invalid());
        }
        let array = StructuralRuntimePlace::from(&referent);
        let bytes = self
            .structural_byte_arrays
            .get(&array)
            .ok_or_else(invalid)?;
        if bytes.len() as u128 != u128::from(length) {
            return Err(invalid());
        }
        referent.structural_type = parameter.structural_type;
        Ok(Some(ByteSequenceBinding::MutableArray {
            array,
            array_type,
            referent,
        }))
    }
}
