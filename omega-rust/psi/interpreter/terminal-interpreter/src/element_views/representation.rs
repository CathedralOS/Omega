//! Element windows over the original byte allocation. Geometry is shared with
//! independent admission; only scalar encoding is executed here. Descriptor
//! copies retain the backing home and never introduce a copyback buffer.

use super::{ElementView, ElementViewBacking};
use crate::byte_sequences::{ByteSequenceView, binding::ByteSequenceBinding};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::runtime_rows::RuntimeRows;
use crate::structural_inputs::byte_arrays::storage::ArrayStorage;
use crate::values::{StructuralRuntimePlace, TerminalScalarValue};
use semantic_vocabulary::{
    IeeeFloatFormat, IntegerSign, IntegerType, IntegerValue, ScalarType, StructuralTypeId,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralPathSegment, StructuralTypeShape,
};
use terminal_semantics::{RepresentationElementLayout, RepresentationScalarLeaf, ScalarViewValue};

pub(super) struct RepresentationView {
    storage: ByteStorage,
    start: usize,
    pub(super) length: usize,
    layout: RepresentationElementLayout,
}

enum ByteStorage {
    Array {
        storage: ArrayStorage,
        offset: usize,
    },
    Immutable(ByteSequenceView),
}

impl RepresentationView {
    pub(super) fn retain_storage(&self, retained: &mut crate::structural_storage::RetainedStorage) {
        if let ByteStorage::Array { storage, .. } = &self.storage {
            retained.array_storage(storage);
        }
    }

    fn scalar_position<'a>(
        &'a self,
        view: &ElementView,
        index: usize,
        path: &[StructuralPathSegment],
        expected: ScalarType,
    ) -> Result<(usize, &'a RepresentationScalarLeaf), TerminalInterpretError> {
        if index >= view.len() {
            return Err(malformed());
        }
        let leaf = self
            .layout
            .leaves
            .iter()
            .find(|leaf| leaf.path == path && leaf.scalar_type == expected)
            .ok_or_else(malformed)?;
        let element = view
            .window
            .start
            .checked_add(index)
            .filter(|index| *index < self.length)
            .ok_or_else(malformed)?;
        let stride = usize::try_from(self.layout.size).map_err(|_| malformed())?;
        let offset = usize::try_from(leaf.offset).map_err(|_| malformed())?;
        let width = scalar_width(expected)?;
        if offset.checked_add(width).is_none_or(|end| end > stride) {
            return Err(malformed());
        }
        let position = element
            .checked_mul(stride)
            .and_then(|offset| self.start.checked_add(offset))
            .and_then(|position| position.checked_add(offset))
            .ok_or_else(malformed)?;
        Ok((position, leaf))
    }
}

