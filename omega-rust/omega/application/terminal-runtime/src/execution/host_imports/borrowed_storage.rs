//! Join semantic loans to native placement without inventing backing bytes.
//! Output observes only covered prefixes. Input uses private staging memory;
//! its zeroed holes are not semantic values: completion decodes a cell only
//! when every byte was supplied by the read or retained from valid entry
//! contents. This permits complete case replacement, including initially
//! inactive payloads, without publishing unread padding or manufactured data.

mod geometry;

use geometry::{NativeCell, NativeCellKind};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerValue, ScalarType};
use terminal_interpreter::{
    TerminalBoundaryStorageBuffer, TerminalBoundaryStorageValue, TerminalEffectRejection,
    TerminalScalarValue, TerminalStorageContents,
};
use terminal_psi::StructuralAccess;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StorageTransfer {
    Observe,
    ReplacePrefix,
}

pub(super) struct CapturedStorage<'layout> {
    pub bytes: Vec<u8>,
    covered: Vec<(usize, usize)>,
    cells: Vec<NativeCell<'layout>>,
    layout: Option<&'layout NativeBorrowedStorageLayout>,
}

fn invalid_operands() -> TerminalEffectRejection {
    TerminalEffectRejection::new(
        "borrowed native storage differs from its captured semantic layout",
    )
}

impl<'layout> CapturedStorage<'layout> {
    pub fn capture(
        buffers: &[TerminalBoundaryStorageBuffer],
        access: StructuralAccess,
        layout: Option<&'layout NativeBorrowedStorageLayout>,
        byte_count: usize,
        transfer: StorageTransfer,
    ) -> Result<Self, TerminalEffectRejection> {
        let [buffer] = buffers else {
            return Err(invalid_operands());
        };
        let mut cells = match layout {
            Some(layout) => geometry::expand(layout)?,
            None => Vec::new(),
        };
        // Terminal canonical order sorts each declaration's fields by identity.
        // Lexical paths therefore place tags before descendants and mixed common
        // fields before payloads, matching semantic capture without dense IDs.
        cells.sort_by(|left, right| left.path.cmp(&right.path));
        if cells.windows(2).any(|pair| pair[0].path == pair[1].path) {
            return Err(invalid_operands());
        }
        let (argument_index, actual_access, values) = match buffer {
            TerminalBoundaryStorageBuffer::Scalar(buffer) if layout.is_none() => (
                buffer.argument_index(),
                buffer.access(),
                vec![TerminalBoundaryStorageValue {
                    path: buffer.field_path().to_vec(),
                    contents: TerminalStorageContents::Scalar {
                        value: buffer.value(),
                        stored_scalar: buffer.storage_scalar_type(),
                    },
                }],
            ),
            TerminalBoundaryStorageBuffer::Structural(buffer)
                if layout
                    .is_some_and(|layout| layout.structural_type == buffer.structural_type()) =>
            {
                (
                    buffer.argument_index(),
                    buffer.access(),
                    buffer.values().to_vec(),
                )
            }
            _ => return Err(invalid_operands()),
        };
        if argument_index != 0
            || actual_access != access
            || (transfer == StorageTransfer::ReplacePrefix
                && access != StructuralAccess::MutableBorrow)
        {
            return Err(invalid_operands());
        }
        let mut encoded = Vec::with_capacity(values.len());
        for value in &values {
            let (offset, bytes) = if let Some(layout) = layout {
                let position = cells
                    .binary_search_by(|cell| cell.path.cmp(&value.path))
                    .map_err(|_| invalid_operands())?;
                let cell = &cells[position];
                (cell.byte_offset, cell.encode(layout, value.contents)?)
            } else {
                if values.len() != 1 || !value.path.is_empty() {
                    return Err(invalid_operands());
                }
                let TerminalStorageContents::Scalar {
                    value,
                    stored_scalar,
                } = value.contents
                else {
                    return Err(invalid_operands());
                };
                (
                    0,
                    encode(value, stored_scalar)?[..scalar_extent(stored_scalar)?].to_vec(),
                )
            };
            encoded.push((offset, bytes));
        }
        encoded.sort_by_key(|(offset, _)| *offset);
        let mut covered = Vec::with_capacity(encoded.len());
        let mut previous_end = 0;
        for (offset, bytes) in &encoded {
            let end = offset
                .checked_add(bytes.len())
                .ok_or_else(invalid_operands)?;
            if *offset < previous_end
                || layout.is_some_and(|layout| end > layout.byte_size as usize)
            {
                return Err(invalid_operands());
            }
            covered.push((*offset, end));
            previous_end = end;
        }
        let native_extent = layout.map_or(previous_end, |layout| layout.byte_size as usize);
        if byte_count > native_extent {
            return Err(TerminalEffectRejection::new(
                "descriptor byte count exceeds the exact borrowed storage extent",
            ));
        }
        if transfer == StorageTransfer::Observe && !covered_range(&covered, 0, byte_count) {
            return Err(TerminalEffectRejection::new(
                "borrowed storage has padding, overlap or unmodeled bytes",
            ));
        }
        if transfer == StorageTransfer::ReplacePrefix {
            let mut possible = match layout {
                Some(layout) => geometry::input_coverage(layout, &values, &cells, byte_count)?,
                None => covered.clone(),
            };
            possible.sort_unstable();
            if !covered_range(&possible, 0, byte_count) {
                return Err(TerminalEffectRejection::new(
                    "input accesses padding or bytes without a scalar or discriminator recipe",
                ));
            }
        }
        let mut bytes = vec![0; previous_end.max(byte_count)];
        for (offset, encoded) in encoded {
            bytes[offset..offset + encoded.len()].copy_from_slice(&encoded);
        }
        Ok(Self {
            bytes,
            covered,
            cells,
            layout,
        })
    }

