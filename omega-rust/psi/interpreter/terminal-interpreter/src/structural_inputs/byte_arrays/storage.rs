//! Array loans retain one backing home across suspended calls. Frame homes
//! address the owner's payload directly; no second byte store or copyback exists.
//! Verified loans cannot outlive their owner, so a suspended frame's stack depth
//! remains stable for the lifetime of the loan, including nested forwarding.

use crate::byte_sequences::ByteSequenceView;
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::scalar_array::TerminalScalarArrayValue;
use crate::values::{StructuralRuntimePlace, TerminalScalarValue};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, PlaceId, StructuralTypeId};

#[derive(Clone)]
pub(crate) enum ArrayStorageHome {
    Runtime,
    Frame {
        depth: usize,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
}

#[derive(Clone)]
pub(crate) struct ArrayStorage {
    pub(crate) referent: StructuralRuntimePlace,
    pub(crate) home: ArrayStorageHome,
}

impl From<StructuralRuntimePlace> for ArrayStorage {
    fn from(referent: StructuralRuntimePlace) -> Self {
        Self {
            referent,
            home: ArrayStorageHome::Runtime,
        }
    }
}

impl TerminalExecution {
    fn frame_array(&self, depth: usize, place: PlaceId) -> Option<&TerminalScalarArrayValue> {
        if depth == self.call_stack.len() {
            self.scalar_array_values.get(&place)
        } else {
            self.call_stack.get(depth)?.scalar_array_values.get(&place)
        }
    }

    /// A frame-owned fixed byte array has one payload, not an opaque projected
    /// host root. Admission currently permits its whole backing plus byte ranges.
    fn frame_byte_array_window(
        &self,
        storage: &ArrayStorage,
    ) -> Result<(usize, usize), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let ArrayStorageHome::Frame {
            depth,
            place,
            structural_type,
        } = storage.home
        else {
            return Err(invalid());
        };
        let array = self.frame_array(depth, place).ok_or_else(invalid)?;
        let count = super::byte_array_length(&self.program.structural_types, structural_type)
            .ok_or_else(invalid)?;
        if array.structural_type != structural_type
            || u64::try_from(array.elements.len()).ok() != Some(count)
            || !storage.referent.path.is_empty()
            || !matches!(
                self.program
                    .structural_types
                    .get(&structural_type)
                    .map(|declaration| &declaration.shape),
                Some(terminal_psi::StructuralTypeShape::FixedArray { .. })
            )
        {
            return Err(invalid());
        }
        Ok((0, array.elements.len()))
    }

    pub(crate) fn stored_byte_array_length(
        &self,
        storage: &ArrayStorage,
    ) -> Result<usize, TerminalInterpretError> {
        match storage.home {
            ArrayStorageHome::Runtime => self
                .structural_byte_storage
                .get(&storage.referent)
                .map(ByteSequenceView::len)
                .ok_or(TerminalInterpretError::VerifiedOperationMalformed),
            ArrayStorageHome::Frame { .. } => self
                .frame_byte_array_window(storage)
                .map(|(start, end)| end - start),
        }
    }

    pub(crate) fn stored_byte_array_read(
        &self,
        storage: &ArrayStorage,
        index: usize,
    ) -> Result<u8, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        match storage.home {
            ArrayStorageHome::Runtime => self
                .structural_byte_storage
                .get(&storage.referent)
                .and_then(|bytes| bytes.get(index))
                .copied()
                .ok_or_else(invalid),
            ArrayStorageHome::Frame { depth, place, .. } => {
                let (start, end) = self.frame_byte_array_window(storage)?;
                let absolute = start
                    .checked_add(index)
                    .filter(|index| *index < end)
                    .ok_or_else(invalid)?;
                let value = self
                    .frame_array(depth, place)
                    .and_then(|array| array.elements.get(absolute))
                    .ok_or_else(invalid)?;
                scalar_byte(value)
            }
        }
    }

    pub(crate) fn stored_byte_array_write(
        &mut self,
        storage: &ArrayStorage,
        index: usize,
        byte: u8,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        match storage.home {
            ArrayStorageHome::Runtime => self
                .structural_byte_storage
                .get_mut(&storage.referent)
                .and_then(|bytes| bytes.replace_byte(index, byte))
                .ok_or_else(invalid),
            ArrayStorageHome::Frame { depth, place, .. } => {
                let (start, end) = self.frame_byte_array_window(storage)?;
                let absolute = start
                    .checked_add(index)
                    .filter(|index| *index < end)
                    .ok_or_else(invalid)?;
                let array = if depth == self.call_stack.len() {
                    self.scalar_array_values.get_mut(&place)
                } else {
                    self.call_stack
                        .get_mut(depth)
                        .and_then(|frame| frame.scalar_array_values.get_mut(&place))
                }
                .ok_or_else(invalid)?;
                let scalar_type =
                    IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| invalid())?;
                *array.elements.get_mut(absolute).ok_or_else(invalid)? =
                    TerminalScalarValue::Integer {
                        scalar_type,
                        value: IntegerValue::Unsigned(u128::from(byte)),
                    };
                Ok(())
            }
        }
    }

    pub(crate) fn stored_byte_array_snapshot(
        &self,
        storage: &ArrayStorage,
        start: usize,
        end: usize,
    ) -> Result<ByteSequenceView, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        if let ArrayStorageHome::Runtime = storage.home {
            return self
                .structural_byte_storage
                .get(&storage.referent)
                .and_then(|bytes| bytes.subslice(start, end))
                .ok_or_else(invalid);
        }
        if start > end || end > self.stored_byte_array_length(storage)? {
            return Err(invalid());
        }
        let ArrayStorageHome::Frame { depth, place, .. } = storage.home else {
            return Err(invalid());
        };
        self.frame_array(depth, place)
            .and_then(|array| array.elements.get(start..end))
            .ok_or_else(invalid)?
            .iter()
            .map(scalar_byte)
            .collect::<Result<Vec<_>, _>>()
            .map(ByteSequenceView::new)
    }
}

fn scalar_byte(value: &TerminalScalarValue) -> Result<u8, TerminalInterpretError> {
    let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
    let TerminalScalarValue::Integer {
        scalar_type,
        value: IntegerValue::Unsigned(byte),
    } = value
    else {
        return Err(invalid());
    };
    if scalar_type.sign() != IntegerSign::Unsigned
        || scalar_type.bits() != 8
        || scalar_type.is_address()
    {
        return Err(invalid());
    }
    u8::try_from(*byte).map_err(|_| invalid())
}
