//! Exact original byte-descriptor capture and native completion acknowledgment.

use super::{BoundaryStorageInputs, TerminalBoundaryStorageBuffer};
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::byte_sequences::storage::ByteSequenceStorage;
use crate::byte_sequences::{ByteSequenceView, OriginalByteArrayWindow};
use crate::effects::TerminalEffectRejection;
use crate::effects::original_storage::TerminalOriginalStorageLoan;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::primitive_storage::{PrimitiveStorage, PrimitiveStorageEndpoint};
use crate::values::TerminalStructuralValue;
use std::sync::Arc;
use terminal_psi::{StructuralAccess, StructuralParameterDeclaration};

/// A Core-authenticated original byte window, with no staged native stores.
#[derive(Debug)]
pub struct TerminalBoundaryByteViewBuffer {
    pub(super) identity: Arc<()>,
    pub(super) argument_index: usize,
    pub(super) access: StructuralAccess,
    length: u64,
    pub(super) original_storage: Option<TerminalOriginalStorageLoan>,
    acknowledged: bool,
}

impl TerminalBoundaryByteViewBuffer {
    pub fn argument_index(&self) -> usize {
        self.argument_index
    }
    pub fn access(&self) -> StructuralAccess {
        self.access
    }
    pub fn length(&self) -> u64 {
        self.length
    }
    pub fn original_storage(&self) -> Option<&TerminalOriginalStorageLoan> {
        self.original_storage.as_ref()
    }
    /// The native provider already wrote the original cells. Completion will
    /// validate those cells and record mutation without replaying stores.
    pub fn acknowledge_original_write(&mut self) -> Result<(), TerminalEffectRejection> {
        if self.access != StructuralAccess::MutableBorrow
            || self.original_storage.is_none()
            || self.acknowledged
        {
            return Err(TerminalEffectRejection::new(
                "byte-view completion is not an unacknowledged original mutable loan",
            ));
        }
        self.acknowledged = true;
        Ok(())
    }
}

pub(super) struct BoundaryByteViewBinding {
    identity: Arc<()>,
    pub(super) argument_index: usize,
    pub(super) access: StructuralAccess,
    pub(super) source: ByteSequenceBinding,
    referent: TerminalStructuralValue,
    original: ByteSequenceStorage,
    pub(super) residents: Vec<std::rc::Rc<dyn crate::TerminalScalarBacking>>,
}

