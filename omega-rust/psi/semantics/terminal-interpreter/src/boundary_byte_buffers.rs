//! Exact array/field loans and separately staged external byte-field replacement.

use super::*;
use terminal_psi::{ByteSequenceCarrier, StructuralFieldType};

/// One mutable boundary argument's live bytes and original inline capacity.
/// Replacement is staged: the interpreter commits it only after the handler
/// succeeds and its declared result is validated. This grants no qualifications.
#[derive(Debug)]
pub struct TerminalBoundaryByteBuffer {
    argument_index: usize,
    capacity: u64,
    bytes: Vec<u8>,
}

impl TerminalBoundaryByteBuffer {
    /// Index in the effect's structural arguments, not its scalar arguments.
    pub fn argument_index(&self) -> usize {
        self.argument_index
    }

    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Replace the entire live sequence, including its length. An oversized
    /// replacement rejects without changing the staged value.
    pub fn replace(&mut self, bytes: &[u8]) -> Result<(), TerminalEffectRejection> {
        if bytes.len() as u128 > u128::from(self.capacity) {
            return Err(TerminalEffectRejection::new(
                "boundary byte replacement exceeds inline capacity",
            ));
        }
        self.bytes.clear();
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}

struct BoundaryByteBufferBinding {
    argument_index: usize,
    capacity: u64,
    field: StructuralByteSequenceRuntimeField,
}

pub(super) struct BoundaryArguments {
    pub(super) values: Vec<TerminalStructuralValue>,
    pub(super) bytes: Vec<Option<Vec<u8>>>,
    pub(super) buffers: Vec<TerminalBoundaryByteBuffer>,
    bindings: Vec<BoundaryByteBufferBinding>,
    byte_sequences: Vec<Option<ByteSequenceBinding>>,
}

impl BoundaryArguments {
    /// Provider bodies borrow the live array/field binding, not the staged external
    /// buffers. Nested boundary calls commit backing; provider return must not
    /// restore the pre-call snapshot.
    pub(super) fn into_call_arguments(
        self,
        parameters: &[StructuralParameterDeclaration],
    ) -> Result<StructuralCallArguments, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let values = bind_structural_arguments(parameters, &self.values)?;
        if parameters.len() != self.byte_sequences.len() {
            return Err(invalid());
        }
        let mut byte_sequences = BTreeMap::new();
        for (parameter, binding) in parameters.iter().zip(self.byte_sequences) {
            let Some(binding) = binding else {
                continue;
            };
            let access = match &binding {
                ByteSequenceBinding::Immutable(_) => StructuralAccess::SharedBorrow,
                ByteSequenceBinding::MutableField { .. }
                | ByteSequenceBinding::MutableArray { .. } => StructuralAccess::MutableBorrow,
            };
            if parameter.access != access
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || byte_sequences.insert(parameter.place, binding).is_some()
            {
                return Err(invalid());
            }
        }
        Ok(StructuralCallArguments {
            values,
            byte_sequences,
            scalar_arrays: BTreeMap::new(),
        })
    }

    /// A handler can reorder its slice, but cannot thereby retarget writeback.
    /// Check every binding before committing either fields or the result.
    pub(super) fn validate_writeback(&self) -> Result<(), TerminalInterpretError> {
        if self.buffers.len() != self.bindings.len()
            || self
                .buffers
                .iter()
                .zip(&self.bindings)
                .any(|(buffer, binding)| {
                    buffer.argument_index != binding.argument_index
                        || buffer.capacity != binding.capacity
                        || buffer.bytes.len() as u128 > u128::from(binding.capacity)
                })
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        Ok(())
    }

    pub(super) fn commit(self, execution: &mut TerminalExecution) {
        for (buffer, binding) in self.buffers.into_iter().zip(self.bindings) {
            execution
                .structural_byte_sequence_fields
                .insert(binding.field, ByteSequenceView::new(buffer.bytes));
        }
    }
}

