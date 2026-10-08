//! A demanded native byte prefix assembled from authenticated scalar loans.
//! Native placement selects byte coordinates; captured backing selects access,
//! codecs and field restrictions. Neither can substitute for the other. Every
//! accessed byte must belong to exactly one scalar: padding and pointers are not
//! invented. Unaccessed padding is not a reason to refuse a backed prefix. Keep
//! a complete captured scalar when the prefix ends inside it, so writeback can
//! preserve its unread bytes; fields outside the prefix receive no replacement.

use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerValue,
    ScalarType,
};
use terminal_interpreter::{
    TerminalBoundaryScalarBuffer, TerminalEffectRejection, TerminalScalarValue,
};
use terminal_psi::StructuralAccess;

pub(super) struct CapturedStorage {
    pub bytes: Vec<u8>,
    segments: Vec<(usize, usize, usize)>,
}

fn invalid_operands() -> TerminalEffectRejection {
    TerminalEffectRejection::new("borrowed native storage differs from its captured scalar layout")
}

impl CapturedStorage {
    pub fn capture(
        buffers: &[TerminalBoundaryScalarBuffer],
        access: StructuralAccess,
        layout: Option<&NativeBorrowedStorageLayout>,
        byte_count: usize,
    ) -> Result<Self, TerminalEffectRejection> {
        let mut segments = Vec::with_capacity(buffers.len());
        for (buffer_index, buffer) in buffers.iter().enumerate() {
            if buffer.argument_index() != 0 || buffer.access() != access {
                return Err(invalid_operands());
            }
            let byte_size = scalar_extent(buffer.storage_scalar_type())?;
            let byte_offset = if let Some(layout) = layout {
                let mut matched = None;
                for field in layout.fields() {
                    let template = layout.field_path(field);
                    if template.len() != buffer.field_path().len() {
                        continue;
                    }
                    let dimensions = layout.field_repetitions(field);
                    let mut offset = u64::from(field.byte_offset);
                    let mut agrees = true;
                    for (path_position, (expected, actual)) in
                        template.zip(buffer.field_path()).enumerate()
                    {
                        if let Some(dimension) = dimensions
                            .iter()
                            .find(|dimension| dimension.path_position == path_position)
                        {
                            let CanonicalStructuralPathSegment::FixedIndex(element_index) = actual
                            else {
                                agrees = false;
                                break;
                            };
                            if *element_index >= dimension.length {
                                agrees = false;
                                break;
                            }
                            offset = offset
                                .checked_add(
                                    element_index
                                        .checked_mul(u64::from(dimension.byte_stride))
                                        .ok_or_else(invalid_operands)?,
                                )
                                .ok_or_else(invalid_operands)?;
                        } else if expected != *actual {
                            agrees = false;
                            break;
                        }
                    }
                    if agrees {
                        if matched.is_some() || field.stored_scalar != buffer.storage_scalar_type()
                        {
                            return Err(invalid_operands());
                        }
                        matched = Some(usize::try_from(offset).map_err(|_| invalid_operands())?);
                    }
                }
                matched.ok_or_else(invalid_operands)?
            } else {
                if buffers.len() != 1 || !buffer.field_path().is_empty() {
                    return Err(invalid_operands());
                }
                0
            };
            segments.push((buffer_index, byte_offset, byte_size));
        }
        segments.sort_unstable_by_key(|(_, offset, _)| *offset);
        let layout_extent = layout
            .map(|layout| usize::try_from(layout.byte_size).map_err(|_| invalid_operands()))
            .transpose()?;
        let mut previous_end = 0usize;
        for (_, offset, size) in &segments {
            let end = offset.checked_add(*size).ok_or_else(invalid_operands)?;
            if *offset < previous_end || layout_extent.is_some_and(|extent| end > extent) {
                return Err(invalid_operands());
            }
            previous_end = end;
        }
        let native_extent = layout_extent.unwrap_or(previous_end);
        if byte_count > native_extent {
            return Err(TerminalEffectRejection::new(
                "descriptor byte count exceeds the exact borrowed storage extent",
            ));
        }
        // Native pointer/count semantics demand this prefix, not a snapshot of
        // the entire object. Authenticate all supplied coordinates above, then
        // retain only complete scalars touched by the actual byte interval.
        segments.retain(|(_, offset, _)| *offset < byte_count);
        let mut extent = 0usize;
        for (_, offset, size) in &segments {
            if *offset != extent {
                return Err(TerminalEffectRejection::new(
                    "borrowed storage has padding, overlap or unmodeled bytes",
                ));
            }
            extent = extent.checked_add(*size).ok_or_else(invalid_operands)?;
        }
        if extent < byte_count {
            return Err(TerminalEffectRejection::new(
                "borrowed storage has padding, overlap or unmodeled bytes",
            ));
        }
        let mut bytes = vec![0; extent];
        for (buffer_index, offset, size) in &segments {
            let buffer = &buffers[*buffer_index];
            bytes[*offset..*offset + *size]
                .copy_from_slice(&encode(buffer.value(), buffer.storage_scalar_type())?[..*size]);
        }
        Ok(Self { bytes, segments })
    }

    pub fn stage(
        &self,
        buffers: &mut [TerminalBoundaryScalarBuffer],
    ) -> Result<(), TerminalEffectRejection> {
        // Decode every payload before staging replacements; the interpreter
        // then validates every original endpoint before committing any store.
        let replacements = self
            .segments
            .iter()
            .map(|(buffer_index, offset, size)| {
                let buffer = &buffers[*buffer_index];
                decode(
                    &self.bytes[*offset..*offset + *size],
                    buffer.storage_scalar_type(),
                    buffer.value().scalar_type(),
                )
                .map(|value| (*buffer_index, value))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (buffer_index, value) in replacements {
            buffers[buffer_index].replace(value)?;
        }
        Ok(())
    }
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