impl BoundaryStorageInputs {
    pub(crate) fn capture_byte_view(
        &mut self,
        execution: &TerminalExecution,
        parameter: &StructuralParameterDeclaration,
        referent: &TerminalStructuralValue,
        source: &ByteSequenceBinding,
        argument_index: usize,
    ) -> Result<(), TerminalInterpretError> {
        let access = terminal_semantics::descriptor_parameter_access(
            parameter,
            execution.program.structural_types().values(),
        )
        .ok_or_else(malformed)?;
        if parameter.structural_type != referent.structural_type
            || !matches!(
                access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            )
            || access != source.access()
            || !parameter.projected_qualifications.is_empty()
            || (access == StructuralAccess::MutableBorrow
                && (!parameter.qualifications.is_empty()
                    || !referent.qualifications.is_empty()
                    || !referent.projected_qualifications.is_empty()))
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "original byte-view capture requires its exact readable access and qualification scope",
            ));
        }
        let length = execution.byte_descriptor_length(source, referent)?;
        let bytes = match source {
            ByteSequenceBinding::Immutable(bytes) => {
                ByteSequenceStorage::copy(bytes.bytes(), execution.account.as_ref())?
            }
            ByteSequenceBinding::MutableArray {
                array,
                offset,
                length,
                ..
            } => {
                let start = usize::try_from(*offset).map_err(|_| malformed())?;
                let end = usize::try_from(offset.checked_add(*length).ok_or_else(malformed)?)
                    .map_err(|_| malformed())?;
                let view = execution.stored_byte_array_read_view(array, start, end)?;
                ByteSequenceStorage::from_values(
                    view.len(),
                    (0..view.len()).map(|index| view.read(index)),
                    execution.account.as_ref(),
                )?
            }
            _ => {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "original byte-view capture does not support a replaceable bounded byte owner",
                ));
            }
        };
        if u64::try_from(bytes.bytes().len()).ok() != Some(length) {
            return Err(malformed());
        }
        let identity = Arc::new(());
        self.buffers.push(TerminalBoundaryStorageBuffer::ByteView(
            TerminalBoundaryByteViewBuffer {
                identity: Arc::clone(&identity),
                argument_index,
                access,
                length,
                original_storage: None,
                acknowledged: false,
            },
        ));
        self.byte_views.push(BoundaryByteViewBinding {
            identity,
            argument_index,
            access,
            source: source.clone(),
            referent: referent.clone(),
            original: bytes,
            residents: Vec::new(),
        });
        Ok(())
    }

    pub(super) fn validate_byte_views(
        &self,
        execution: &TerminalExecution,
    ) -> Result<(), TerminalInterpretError> {
        let buffers = self
            .buffers
            .iter()
            .filter_map(|buffer| match buffer {
                TerminalBoundaryStorageBuffer::ByteView(buffer) => Some(buffer),
                _ => None,
            })
            .collect::<Vec<_>>();
        if buffers.len() != self.byte_views.len() {
            return Err(malformed());
        }
        for (buffer, binding) in buffers.into_iter().zip(&self.byte_views) {
            if !Arc::ptr_eq(&buffer.identity, &binding.identity)
                || buffer.argument_index != binding.argument_index
                || buffer.access != binding.access
                || buffer.original_storage.is_none()
                || buffer.length
                    != execution.byte_descriptor_length(&binding.source, &binding.referent)?
                || (buffer.access == StructuralAccess::MutableBorrow && !buffer.acknowledged)
            {
                return Err(malformed());
            }
            if let Some(array) = binding.array_window()? {
                let end = array
                    .offset
                    .checked_add(array.length)
                    .ok_or_else(malformed)?;
                for (relative, index) in (array.offset..end).enumerate() {
                    let current = execution.stored_byte_array_read(&array.array, index)?;
                    if buffer.access == StructuralAccess::SharedBorrow {
                        if current != binding.original.bytes()[relative] {
                            return Err(malformed());
                        }
                    }
                    if buffer.access == StructuralAccess::MutableBorrow {
                        let storage = PrimitiveStorage {
                            endpoint: PrimitiveStorageEndpoint::ArrayElement {
                                array: array.array.clone(),
                                index,
                            },
                            encoding: None,
                        };
                        execution.validate_scalar_loan_qualifications(&storage)?;
                    }
                }
            } else {
                // A raw immutable owner supplies only shared access. Observe
                // the same native residents rather than trusting entry bytes.
                if buffer.access != StructuralAccess::SharedBorrow
                    || binding.residents.len() != binding.original.bytes().len()
                {
                    return Err(malformed());
                }
                for (resident, expected) in binding.residents.iter().zip(binding.original.bytes()) {
                    let current = resident
                        .read()
                        .map_err(TerminalInterpretError::ResidentStorage)?;
                    if crate::structural_inputs::byte_arrays::storage::scalar_byte(&current)?
                        != *expected
                    {
                        return Err(malformed());
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn commit_byte_views(
        &mut self,
        execution: &mut TerminalExecution,
    ) -> Result<(), TerminalInterpretError> {
        for binding in &self.byte_views {
            let buffer = self
                .buffers
                .iter()
                .find_map(|buffer| match buffer {
                    TerminalBoundaryStorageBuffer::ByteView(buffer)
                        if Arc::ptr_eq(&buffer.identity, &binding.identity) =>
                    {
                        Some(buffer)
                    }
                    _ => None,
                })
                .ok_or_else(malformed)?;
            if binding.access != StructuralAccess::MutableBorrow {
                continue;
            }
            let array = binding.array_window()?.ok_or_else(malformed)?;
            if !buffer.acknowledged {
                return Err(malformed());
            }
            for index in array.offset
                ..array
                    .offset
                    .checked_add(array.length)
                    .ok_or_else(malformed)?
            {
                execution.record_original_scalar_write(&PrimitiveStorage {
                    endpoint: PrimitiveStorageEndpoint::ArrayElement {
                        array: array.array.clone(),
                        index,
                    },
                    encoding: None,
                })?;
            }
        }
        Ok(())
    }
}

impl BoundaryByteViewBinding {
    pub(super) fn entry_bytes(&self) -> &[u8] {
        self.original.bytes()
    }
    pub(super) fn view_type(&self) -> semantic_vocabulary::StructuralTypeId {
        self.referent.structural_type
    }
    pub(super) fn array_window(
        &self,
    ) -> Result<Option<OriginalByteArrayWindow>, TerminalInterpretError> {
        Ok(match &self.source {
            ByteSequenceBinding::Immutable(bytes) => bytes.original_array().cloned(),
            ByteSequenceBinding::MutableArray {
                array,
                array_type,
                offset,
                length,
                ..
            } => Some(OriginalByteArrayWindow {
                array: array.clone(),
                array_type: *array_type,
                offset: usize::try_from(*offset).map_err(|_| malformed())?,
                length: usize::try_from(*length).map_err(|_| malformed())?,
            }),
            _ => return Err(malformed()),
        })
    }

    pub(super) fn raw_bytes(&self) -> Result<&ByteSequenceView, TerminalInterpretError> {
        match &self.source {
            ByteSequenceBinding::Immutable(bytes) if bytes.original_array().is_none() => Ok(bytes),
            _ => Err(malformed()),
        }
    }
}
