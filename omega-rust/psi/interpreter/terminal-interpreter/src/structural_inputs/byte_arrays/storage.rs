//! Array loans retain one backing home across suspended calls. Frame homes
//! address the owner's payload directly; no second byte store or copyback exists.
//! Verified loans cannot outlive their owner, so a suspended frame's stack depth
//! remains stable for the lifetime of the loan, including nested forwarding.

use crate::byte_sequences::ByteSequenceView;
use crate::byte_sequences::storage::ByteSequenceStorage;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::execution::frame_field_mut;
use crate::scalar_array::PackedScalarArrayValue;
use crate::scalar_storage::{ScalarCell, TerminalScalarBacking};
use crate::values::{StructuralRuntimePlace, TerminalScalarValue};
use evaluation_accounting::EvaluationInvocationAccount;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, PlaceId, StructuralTypeId};
use std::rc::Rc;

/// One fixed-array payload at its existing structural storage key. Byte views
/// stay compact until original residence requires actual mutable scalar cells.
#[derive(Clone)]
pub(crate) struct StructuralByteArrayStorage {
    pub(crate) structural_type: StructuralTypeId,
    backing: StructuralByteArrayBacking,
    account: Option<EvaluationInvocationAccount>,
}

#[derive(Clone)]
enum StructuralByteArrayBacking {
    Local(ByteSequenceView),
    Cells(PackedScalarArrayValue),
}

pub(crate) struct PreparedStructuralByteArrayReplacement {
    retained: StructuralByteArrayStorage,
    values: ByteSequenceView,
}

impl PreparedStructuralByteArrayReplacement {
    pub(crate) fn commit(mut self) -> Result<StructuralByteArrayStorage, TerminalInterpretError> {
        for (index, byte) in self.values.bytes().iter().enumerate() {
            self.retained.replace_byte(index, *byte)?;
        }
        Ok(self.retained)
    }
}

