//! Exact direct ABI fragments, with no memory access beyond the declared width.
//!
//! R10 and X16 are native caller-clobbered scratch registers, outside the
//! canonical argument/result banks and the R11/X9 indirect address registers.
//! Partial stores preserve the source register; partial loads preserve the base.

use super::*;

const X86_SCRATCH: u8 = 10;
const AARCH64_SCRATCH: u8 = 16;

fn packed(width: u16) -> bool {
    matches!(width, 3 | 5 | 6 | 7)
}

fn last_offset(offset: u32, width: u16) -> Result<(), EmissionError> {
    offset
        .checked_add(u32::from(width) - 1)
        .map(|_| ())
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)
}

pub(super) fn x86_stack_load(
    bytes: &mut Vec<u8>,
    destination: u8,
    offset: u32,
    width: u16,
) -> Result<(), EmissionError> {
    x86_load(
        bytes,
        destination,
        offset,
        width,
        emit_x86_64_stack_load_width,
    )
}

pub(super) fn x86_memory_load(
    bytes: &mut Vec<u8>,
    destination: u8,
    base: u8,
    offset: u32,
    width: u16,
) -> Result<(), EmissionError> {
    if packed(width) && (base == X86_SCRATCH || base == destination) {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    x86_load(
        bytes,
        destination,
        offset,
        width,
        |bytes, register, offset, width| {
            emit_x86_64_memory_load_width(bytes, register, base, offset, width)
        },
    )
}

fn x86_load(
    bytes: &mut Vec<u8>,
    destination: u8,
    offset: u32,
    width: u16,
    mut load: impl FnMut(&mut Vec<u8>, u8, u32, u16) -> Result<(), EmissionError>,
) -> Result<(), EmissionError> {
    if !packed(width) {
        return load(bytes, destination, offset, width);
    }
    if destination == X86_SCRATCH {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    last_offset(offset, width)?;
    // Byte loads write a 32-bit destination and clear its upper bits. In
    // particular, packing does not rely on the legacy 16-bit MOVZX encoding.
    load(bytes, destination, offset, 1)?;
    for index in 1..width {
        load(bytes, X86_SCRATCH, offset + u32::from(index), 1)?;
        bytes.extend_from_slice(&[0x49, 0xc1, 0xe2, (index * 8) as u8]);
        bytes.extend_from_slice(&[
            0x4c | ((destination >> 3) & 1),
            0x09,
            0xd0 | (destination & 7),
        ]);
    }
    Ok(())
}

pub(super) fn x86_stack_store(
    bytes: &mut Vec<u8>,
    source: u8,
    offset: u32,
    width: u16,
) -> Result<(), EmissionError> {
    if !packed(width) {
        return emit_x86_64_stack_store_width(bytes, source, offset, width);
    }
    if source == X86_SCRATCH {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    last_offset(offset, width)?;
    bytes.extend_from_slice(&[
        0x49 | (((source >> 3) & 1) << 2),
        0x89,
        0xc2 | ((source & 7) << 3),
    ]);
    for index in 0..width {
        emit_x86_64_stack_store_width(bytes, X86_SCRATCH, offset + u32::from(index), 1)?;
        if index + 1 != width {
            bytes.extend_from_slice(&[0x49, 0xc1, 0xea, 8]);
        }
    }
    Ok(())
}

pub(super) fn aarch64_stack_load(
    instructions: &mut Vec<u32>,
    destination: u8,
    offset: u32,
    width: u16,
) -> Result<(), EmissionError> {
    aarch64_load(
        instructions,
        destination,
        offset,
        width,
        |register, offset, width| {
            aarch64_unit_stack_access(aarch64_load_base(width)?, register, offset, width)
        },
    )
}

pub(super) fn aarch64_memory_load(
    instructions: &mut Vec<u32>,
    destination: u8,
    base: u8,
    offset: u32,
    width: u16,
) -> Result<(), EmissionError> {
    if packed(width) && (base == AARCH64_SCRATCH || base == destination) {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    aarch64_load(
        instructions,
        destination,
        offset,
        width,
        |register, offset, width| {
            aarch64_unit_memory_access(aarch64_load_base(width)?, register, base, offset, width)
        },
    )
}

fn aarch64_load(
    instructions: &mut Vec<u32>,
    destination: u8,
    offset: u32,
    width: u16,
    mut load: impl FnMut(u8, u32, u16) -> Result<u32, EmissionError>,
) -> Result<(), EmissionError> {
    if !packed(width) {
        instructions.push(load(destination, offset, width)?);
        return Ok(());
    }
    if destination == AARCH64_SCRATCH {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    last_offset(offset, width)?;
    instructions.push(load(destination, offset, 1)?);
    for index in 1..width {
        instructions.push(load(AARCH64_SCRATCH, offset + u32::from(index), 1)?);
        instructions.push(
            0xaa00_0000
                | (u32::from(AARCH64_SCRATCH) << 16)
                | (u32::from(index * 8) << 10)
                | (u32::from(destination) << 5)
                | u32::from(destination),
        );
    }
    Ok(())
}

pub(super) fn aarch64_stack_store(
    instructions: &mut Vec<u32>,
    source: u8,
    offset: u32,
    width: u16,
) -> Result<(), EmissionError> {
    if !packed(width) {
        instructions.push(aarch64_unit_stack_access(
            aarch64_store_base(width)?,
            source,
            offset,
            width,
        )?);
        return Ok(());
    }
    if source == AARCH64_SCRATCH {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    last_offset(offset, width)?;
    instructions.push(0xaa00_03e0 | (u32::from(source) << 16) | u32::from(AARCH64_SCRATCH));
    for index in 0..width {
        instructions.push(aarch64_unit_stack_access(
            aarch64_store_base(1)?,
            AARCH64_SCRATCH,
            offset + u32::from(index),
            1,
        )?);
        if index + 1 != width {
            instructions.push(
                0xd340_fc00
                    | (8 << 16)
                    | (u32::from(AARCH64_SCRATCH) << 5)
                    | u32::from(AARCH64_SCRATCH),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_width_encodings_are_unchanged() {
        for width in [1, 2, 4, 8] {
            let mut expected = Vec::new();
            let mut actual = Vec::new();
            emit_x86_64_stack_load_width(&mut expected, 1, 16, width).unwrap();
            x86_stack_load(&mut actual, 1, 16, width).unwrap();
            emit_x86_64_memory_load_width(&mut expected, 1, 11, 16, width).unwrap();
            x86_memory_load(&mut actual, 1, 11, 16, width).unwrap();
            emit_x86_64_stack_store_width(&mut expected, 1, 16, width).unwrap();
            x86_stack_store(&mut actual, 1, 16, width).unwrap();
            assert_eq!(actual, expected);
            let mut actual = Vec::new();
            aarch64_stack_load(&mut actual, 1, 16, width).unwrap();
            aarch64_memory_load(&mut actual, 1, 9, 16, width).unwrap();
            aarch64_stack_store(&mut actual, 1, 16, width).unwrap();
            assert_eq!(
                actual,
                [
                    aarch64_unit_stack_access(aarch64_load_base(width).unwrap(), 1, 16, width)
                        .unwrap(),
                    aarch64_unit_memory_access(aarch64_load_base(width).unwrap(), 1, 9, 16, width)
                        .unwrap(),
                    aarch64_unit_stack_access(aarch64_store_base(width).unwrap(), 1, 16, width)
                        .unwrap(),
                ]
            );
        }
    }

    #[test]
    fn packed_loads_visit_only_exact_bytes_even_at_unaligned_offsets() {
        for width in [3, 5, 6, 7] {
            let mut accesses = Vec::new();
            x86_load(
                &mut Vec::new(),
                1,
                17,
                width,
                |_, register, offset, width| {
                    accesses.push((register, offset, width));
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(
                accesses,
                (0..width)
                    .map(|index| {
                        (
                            if index == 0 { 1 } else { X86_SCRATCH },
                            17 + u32::from(index),
                            1,
                        )
                    })
                    .collect::<Vec<_>>()
            );
            accesses.clear();
            aarch64_load(&mut Vec::new(), 1, 17, width, |register, offset, width| {
                accesses.push((register, offset, width));
                Ok(0)
            })
            .unwrap();
            assert_eq!(
                accesses,
                (0..width)
                    .map(|index| {
                        (
                            if index == 0 { 1 } else { AARCH64_SCRATCH },
                            17 + u32::from(index),
                            1,
                        )
                    })
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn packed_stores_preserve_source_and_touch_only_logical_bytes() {
        for width in [3, 5, 6, 7] {
            let mut bytes = Vec::new();
            x86_stack_store(&mut bytes, 1, 17, width).unwrap();
            assert_eq!(&bytes[..3], &[0x49, 0x89, 0xca]); // mov r10, rcx
            let mut cursor = 3;
            for index in 0..width {
                assert_eq!(
                    &bytes[cursor..cursor + 5],
                    &[0x44, 0x88, 0x54, 0x24, 17 + index as u8]
                );
                cursor += 5;
                if index + 1 != width {
                    assert_eq!(&bytes[cursor..cursor + 4], &[0x49, 0xc1, 0xea, 8]);
                    cursor += 4;
                }
            }
            assert_eq!(bytes.len(), cursor);
            let mut instructions = Vec::new();
            aarch64_stack_store(&mut instructions, 1, 17, width).unwrap();
            assert_eq!(instructions[0], 0xaa01_03f0); // mov x16, x1
            let mut cursor = 1;
            for index in 0..width {
                assert_eq!(
                    instructions[cursor],
                    0x3900_03f0 | ((17 + u32::from(index)) << 10)
                );
                cursor += 1;
                if index + 1 != width {
                    assert_eq!(instructions[cursor], 0xd348_fe10); // lsr x16, x16, 8
                    cursor += 1;
                }
            }
            assert_eq!(instructions.len(), cursor);
        }
    }

    #[test]
    fn packed_fragments_reject_scratch_and_base_collisions_before_emission() {
        for width in [3, 5, 6, 7] {
            let mut bytes = Vec::new();
            assert!(x86_stack_load(&mut bytes, X86_SCRATCH, 0, width).is_err());
            assert!(x86_stack_store(&mut bytes, X86_SCRATCH, 0, width).is_err());
            assert!(x86_memory_load(&mut bytes, 11, 11, 0, width).is_err());
            assert!(x86_memory_load(&mut bytes, 0, X86_SCRATCH, 0, width).is_err());
            assert!(x86_stack_load(&mut bytes, 0, u32::MAX, width).is_err());
            assert!(bytes.is_empty());
            let mut instructions = Vec::new();
            assert!(aarch64_stack_load(&mut instructions, AARCH64_SCRATCH, 0, width).is_err());
            assert!(aarch64_stack_store(&mut instructions, AARCH64_SCRATCH, 0, width).is_err());
            assert!(aarch64_memory_load(&mut instructions, 9, 9, 0, width).is_err());
            assert!(aarch64_memory_load(&mut instructions, 0, AARCH64_SCRATCH, 0, width).is_err());
            assert!(aarch64_stack_load(&mut instructions, 0, u32::MAX, width).is_err());
            assert!(instructions.is_empty());
        }
    }
}