impl TerminalExecution<'_> {
    pub(super) fn representation_view(
        &mut self,
        source: &StructuralArgument,
        collection: StructuralTypeId,
        element: StructuralTypeId,
        start: usize,
        end: usize,
    ) -> Result<RepresentationView, TerminalInterpretError> {
        let extent = terminal_semantics::representation_byte_extent(
            self.program.structural_types().values(),
            collection,
        )
        .and_then(|extent| usize::try_from(extent).ok())
        .ok_or_else(malformed)?;
        let layout = terminal_semantics::representation_element_layout(
            self.program.structural_types().values(),
            element,
        )
        .ok_or_else(malformed)?;
        let stride = usize::try_from(layout.size).map_err(|_| malformed())?;
        if start > end || end > extent || stride == 0 || (end - start) % stride != 0 {
            return Err(malformed());
        }
        let storage = if matches!(
            self.program
                .structural_types()
                .get(&collection)
                .map(|row| &row.shape),
            Some(StructuralTypeShape::ByteSequence(_))
        ) {
            if !source.path.is_empty() {
                return Err(malformed());
            }
            let value = self
                .structural_values
                .get(&source.place)
                .ok_or_else(malformed)?;
            if !value.qualifications.is_empty()
                || self.byte_sequence_length(source.place)? != extent as u64
            {
                return Err(malformed());
            }
            match self
                .byte_sequence_values
                .get(&source.place)
                .ok_or_else(malformed)?
            {
                binding @ ByteSequenceBinding::MutableArray { array, offset, .. } => {
                    binding.validate_mutable_referent(self, value)?;
                    ByteStorage::Array {
                        storage: array.clone(),
                        offset: usize::try_from(*offset).map_err(|_| malformed())?,
                    }
                }
                ByteSequenceBinding::Immutable(bytes)
                    if source.access == StructuralAccess::SharedBorrow =>
                {
                    ByteStorage::Immutable(bytes.clone())
                }
                _ => return Err(malformed()),
            }
        } else {
            let mut cursor = self.local_structural_identities.cursor();
            let (root, home) = self.array_storage_root_staged(
                source.place,
                &mut cursor,
                &mut RuntimeRows::new(self.account.as_ref()),
            )?;
            let values = RuntimeRows::from_rows(self.account.as_ref(), [(source.place, root)])?;
            let referent = crate::custody::resolve_single_structural_argument(
                &self.program.structural_types(),
                &values,
                &source,
            )?;
            if referent.structural_type != collection || !referent.qualifications.is_empty() {
                return Err(malformed());
            }
            let storage = ArrayStorage {
                referent: StructuralRuntimePlace::from(&referent),
                home,
            };
            if self.stored_byte_array_length(&storage)? != extent {
                return Err(malformed());
            }
            self.local_structural_identities.commit_cursor(cursor);
            ByteStorage::Array { storage, offset: 0 }
        };
        Ok(RepresentationView {
            storage,
            start,
            length: (end - start) / stride,
            layout,
        })
    }

    pub(super) fn read_representation_element(
        &self,
        view: &ElementView,
        index: usize,
        path: &[StructuralPathSegment],
        expected: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        let ElementViewBacking::Representation(representation) = &view.backing else {
            return Err(malformed());
        };
        let (position, _) = representation.scalar_position(view, index, path, expected)?;
        let mut bits = 0_u64;
        for byte_index in 0..scalar_width(expected)? {
            let index = position.checked_add(byte_index).ok_or_else(malformed)?;
            let byte = match &representation.storage {
                ByteStorage::Array { storage, offset } => self.stored_byte_array_read(
                    storage,
                    offset.checked_add(index).ok_or_else(malformed)?,
                )?,
                ByteStorage::Immutable(bytes) => *bytes.get(index).ok_or_else(malformed)?,
            };
            bits |= u64::from(byte) << (byte_index * 8);
        }
        decode_scalar(expected, bits)
    }

    pub(super) fn write_representation_element(
        &mut self,
        view: &ElementView,
        index: usize,
        path: &[StructuralPathSegment],
        scalar: TerminalScalarValue,
    ) -> Result<(), TerminalInterpretError> {
        if view.access != StructuralAccess::MutableBorrow {
            return Err(malformed());
        }
        let ElementViewBacking::Representation(representation) = &view.backing else {
            return Err(malformed());
        };
        let (position, _) =
            representation.scalar_position(view, index, path, scalar.scalar_type())?;
        let ByteStorage::Array { storage, offset } = &representation.storage else {
            return Err(malformed());
        };
        let position = offset.checked_add(position).ok_or_else(malformed)?;
        let width = scalar_width(scalar.scalar_type())?;
        let extent = self.stored_byte_array_length(storage)?;
        if position.checked_add(width).is_none_or(|end| end > extent) {
            return Err(malformed());
        }
        let bits = encode_scalar(scalar)?;
        for byte_index in 0..width {
            self.stored_byte_array_write(
                storage,
                position + byte_index,
                (bits >> (byte_index * 8)) as u8,
            )?;
        }
        Ok(())
    }
}

fn scalar_width(scalar: ScalarType) -> Result<usize, TerminalInterpretError> {
    match scalar {
        ScalarType::Integer(integer)
            if !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            Ok(usize::from(integer.bits() / 8))
        }
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => Ok(4),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => Ok(8),
        _ => Err(malformed()),
    }
}
fn bit_carrier(scalar: ScalarType) -> Result<ScalarType, TerminalInterpretError> {
    let bits = u16::try_from(scalar_width(scalar)? * 8).map_err(|_| malformed())?;
    Ok(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, bits).map_err(|_| malformed())?,
    ))
}

fn decode_scalar(
    scalar: ScalarType,
    bits: u64,
) -> Result<TerminalScalarValue, TerminalInterpretError> {
    let viewed = terminal_semantics::scalar_reference_view_value(
        bit_carrier(scalar)?,
        scalar,
        ScalarViewValue::Integer(IntegerValue::Unsigned(u128::from(bits))),
    )
    .ok_or_else(malformed)?;
    match (scalar, viewed) {
        (ScalarType::Integer(scalar_type), ScalarViewValue::Integer(value)) => {
            Ok(TerminalScalarValue::Integer { scalar_type, value })
        }
        (ScalarType::IeeeFloat(_), ScalarViewValue::IeeeFloat(value)) => {
            Ok(TerminalScalarValue::IeeeFloat(value))
        }
        _ => Err(malformed()),
    }
}

fn encode_scalar(scalar: TerminalScalarValue) -> Result<u64, TerminalInterpretError> {
    let value = match scalar {
        TerminalScalarValue::Integer { value, .. } => ScalarViewValue::Integer(value),
        TerminalScalarValue::IeeeFloat(value) => ScalarViewValue::IeeeFloat(value),
        _ => return Err(malformed()),
    };
    let viewed = terminal_semantics::scalar_reference_view_value(
        scalar.scalar_type(),
        bit_carrier(scalar.scalar_type())?,
        value,
    )
    .ok_or_else(malformed)?;
    let ScalarViewValue::Integer(IntegerValue::Unsigned(bits)) = viewed else {
        return Err(malformed());
    };
    u64::try_from(bits).map_err(|_| malformed())
}