    /// The returned count grants initialization only to the supplied prefix.
    /// A short read retains valid old bytes; unread inactive bytes stay absent.
    pub fn stage(
        &self,
        buffers: &mut [TerminalBoundaryStorageBuffer],
        written: usize,
    ) -> Result<(), TerminalEffectRejection> {
        if written > self.bytes.len() {
            return Err(invalid_operands());
        }
        let [buffer] = buffers else {
            return Err(invalid_operands());
        };
        let mut covered = self.covered.clone();
        covered.push((0, written));
        covered.sort_unstable();
        if let TerminalBoundaryStorageBuffer::Scalar(buffer) = buffer {
            let size = scalar_extent(buffer.storage_scalar_type())?;
            if !covered_range(&covered, 0, size) {
                return Err(invalid_operands());
            }
            return buffer.replace(decode(
                &self.bytes[..size],
                buffer.storage_scalar_type(),
                buffer.value().scalar_type(),
            )?);
        }
        let TerminalBoundaryStorageBuffer::Structural(buffer) = buffer else {
            return Err(invalid_operands());
        };
        let layout = self.layout.ok_or_else(invalid_operands)?;
        let mut values: Vec<TerminalBoundaryStorageValue> = Vec::new();
        for cell in &self.cells {
            if !geometry::is_active(&cell.path, &values)? {
                continue;
            }
            let size = cell.byte_size()?;
            let end = cell
                .byte_offset
                .checked_add(size)
                .ok_or_else(invalid_operands)?;
            if !covered_range(&covered, cell.byte_offset, end) {
                // Non-scalar authority (e.g. shared atomic residents) is not a
                // field supplied by a native layout. Mutable snapshots capture
                // all ordinary/atomic leaves; a newly selected case must have
                // complete bytes rather than inheriting an inactive zero fill.
                return Err(TerminalEffectRejection::new(
                    "new active case has bytes neither supplied nor validly retained",
                ));
            }
            let contents = match cell.kind {
                NativeCellKind::Scalar(field) => TerminalStorageContents::Scalar {
                    value: decode(
                        self.bytes
                            .get(cell.byte_offset..end)
                            .ok_or_else(invalid_operands)?,
                        field.stored_scalar,
                        field.logical_scalar,
                    )?,
                    stored_scalar: field.stored_scalar,
                },
                NativeCellKind::Case(discriminator) => {
                    let encoded: [u8; 4] = self
                        .bytes
                        .get(cell.byte_offset..end)
                        .ok_or_else(invalid_operands)?
                        .try_into()
                        .map_err(|_| invalid_operands())?;
                    let tag = i32::from_le_bytes(encoded);
                    let case = layout
                        .discriminator_cases(discriminator)
                        .find(|mapping| mapping.native_tag == tag)
                        .ok_or_else(|| {
                            TerminalEffectRejection::new(
                                "native discriminator does not name a declared case",
                            )
                        })?;
                    TerminalStorageContents::Case(case.case)
                }
            };
            values.push(TerminalBoundaryStorageValue {
                path: cell.path.clone(),
                contents,
            });
        }
        buffer.replace(values)
    }
}