impl TerminalExecution {
    pub(super) fn resolve_boundary_arguments(
        &self,
        parameters: &[StructuralParameterDeclaration],
        arguments: &[StructuralArgument],
    ) -> Result<BoundaryArguments, TerminalInterpretError> {
        let mut resolved = self.prepare_boundary_arguments(parameters, arguments)?;
        for (argument_index, binding) in resolved.byte_sequences.iter().enumerate() {
            match binding {
                None => resolved.bytes.push(None),
                Some(ByteSequenceBinding::MutableArray { .. }) => {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
                Some(ByteSequenceBinding::Immutable(view)) => {
                    resolved.bytes.push(Some(view.bytes().to_vec()));
                }
                Some(ByteSequenceBinding::MutableField {
                    field, capacity, ..
                }) => {
                    let bytes = self
                        .structural_byte_sequence_fields
                        .get(field)
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?
                        .bytes();
                    resolved.bytes.push(Some(bytes.to_vec()));
                    resolved.buffers.push(TerminalBoundaryByteBuffer {
                        argument_index,
                        capacity: *capacity,
                        bytes: bytes.to_vec(),
                    });
                }
            }
        }
        Ok(resolved)
    }

    pub(super) fn prepare_boundary_arguments(
        &self,
        parameters: &[StructuralParameterDeclaration],
        arguments: &[StructuralArgument],
    ) -> Result<BoundaryArguments, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        if parameters.len() != arguments.len() {
            return Err(invalid());
        }
        let mut resolved = BoundaryArguments {
            values: Vec::with_capacity(arguments.len()),
            bytes: Vec::with_capacity(arguments.len()),
            buffers: Vec::new(),
            bindings: Vec::new(),
            byte_sequences: Vec::with_capacity(arguments.len()),
        };
        for (argument_index, (parameter, argument)) in parameters.iter().zip(arguments).enumerate()
        {
            // Reuse the ordinary call's exact initialized-array binding. It can
            // enter a checked provider and be forwarded again, but never enters
            // the external field-replacement staging/writeback protocol.
            if let Some(binding @ ByteSequenceBinding::MutableArray { .. }) =
                self.prepare_array_view_argument(parameter, argument)?
            {
                let ByteSequenceBinding::MutableArray { referent, .. } = &binding else {
                    return Err(invalid());
                };
                resolved.values.push(referent.clone());
                resolved.byte_sequences.push(Some(binding));
                continue;
            }
            let declaration = self
                .structural_types
                .get(&parameter.structural_type)
                .ok_or_else(invalid)?;
            if parameter.access == StructuralAccess::MutableBorrow
                && declaration.shape
                    == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
            {
                if argument.access != StructuralAccess::MutableBorrow
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return Err(invalid());
                }
                let binding = self.resolve_mutable_boundary_binding(parameter, argument)?;
                let ByteSequenceBinding::MutableField {
                    field: destination,
                    capacity,
                    referent,
                    ..
                } = &binding
                else {
                    return Err(invalid());
                };
                let bytes = self
                    .structural_byte_sequence_fields
                    .get(destination)
                    .ok_or_else(invalid)?
                    .bytes();
                if bytes.len() as u128 > u128::from(*capacity) {
                    return Err(invalid());
                }
                resolved.values.push(referent.clone());
                resolved.bindings.push(BoundaryByteBufferBinding {
                    argument_index,
                    capacity: *capacity,
                    field: destination.clone(),
                });
                resolved.byte_sequences.push(Some(binding));
            } else {
                let values = resolve_structural_arguments(
                    &self.structural_types,
                    &self.structural_values,
                    std::slice::from_ref(argument),
                )?;
                let mut bytes = self.bind_byte_sequence_arguments(
                    std::slice::from_ref(parameter),
                    std::slice::from_ref(argument),
                    &values,
                )?;
                let binding = bytes.remove(&parameter.place);
                resolved.byte_sequences.push(binding);
                resolved.values.extend(values);
            }
        }
        Ok(resolved)
    }
    fn resolve_mutable_boundary_binding(
        &self,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Result<ByteSequenceBinding, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        if argument.path.is_empty() {
            let value = self
                .structural_values
                .get(&argument.place)
                .ok_or_else(invalid)?;
            if value.structural_type != parameter.structural_type {
                return Err(invalid());
            }
            let binding = self
                .byte_sequence_values
                .get(&argument.place)
                .ok_or_else(invalid)?;
            binding.validate_mutable_referent(&self.structural_types, value)?;
            return Ok(binding.clone());
        }
        let Some((StructuralPathSegment::Field(identity), prefix)) = argument.path.split_last()
        else {
            return Err(invalid());
        };
        let mut parent = resolve_structural_arguments(
            &self.structural_types,
            &self.structural_values,
            &[StructuralArgument {
                place: argument.place,
                path: prefix.to_vec(),
                access: argument.access,
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
        let field = fields
            .iter()
            .find(|field| field.identity == *identity && !field.relevance.is_erased())
            .ok_or_else(invalid)?;
        let StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) =
            field.field_type
        else {
            return Err(invalid());
        };
        let destination = StructuralByteSequenceRuntimeField {
            parent: StructuralRuntimePlace::from(&parent),
            field: field.id,
        };

        let parent_type = parent.structural_type;
        // Present the callee parameter type, retaining the original
        // referent and full field path for alias/custody checks.
        parent.structural_type = parameter.structural_type;
        parent.qualifications.clear();
        parent
            .path
            .push(StructuralPathSegment::Field(identity.clone()));

        let binding = ByteSequenceBinding::MutableField {
            field: destination,
            capacity,
            parent_type,
            referent: parent,
        };
        binding.validate_mutable_referent(
            &self.structural_types,
            match &binding {
                ByteSequenceBinding::MutableField { referent, .. } => referent,
                ByteSequenceBinding::Immutable(_) | ByteSequenceBinding::MutableArray { .. } => {
                    return Err(invalid());
                }
            },
        )?;
        Ok(binding)
    }
}

#[cfg(test)]
mod binding_tests;

#[cfg(test)]
mod tests {
    use super::TerminalBoundaryByteBuffer;

    #[test]
    fn replacements_use_live_length_not_capacity_sized_allocation() {
        for capacity in [0, 1, u64::MAX] {
            let mut buffer = TerminalBoundaryByteBuffer {
                argument_index: 0,
                capacity,
                bytes: Vec::new(),
            };
            buffer.replace(&[]).unwrap();
            if capacity == 0 {
                assert!(buffer.replace(&[0]).is_err());
                assert!(buffer.bytes().is_empty());
            } else {
                buffer.replace(&[255]).unwrap();
                assert_eq!(buffer.bytes(), &[255]);
                buffer.replace(&[]).unwrap();
                assert!(buffer.bytes().is_empty());
            }
            assert_eq!(buffer.capacity(), capacity);
        }
    }
}
