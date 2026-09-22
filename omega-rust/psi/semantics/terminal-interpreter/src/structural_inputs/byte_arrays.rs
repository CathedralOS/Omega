//! Explicit initialized fixed-array inputs and their fieldless original backing.
use crate::byte_sequences::ByteSequenceView;
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::custody::resolve_structural_arguments;
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::values::StructuralRuntimePlace;
use crate::values::TerminalStructuralValue;
use semantic_vocabulary::ScalarType;
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeMap;
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralPathSegment;
use terminal_psi::StructuralTypeDeclaration;
use terminal_psi::StructuralTypeShape;
use terminal_psi::{ByteSequenceCarrier, StructuralFieldType};

/// Initialized contents of a true fixed u8 array reachable from an entry input.
/// The static field/index path is relative to the dense structural argument. Exactly the declared
/// positive number of bytes is required; missing contents are never initialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStructuralByteArrayValue {
    pub argument_index: u32,
    pub path: Vec<StructuralPathSegment>,
    pub bytes: Vec<u8>,
}

pub(crate) fn byte_array_length(
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
        structural_type = match (segment, &types.get(&structural_type)?.shape) {
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                next
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}

impl TerminalExecution {
    pub(crate) fn bind_byte_arrays(
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
            let parameter = machine
                .structural_parameters
                .get(argument.argument_index as usize)
                .ok_or_else(invalid)?;
            if !matches!(
                parameter.access,
                StructuralAccess::Owned
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::SharedBorrow
            ) || !matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            ) || !parameter.qualifications.is_empty()
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

    pub(crate) fn prepare_array_view_argument(
        &self,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Result<Option<(TerminalStructuralValue, ByteSequenceBinding)>, TerminalInterpretError>
    {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        if !matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
        ) || !self
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
            || argument.access != parameter.access
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(argument.place))
        {
            return Err(invalid());
        }
        if argument.path.is_empty()
            && let Some(binding @ ByteSequenceBinding::Immutable(_)) =
                self.byte_sequence_values.get(&argument.place)
        {
            let value = self
                .structural_values
                .get(&argument.place)
                .ok_or_else(invalid)?;
            if parameter.access != StructuralAccess::SharedBorrow
                || value.structural_type != parameter.structural_type
                || !value.qualifications.is_empty()
            {
                return Err(invalid());
            }
            return Ok(Some((value.clone(), binding.clone())));
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
            if parameter.access == StructuralAccess::SharedBorrow {
                let ByteSequenceBinding::MutableArray {
                    array,
                    offset,
                    length,
                    ..
                } = binding
                else {
                    return Err(invalid());
                };
                let bytes = self.structural_byte_arrays.get(array).ok_or_else(invalid)?;
                let start = usize::try_from(*offset).map_err(|_| invalid())?;
                let end = usize::try_from(offset.checked_add(*length).ok_or_else(invalid)?)
                    .map_err(|_| invalid())?;
                return Ok(Some((
                    value.clone(),
                    ByteSequenceBinding::Immutable(bytes.subslice(start, end).ok_or_else(invalid)?),
                )));
            }
            return Ok(Some((value.clone(), binding.clone())));
        }
        // A genuine array projection resolves to its own structural type. The
        // bounded byte-field presentation remains owned by its separate path.
        let Some(root) = self.structural_values.get(&argument.place) else {
            return Err(invalid());
        };
        // A window changes the borrowed extent, never the backing's identity.
        // Keep the range out of the storage key so repeated and forwarded calls
        // update the same array and leave bytes outside the loan untouched.
        let backing_path = match argument.path.split_last() {
            Some((StructuralPathSegment::FixedByteRange { .. }, backing)) => backing,
            _ => &argument.path,
        };
        let Some(array_type) =
            array_path_type(&self.structural_types, root.structural_type, backing_path)
        else {
            return Ok(None);
        };
        let Some(length) = byte_array_length(&self.structural_types, array_type) else {
            return Ok(None);
        };
        let source = self
            .machines
            .get(&self.current_machine)
            .ok_or_else(invalid)?
            .structural_parameters
            .iter()
            .chain(
                self.machines
                    .get(&self.current_machine)
                    .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(
                        self.current_machine,
                    ))?
                    .blocks
                    .values()
                    .flat_map(|block| &block.structural_parameters),
            )
            .find(|source| source.place == argument.place)
            .ok_or_else(invalid)?;
        if source.multiplicity != StructuralMultiplicity::Unrestricted
            || source.structural_type != root.structural_type
            || !source.qualifications.is_empty()
            || !source.projected_qualifications.is_empty()
        {
            return Err(invalid());
        }
        let window = terminal_semantics::fixed_byte_array_window(
            self.structural_types.values(),
            source,
            argument,
            parameter,
        )
        .ok_or_else(invalid)?;
        let backing_argument = StructuralArgument {
            place: argument.place,
            path: window.backing_path.to_vec(),
            access: argument.access,
        };
        let mut referent = resolve_structural_arguments(
            &self.structural_types,
            &self.structural_values,
            std::slice::from_ref(&backing_argument),
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
        if let Some(segment @ StructuralPathSegment::FixedByteRange { .. }) = argument.path.last() {
            referent.path.push(segment.clone());
        }
        if parameter.access == StructuralAccess::SharedBorrow {
            let start = usize::try_from(window.offset).map_err(|_| invalid())?;
            let end = usize::try_from(
                window
                    .offset
                    .checked_add(window.length)
                    .ok_or_else(invalid)?,
            )
            .map_err(|_| invalid())?;
            return Ok(Some((
                referent,
                ByteSequenceBinding::Immutable(bytes.subslice(start, end).ok_or_else(invalid)?),
            )));
        }
        Ok(Some((
            referent.clone(),
            ByteSequenceBinding::MutableArray {
                array,
                array_type,
                offset: window.offset,
                length: window.length,
                referent,
            },
        )))
    }
}