fn covered_range(covered: &[(usize, usize)], start: usize, end: usize) -> bool {
    let mut frontier = start;
    for &(begin, limit) in covered {
        if begin > frontier {
            break;
        }
        frontier = frontier.max(limit);
        if frontier >= end {
            return true;
        }
    }
    frontier >= end
}

pub(super) fn scalar_extent(scalar: ScalarType) -> Result<usize, TerminalEffectRejection> {
    match scalar {
        ScalarType::Boolean => Ok(1),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => Ok(4),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => Ok(8),
        ScalarType::Integer(integer)
            if !integer.is_address()
                && integer.bits() != 0
                && integer.bits() <= 128
                && integer.bits() % 8 == 0 =>
        {
            Ok(usize::from(integer.bits() / 8))
        }
        _ => Err(invalid_operands()),
    }
}

fn encode(
    value: TerminalScalarValue,
    storage: ScalarType,
) -> Result<[u8; 16], TerminalEffectRejection> {
    let mut bytes = [0; 16];
    match (value, storage) {
        (TerminalScalarValue::Boolean(value), ScalarType::Boolean) => bytes[0] = u8::from(value),
        (
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(bits)),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        ) => bytes[..4].copy_from_slice(&bits.to_le_bytes()),
        (
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(bits)),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
        ) => bytes[..8].copy_from_slice(&bits.to_le_bytes()),
        (TerminalScalarValue::Integer { scalar_type, value }, ScalarType::Integer(stored))
            if !scalar_type.is_address()
                && scalar_type.bits() >= stored.bits()
                && scalar_type.admits(value) =>
        {
            let value = integer_value_with_sign(value, stored.sign())?;
            if !stored.admits(value) {
                return Err(invalid_operands());
            }
            bytes = match value {
                IntegerValue::Signed(value) => value.to_le_bytes(),
                IntegerValue::Unsigned(value) => value.to_le_bytes(),
            };
        }
        _ => return Err(invalid_operands()),
    }
    Ok(bytes)
}

fn decode(
    bytes: &[u8],
    storage: ScalarType,
    logical: ScalarType,
) -> Result<TerminalScalarValue, TerminalEffectRejection> {
    if bytes.len() != scalar_extent(storage)? {
        return Err(invalid_operands());
    }
    match (storage, logical) {
        (ScalarType::Boolean, ScalarType::Boolean) => match bytes {
            [0] => Ok(TerminalScalarValue::Boolean(false)),
            [1] => Ok(TerminalScalarValue::Boolean(true)),
            _ => Err(invalid_operands()),
        },
        (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        ) => Ok(TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(
            u32::from_le_bytes(bytes.try_into().map_err(|_| invalid_operands())?),
        ))),
        (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
        ) => Ok(TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(
            u64::from_le_bytes(bytes.try_into().map_err(|_| invalid_operands())?),
        ))),
        (ScalarType::Integer(stored), ScalarType::Integer(integer))
            if integer.bits() >= stored.bits() && !integer.is_address() =>
        {
            let mut padded = [0; 16];
            padded[..bytes.len()].copy_from_slice(bytes);
            let raw = u128::from_le_bytes(padded);
            let value = match stored.sign() {
                IntegerSign::Unsigned => IntegerValue::Unsigned(raw),
                IntegerSign::Signed => IntegerValue::Signed(
                    ((raw << (128 - stored.bits())) as i128) >> (128 - stored.bits()),
                ),
            };
            let value = integer_value_with_sign(value, integer.sign())?;
            if !integer.admits(value) {
                return Err(invalid_operands());
            }
            Ok(TerminalScalarValue::Integer {
                scalar_type: integer,
                value,
            })
        }
        _ => Err(invalid_operands()),
    }
}

fn integer_value_with_sign(
    value: IntegerValue,
    sign: IntegerSign,
) -> Result<IntegerValue, TerminalEffectRejection> {
    match (sign, value) {
        (IntegerSign::Unsigned, IntegerValue::Signed(value)) => Ok(IntegerValue::Unsigned(
            u128::try_from(value).map_err(|_| invalid_operands())?,
        )),
        (IntegerSign::Signed, IntegerValue::Unsigned(value)) => Ok(IntegerValue::Signed(
            i128::try_from(value).map_err(|_| invalid_operands())?,
        )),
        (_, value) => Ok(value),
    }
}