impl StructuralByteArrayStorage {
    pub(crate) fn new(
        structural_type: StructuralTypeId,
        bytes: ByteSequenceView,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Self {
        Self {
            structural_type,
            backing: StructuralByteArrayBacking::Local(bytes),
            account: account.cloned(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        match &self.backing {
            StructuralByteArrayBacking::Local(bytes) => bytes.len(),
            StructuralByteArrayBacking::Cells(cells) => cells.len(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn local_view(&self) -> Option<&ByteSequenceView> {
        match &self.backing {
            StructuralByteArrayBacking::Local(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub(crate) fn local_view_mut(&mut self) -> Option<&mut ByteSequenceView> {
        match &mut self.backing {
            StructuralByteArrayBacking::Local(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub(crate) fn packed(&self) -> Option<&PackedScalarArrayValue> {
        match &self.backing {
            StructuralByteArrayBacking::Cells(cells) => Some(cells),
            _ => None,
        }
    }

    pub(crate) fn measure_result_custody<'storage>(
        &'storage self,
        custody: &mut crate::result_custody::ResultCustody<'storage>,
    ) -> Result<(), TerminalInterpretError> {
        match &self.backing {
            StructuralByteArrayBacking::Local(bytes) => custody.bytes(bytes),
            StructuralByteArrayBacking::Cells(cells) => custody.add_cells(cells.len()),
        }
    }

    pub(crate) fn has_residents(&self) -> bool {
        self.packed().is_some_and(|cells| {
            cells
                .cells()
                .iter()
                .any(|cell| matches!(cell, ScalarCell::Resident { .. }))
        })
    }

    /// Whole guest replacement keeps every established original endpoint.
    /// Freeze incoming guest values and fund COW before publishing any write.
    pub(crate) fn prepare_replacement(
        &self,
        incoming: &Self,
    ) -> Result<Option<PreparedStructuralByteArrayReplacement>, TerminalInterpretError> {
        if !self.has_residents() {
            return Ok(None);
        }
        if self.structural_type != incoming.structural_type || self.len() != incoming.len() {
            return Err(malformed());
        }
        let (index, resident) = self
            .packed()
            .ok_or_else(malformed)?
            .cells()
            .iter()
            .enumerate()
            .find(|(_, cell)| matches!(cell, ScalarCell::Resident { .. }))
            .ok_or_else(malformed)?;
        let resident = resident.clone();
        let mut retained = self.clone();
        let account = retained.account.clone();
        retained.prepare_resident_install(index, &resident, account.as_ref())?;
        Ok(Some(PreparedStructuralByteArrayReplacement {
            retained,
            values: incoming.copy_snapshot()?,
        }))
    }

    pub(crate) fn read_view(&self) -> Result<crate::TerminalByteRead<'_>, TerminalInterpretError> {
        self.read_window(0, self.len())
    }

    pub(crate) fn read_window(
        &self,
        start: usize,
        end: usize,
    ) -> Result<crate::TerminalByteRead<'_>, TerminalInterpretError> {
        match &self.backing {
            StructuralByteArrayBacking::Local(bytes) => Ok(crate::TerminalByteRead::contiguous(
                bytes.bytes().get(start..end).ok_or_else(malformed)?,
            )),
            StructuralByteArrayBacking::Cells(cells) => {
                crate::TerminalByteRead::cells(cells.cells().get(start..end).ok_or_else(malformed)?)
            }
        }
    }

    pub(crate) fn read(&self, index: usize) -> Result<u8, TerminalInterpretError> {
        match &self.backing {
            StructuralByteArrayBacking::Local(bytes) => {
                bytes.get(index).copied().ok_or_else(malformed)
            }
            StructuralByteArrayBacking::Cells(cells) => {
                scalar_byte(&cells.element(index)?.ok_or_else(malformed)?)
            }
        }
    }

    /// A genuine guest value copy, never a physical-home alias or writeback.
    pub(crate) fn copy_snapshot(&self) -> Result<ByteSequenceView, TerminalInterpretError> {
        Ok(ByteSequenceView::new(ByteSequenceStorage::from_values(
            self.len(),
            (0..self.len()).map(|index| self.read(index)),
            self.account.as_ref(),
        )?))
    }

    pub(crate) fn relocated(&self, transfer: bool) -> Result<Self, TerminalInterpretError> {
        if transfer {
            return Ok(self.clone());
        }
        Ok(Self::new(
            self.structural_type,
            self.copy_snapshot()?,
            self.account.as_ref(),
        ))
    }

    pub(crate) fn replace_byte(
        &mut self,
        index: usize,
        byte: u8,
    ) -> Result<(), TerminalInterpretError> {
        match &mut self.backing {
            StructuralByteArrayBacking::Local(bytes) => bytes.replace_byte(index, byte),
            StructuralByteArrayBacking::Cells(cells) => {
                cells.set_element(index, byte_scalar(byte)?)
            }
        }
    }

    pub(crate) fn capture_scalar(
        &self,
        index: usize,
    ) -> Result<(TerminalScalarValue, Option<Rc<dyn TerminalScalarBacking>>), TerminalInterpretError>
    {
        match &self.backing {
            StructuralByteArrayBacking::Local(bytes) => {
                Ok((byte_scalar(*bytes.get(index).ok_or_else(malformed)?)?, None))
            }
            StructuralByteArrayBacking::Cells(cells) => {
                let cell = cells.cell(index).ok_or_else(malformed)?;
                Ok((
                    cell.read()?,
                    match cell {
                        ScalarCell::Local(_) => None,
                        ScalarCell::Resident { backing, .. } => Some(backing.clone()),
                    },
                ))
            }
        }
    }

    /// Promotion and COW are preflight: retain the old byte lease until the
    /// replacement cells have been funded, validated and completely prepared.
    pub(crate) fn prepare_resident_install(
        &mut self,
        index: usize,
        resident: &ScalarCell,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<(), TerminalInterpretError> {
        match &mut self.backing {
            StructuralByteArrayBacking::Local(bytes) => {
                let mut cells = PackedScalarArrayValue::from_bytes(
                    self.structural_type,
                    bytes.bytes(),
                    account,
                )?;
                cells.prepare_resident_install(index, resident)?;
                self.backing = StructuralByteArrayBacking::Cells(cells);
                self.account = account.cloned();
                Ok(())
            }
            StructuralByteArrayBacking::Cells(cells) => {
                cells.prepare_resident_install(index, resident)
            }
        }
    }

    pub(crate) fn install_resident(
        &mut self,
        index: usize,
        resident: ScalarCell,
    ) -> Result<(), TerminalInterpretError> {
        let StructuralByteArrayBacking::Cells(cells) = &mut self.backing else {
            return Err(malformed());
        };
        cells.install_resident(index, resident)
    }
}

fn byte_scalar(byte: u8) -> Result<TerminalScalarValue, TerminalInterpretError> {
    Ok(TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?,
        value: IntegerValue::Unsigned(u128::from(byte)),
    })
}

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
                bytes.read_window(start, end)
            }
            ArrayStorageHome::Frame { depth, place, .. } => {
                let (base, limit) = self.frame_byte_array_window(storage)?;
                if start > end || end > limit - base {
                    return Err(malformed());
                }
                let values = self.frame_array(depth, place).ok_or_else(malformed)?;
                TerminalByteRead::cells(&values.cells()[base + start..base + end])
            }
        }
    }

    pub(crate) fn frame_array(
        &self,
        depth: usize,
        place: PlaceId,
    ) -> Option<&PackedScalarArrayValue> {
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
    pub(crate) fn frame_byte_array_window(
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
            || u64::try_from(array.len()).ok() != Some(count)
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
            let terminal_psi::StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } = types.get(&selected).ok_or_else(malformed)?.shape
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
            .filter(|end| *end <= array.len())
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
                .ok_or_else(malformed)?
                .read(index),
            ArrayStorageHome::Frame { depth, place, .. } => {
                let (start, end) = self.frame_byte_array_window(storage)?;
                let absolute = start
                    .checked_add(index)
                    .filter(|index| *index < end)
                    .ok_or_else(malformed)?;
                let value = self
                    .frame_array(depth, place)
                    .ok_or_else(malformed)?
                    .element(absolute)?
                    .ok_or_else(malformed)?;
                scalar_byte(&value)
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
                let array = frame_field_mut!(self, depth, scalar_array_values)
                    .backing_mut(&place)
                    .ok_or_else(malformed)?;
                let scalar_type =
                    IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
                array.set_element(
                    absolute,
                    TerminalScalarValue::Integer {
                        scalar_type,
                        value: IntegerValue::Unsigned(u128::from(byte)),
                    },
                )?;
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
            let array = self
                .structural_byte_storage
                .get(&storage.referent)
                .ok_or_else(malformed)?;
            if start > end || end > array.len() {
                return Err(malformed());
            }
            return Ok(ByteSequenceView::new(ByteSequenceStorage::from_values(
                end.checked_sub(start).ok_or_else(malformed)?,
                (start..end).map(|index| array.read(index)),
                self.account.as_ref(),
            )?));
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
            .and_then(|array| array.cells().get(base + start..base + end))
            .ok_or_else(malformed)?
            .iter()
            .map(|cell| scalar_byte(&cell.read()?));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[derive(Debug)]
    struct OriginalByte(Cell<TerminalScalarValue>);
    impl TerminalScalarBacking for OriginalByte {
        fn read(&self) -> Result<TerminalScalarValue, crate::effects::TerminalEffectRejection> {
            Ok(self.0.get())
        }
        fn write(
            &self,
            value: TerminalScalarValue,
        ) -> Result<(), crate::effects::TerminalEffectRejection> {
            self.0.set(value);
            Ok(())
        }
    }

    #[test]
    fn native_promotion_and_whole_replacement_preserve_cells_but_guest_copies_do_not_alias() {
        let array_type = StructuralTypeId::new(1).unwrap();
        let make = |values: &[u8]| {
            StructuralByteArrayStorage::new(
                array_type,
                ByteSequenceView::new(ByteSequenceStorage::copy(values, None).unwrap()),
                None,
            )
        };
        let mut owner = make(&[7, 23]);
        let original = Rc::new(OriginalByte(Cell::new(byte_scalar(7).unwrap())));
        let backing: Rc<dyn TerminalScalarBacking> = original.clone();
        let resident = ScalarCell::resident(byte_scalar(7).unwrap(), backing.clone()).unwrap();
        owner.prepare_resident_install(0, &resident, None).unwrap();
        assert!(
            owner.capture_scalar(0).unwrap().1.is_none(),
            "preflight grants no resident yet"
        );
        owner.install_resident(0, resident).unwrap();
        let snapshot = owner.copy_snapshot().unwrap();
        let incoming = make(&[31, 41]);
        let prepared = owner.prepare_replacement(&incoming).unwrap().unwrap();
        assert_eq!(
            original.0.get(),
            byte_scalar(7).unwrap(),
            "staging cannot publish writes"
        );
        let replaced = prepared.commit().unwrap();
        assert!(Rc::ptr_eq(
            &replaced.capture_scalar(0).unwrap().1.unwrap(),
            &backing
        ));
        assert_eq!(original.0.get(), byte_scalar(31).unwrap());
        assert_eq!(replaced.read(0).unwrap(), 31);
        assert_eq!(replaced.read(1).unwrap(), 41);
        assert_eq!(owner.read(0).unwrap(), 31);
        assert_eq!(owner.read(1).unwrap(), 23);
        assert_eq!(snapshot.bytes(), &[7, 23]);
        original.0.set(byte_scalar(47).unwrap());
        assert_eq!(replaced.read_view().unwrap().read(0).unwrap(), 47);
        assert_eq!(owner.read(0).unwrap(), 47);
        assert_eq!(snapshot.bytes(), &[7, 23]);
    }
}
