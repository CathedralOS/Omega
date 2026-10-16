//! External writes capture the admitted loan, not an identity to resolve later.
//! A view stages its live range with no length-write authority; a whole bounded
//! owner retains its separate replacement contract. Checked providers bypass
//! this staging and borrow live storage through their ordinary call frames.

use crate::byte_sequences::ByteSequenceView;
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::effects::TerminalEffectRejection;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundaryByteWriteScope {
    View,
    Owner,
}

/// One mutable boundary argument's staged bytes. The core commits writes only
/// after validating every captured loan and the handler's declared result.
/// A borrowed view exposes its existing extent, never its owner's capacity.
#[derive(Debug)]
pub struct TerminalBoundaryByteBuffer {
    argument_index: usize,
    capacity: u64,
    scope: BoundaryByteWriteScope,
    bytes: Vec<u8>,
}

impl TerminalBoundaryByteBuffer {
    /// Index in the effect's structural arguments, not its scalar arguments.
    pub fn argument_index(&self) -> usize {
        self.argument_index
    }

    /// Maximum writable byte count. A view reports its initial live length,
    /// never spare capacity in the owning field or array.
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Write existing initialized bytes without changing the loan's extent.
    /// This is private staged storage, not a pointer into interpreter backing.
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    /// Replace a whole bounded owner's live sequence. Ordinary borrowed views
    /// cannot use this callback, even for an equally sized replacement.
    pub fn replace(&mut self, bytes: &[u8]) -> Result<(), TerminalEffectRejection> {
        if self.scope != BoundaryByteWriteScope::Owner {
            return Err(TerminalEffectRejection::new(
                "boundary byte view grants no whole-field replacement authority",
            ));
        }
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

pub(super) struct BoundaryByteBufferBinding {
    argument_index: usize,
    capacity: u64,
    scope: BoundaryByteWriteScope,
    source: ByteSequenceBinding,
}

impl BoundaryByteBufferBinding {
    pub(super) fn capture(
        execution: &TerminalExecution,
        argument_index: usize,
        source: &ByteSequenceBinding,
        scope: BoundaryByteWriteScope,
    ) -> Result<(Self, TerminalBoundaryByteBuffer), TerminalInterpretError> {
        let (bytes, capacity) = match source {
            ByteSequenceBinding::MutableArray {
                array,
                offset,
                length,
                ..
            } => {
                if scope != BoundaryByteWriteScope::View {
                    return Err(malformed());
                }
                source.validate_presented_referent(execution)?;
                let start = usize::try_from(*offset).map_err(|_| malformed())?;
                let end = usize::try_from(offset.checked_add(*length).ok_or_else(malformed)?)
                    .map_err(|_| malformed())?;
                let bytes = execution.stored_byte_array_snapshot(array, start, end)?;
                (bytes.bytes().to_vec(), *length)
            }
            ByteSequenceBinding::MutableField { .. } | ByteSequenceBinding::MutableOwner { .. } => {
                source.validate_presented_referent(execution)?;
                let (bytes, capacity) = execution.exclusive_owner_bytes(source)?;
                let length = u64::try_from(bytes.len()).map_err(|_| malformed())?;
                (
                    bytes.bytes().to_vec(),
                    if scope == BoundaryByteWriteScope::View {
                        length
                    } else {
                        capacity
                    },
                )
            }
            ByteSequenceBinding::Immutable(_) => return Err(malformed()),
        };
        Ok((
            Self {
                argument_index,
                capacity,
                scope,
                source: source.clone(),
            },
            TerminalBoundaryByteBuffer {
                argument_index,
                capacity,
                scope,
                bytes,
            },
        ))
    }

    pub(super) fn validate(
        &self,
        execution: &TerminalExecution,
        buffer: &TerminalBoundaryByteBuffer,
    ) -> Result<(), TerminalInterpretError> {
        if buffer.argument_index != self.argument_index
            || buffer.capacity != self.capacity
            || buffer.scope != self.scope
            || buffer.bytes.len() as u128 > u128::from(self.capacity)
            || self.scope == BoundaryByteWriteScope::View
                && buffer.bytes.len() as u128 != u128::from(self.capacity)
        {
            return Err(malformed());
        }
        // Recheck initialized original backing before publishing any result or
        // writes. The handler cannot replace this captured source or range.
        let current_capacity = match &self.source {
            ByteSequenceBinding::MutableArray {
                array,
                offset,
                length,
                ..
            } => {
                if self.scope != BoundaryByteWriteScope::View {
                    return Err(malformed());
                }
                self.source.validate_presented_referent(execution)?;
                let start = usize::try_from(*offset).map_err(|_| malformed())?;
                let end = usize::try_from(offset.checked_add(*length).ok_or_else(malformed)?)
                    .map_err(|_| malformed())?;
                if end > execution.stored_byte_array_length(array)? {
                    return Err(malformed());
                }
                // Validate every endpoint before any write, without allocating
                // another snapshot of the already captured range.
                for byte_offset in start..end {
                    execution.stored_byte_array_read(array, byte_offset)?;
                }
                *length
            }
            ByteSequenceBinding::MutableField { .. } | ByteSequenceBinding::MutableOwner { .. } => {
                self.source.validate_presented_referent(execution)?;
                let (bytes, capacity) = execution.exclusive_owner_bytes(&self.source)?;
                let length = u64::try_from(bytes.len()).map_err(|_| malformed())?;
                if self.scope == BoundaryByteWriteScope::View {
                    length
                } else {
                    capacity
                }
            }
            ByteSequenceBinding::Immutable(_) => return Err(malformed()),
        };
        if current_capacity != self.capacity {
            return Err(malformed());
        }
        Ok(())
    }

    pub(super) fn commit(
        self,
        execution: &mut TerminalExecution,
        buffer: TerminalBoundaryByteBuffer,
    ) -> Result<(), TerminalInterpretError> {
        match self.source {
            ByteSequenceBinding::MutableArray { array, offset, .. } => {
                let start = usize::try_from(offset).map_err(|_| malformed())?;
                for (byte_offset, byte) in buffer.bytes.into_iter().enumerate() {
                    execution.stored_byte_array_write(
                        &array,
                        start.checked_add(byte_offset).ok_or_else(malformed)?,
                        byte,
                    )?;
                }
            }
            ByteSequenceBinding::MutableField { field, .. } => {
                execution.structural_byte_sequence_fields.insert(
                    field,
                    ByteSequenceBinding::Immutable(ByteSequenceView::new(buffer.bytes)),
                );
            }
            ByteSequenceBinding::MutableOwner { home, .. } => {
                *execution.byte_owner_bytes_mut(&home)? = ByteSequenceView::new(buffer.bytes);
            }
            ByteSequenceBinding::Immutable(_) => return Err(malformed()),
        }
        Ok(())
    }
}
