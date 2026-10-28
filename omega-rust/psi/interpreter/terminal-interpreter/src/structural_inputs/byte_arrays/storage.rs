//! Array loans retain one backing home across suspended calls. Frame homes
//! address the owner's payload directly; no second byte store or copyback exists.
//! Verified loans cannot outlive their owner, so a suspended frame's stack depth
//! remains stable for the lifetime of the loan, including nested forwarding.

use crate::byte_sequences::ByteSequenceView;
use crate::byte_sequences::storage::ByteSequenceStorage;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::scalar_array::TerminalScalarArrayValue;
use crate::values::{StructuralRuntimePlace, TerminalScalarValue};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, PlaceId, StructuralTypeId};

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum ArrayStorageHome {
    Runtime,
    Frame {
        depth: usize,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
}

#[derive(Clone, PartialEq, Eq)]
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

impl TerminalExecution<'_> {
    pub(crate) fn stored_byte_array_read_view(
        &self,
        storage: &ArrayStorage,
        start: usize,
        end: usize,
    ) -> Result<crate::byte_sequences::read::TerminalByteRead<'_>, TerminalInterpretError> {
        use crate::byte_sequences::read::TerminalByteRead;
        match storage.home {
            ArrayStorageHome::Runtime => {
                let Some(bytes) = self.structural_byte_storage.get(&storage.referent) else {
                    if start == 0
                        && end == 0
                        && self.initialized_empty_byte_array(&storage.referent)?
                    {
                        return Ok(TerminalByteRead::contiguous(&[]));
                    }
                    return Err(malformed());
                };
                let window = bytes.bytes().get(start..end).ok_or_else(malformed)?;
                Ok(TerminalByteRead::contiguous(window))
            }
            ArrayStorageHome::Frame { depth, place, .. } => {
                let (base, limit) = self.frame_byte_array_window(storage)?;
                if start > end || end > limit - base {
                    return Err(malformed());
                }
                let values = self.frame_array(depth, place).ok_or_else(malformed)?;
                TerminalByteRead::scalars(&values.elements()[base + start..base + end])
            }
        }
    }

    pub(crate) fn frame_array(
        &self,
        depth: usize,
        place: PlaceId,
    ) -> Option<&TerminalScalarArrayValue> {
        if depth == self.call_stack.len() {
            self.scalar_array_values.backing(&place)
        } else {
            self.call_stack
                .get(depth)?
                .scalar_array_values
                .backing(&place)
        }
    }

    /// A frame-owned fixed byte array has one payload, not an opaque projected
    /// host root. Admission currently permits its whole backing plus byte ranges.
    fn frame_byte_array_window(
        &self,
        storage: &ArrayStorage,
    ) -> Result<(usize, usize), TerminalInterpretError> {
        let ArrayStorageHome::Frame {
            depth,
            place,
            structural_type,
        } = storage.home
        else {
            return Err(malformed());
        };
        let array = self.frame_array(depth, place).ok_or_else(malformed)?;
        let types = self.program.structural_types();
        let (scalar, count) =
            terminal_semantics::scalar_array_leaf_shape(types.values(), structural_type)
                .ok_or_else(malformed)?;
        if array.structural_type != structural_type
            || u64::try_from(array.elements().len()).ok() != Some(count)
            || !matches!(scalar, semantic_vocabulary::ScalarType::Integer(integer) if integer.bits() == 8 && !integer.is_address() && integer.sign() == IntegerSign::Unsigned)
        {
            return Err(malformed());
        }
        let mut selected = structural_type;
        let mut offset = 0usize;
        for segment in &storage.referent.path {
            let terminal_psi::StructuralPathSegment::FixedIndex(index) = segment else {
                return Err(malformed());
            };
            let terminal_psi::StructuralTypeShape::FixedArray { element, length } =
                types.get(&selected).ok_or_else(malformed)?.shape
            else {
                return Err(malformed());
            };
            if *index >= length {
                return Err(malformed());
            }
            let child_count = match types.get(&element).ok_or_else(malformed)?.shape {
                terminal_psi::StructuralTypeShape::PrimitiveScalar(_) => 1,
                _ => {
                    terminal_semantics::scalar_array_leaf_shape(types.values(), element)
                        .ok_or_else(malformed)?
                        .1
                }
            };
            offset = offset
                .checked_add(
                    usize::try_from(index.checked_mul(child_count).ok_or_else(malformed)?)
                        .map_err(|_| malformed())?,
                )
                .ok_or_else(malformed)?;
            selected = element;
        }
        let length =
            usize::try_from(super::byte_array_length(&types, selected).ok_or_else(malformed)?)
                .map_err(|_| malformed())?;
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= array.elements().len())
            .ok_or_else(malformed)?;
        Ok((offset, end))
    }

    pub(crate) fn stored_byte_array_length(
        &self,
        storage: &ArrayStorage,
    ) -> Result<usize, TerminalInterpretError> {
        match storage.home {
            ArrayStorageHome::Runtime => {
                if let Some(bytes) = self.structural_byte_storage.get(&storage.referent) {
                    Ok(bytes.len())
                } else if self.initialized_empty_byte_array(&storage.referent)? {
                    Ok(0)
                } else {
                    Err(malformed())
                }
            }
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
        match storage.home {
            ArrayStorageHome::Runtime => self
                .structural_byte_storage
                .get(&storage.referent)
                .and_then(|bytes| bytes.get(index))
                .copied()
                .ok_or_else(malformed),
            ArrayStorageHome::Frame { depth, place, .. } => {
                let (start, end) = self.frame_byte_array_window(storage)?;
                let absolute = start
                    .checked_add(index)
                    .filter(|index| *index < end)
                    .ok_or_else(malformed)?;
                let value = self
                    .frame_array(depth, place)
                    .and_then(|array| array.elements().get(absolute))
                    .ok_or_else(malformed)?;
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
        let result = match storage.home {
            ArrayStorageHome::Runtime => self
                .structural_byte_storage
                .get_mut(&storage.referent)
                .ok_or_else(malformed)?
                .replace_byte(index, byte),
            ArrayStorageHome::Frame { depth, place, .. } => {
                let (start, end) = self.frame_byte_array_window(storage)?;
                let absolute = start
                    .checked_add(index)
                    .filter(|index| *index < end)
                    .ok_or_else(malformed)?;
                let array = if depth == self.call_stack.len() {
                    self.scalar_array_values.backing_mut(&place)
                } else {
                    self.call_stack
                        .get_mut(depth)
                        .and_then(|frame| frame.scalar_array_values.backing_mut(&place))
                }
                .ok_or_else(malformed)?;
                let scalar_type =
                    IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
                *array
                    .elements_mut()?
                    .get_mut(absolute)
                    .ok_or_else(malformed)? = TerminalScalarValue::Integer {
                    scalar_type,
                    value: IntegerValue::Unsigned(u128::from(byte)),
                };
                Ok(())
            }
        };
        if result.is_ok() {
            self.invalidate_stored_provenance(&storage.referent);
        }
        result
    }

    pub(crate) fn stored_byte_array_snapshot(
        &self,
        storage: &ArrayStorage,
        start: usize,
        end: usize,
    ) -> Result<ByteSequenceView, TerminalInterpretError> {
        if let ArrayStorageHome::Runtime = storage.home {
            if !self.structural_byte_storage.contains_key(&storage.referent)
                && start == 0
                && end == 0
                && self.initialized_empty_byte_array(&storage.referent)?
            {
                return Ok(ByteSequenceView::new(ByteSequenceStorage::from_values(
                    0,
                    std::iter::empty(),
                    self.account.as_ref(),
                )?));
            }
            return self
                .structural_byte_storage
                .get(&storage.referent)
                .and_then(|bytes| bytes.subslice(start, end))
                .ok_or_else(malformed);
        }
        if start > end || end > self.stored_byte_array_length(storage)? {
            return Err(malformed());
        }
        let ArrayStorageHome::Frame { depth, place, .. } = storage.home else {
            return Err(malformed());
        };
        let (base, _) = self.frame_byte_array_window(storage)?;
        let values = self
            .frame_array(depth, place)
            .and_then(|array| array.elements().get(base + start..base + end))
            .ok_or_else(malformed)?
            .iter()
            .map(scalar_byte);
        Ok(ByteSequenceView::new(ByteSequenceStorage::from_values(
            end - start,
            values,
            self.account.as_ref(),
        )?))
    }

    pub(crate) fn initialized_empty_byte_array(
        &self,
        home: &StructuralRuntimePlace,
    ) -> Result<bool, TerminalInterpretError> {
        self.empty_array_storage.initialized_byte_array(
            &self.program.structural_types(),
            &self.structural_byte_storage,
            home,
        )
    }
}

pub(crate) fn scalar_byte(value: &TerminalScalarValue) -> Result<u8, TerminalInterpretError> {
    let TerminalScalarValue::Integer {
        scalar_type,
        value: IntegerValue::Unsigned(byte),
    } = value
    else {
        return Err(malformed());
    };
    if scalar_type.sign() != IntegerSign::Unsigned
        || scalar_type.bits() != 8
        || scalar_type.is_address()
    {
        return Err(malformed());
    }
    u8::try_from(*byte).map_err(|_| malformed())
}
