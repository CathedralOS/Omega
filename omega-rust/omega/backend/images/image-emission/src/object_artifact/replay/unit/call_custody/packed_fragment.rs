//! Exact byte-wise reconstruction for ABI fragments without a native access width.
//!
//! Scratch registers are disjoint from the indirect source bases and ordinary
//! ABI registers. Every memory access is one byte; alignment padding is never
//! loaded or stored, and existing single-instruction widths stay unchanged.

pub(super) fn is_packed(width: u16) -> bool {
    matches!(width, 3 | 5 | 6 | 7)
}

pub(super) fn x86_load(
    bytes: &mut Vec<u8>,
    destination: u8,
    offset: u32,
    width: u16,
    indirect: bool,
) -> Option<()> {
    if !is_packed(width) || destination == 10 || (indirect && destination == 11) {
        return None;
    }
    for index in 0..width {
        let register = if index == 0 { destination } else { 10 };
        let byte_offset = offset.checked_add(u32::from(index))?;
        if indirect {
            super::projected_copy::x86_pointer_load(bytes, register, byte_offset, 1)?;
        } else {
            super::expected_x86_stack_load(bytes, register, byte_offset, 1)?;
        }
        if index != 0 {
            bytes.extend_from_slice(&[0x49, 0xc1, 0xe2, u8::try_from(index * 8).ok()?]);
            bytes.extend_from_slice(&[
                0x4c | ((destination >> 3) & 1),
                0x09,
                0xd0 | (destination & 7),
            ]);
        }
    }
    Some(())
}

pub(super) fn x86_store(bytes: &mut Vec<u8>, source: u8, offset: u32, width: u16) -> Option<()> {
    if !is_packed(width) || source == 10 {
        return None;
    }
    bytes.extend_from_slice(&[
        0x49 | (((source >> 3) & 1) << 2),
        0x89,
        0xc2 | ((source & 7) << 3),
    ]);
    for index in 0..width {
        super::projected_copy::x86_stack_store(
            bytes,
            10,
            offset.checked_add(u32::from(index))?,
            1,
        )?;
        if index + 1 != width {
            bytes.extend_from_slice(&[0x49, 0xc1, 0xea, 8]);
        }
    }
    Some(())
}

pub(super) fn aarch64_load(
    bytes: &mut Vec<u8>,
    destination: u8,
    offset: u32,
    width: u16,
    indirect: bool,
) -> Option<()> {
    if !is_packed(width) || destination == 16 || (indirect && destination == 9) {
        return None;
    }
    for index in 0..width {
        let register = if index == 0 { destination } else { 16 };
        let mut load =
            super::expected_aarch64_stack_load(register, offset.checked_add(u32::from(index))?, 1)?;
        if indirect {
            load = (load & !(31 << 5)) | (9 << 5);
        }
        bytes.extend_from_slice(&load.to_le_bytes());
        if index != 0 {
            let combine = 0xaa00_0000
                | (16 << 16)
                | (u32::from(index * 8) << 10)
                | (u32::from(destination) << 5)
                | u32::from(destination);
            bytes.extend_from_slice(&combine.to_le_bytes());
        }
    }
    Some(())
}

pub(super) fn aarch64_store(
    bytes: &mut Vec<u8>,
    source: u8,
    offset: u32,
    width: u16,
) -> Option<()> {
    if !is_packed(width) || source == 16 {
        return None;
    }
    let copy = 0xaa00_03e0 | (u32::from(source) << 16) | 16;
    bytes.extend_from_slice(&copy.to_le_bytes());
    for index in 0..width {
        bytes.extend_from_slice(
            &super::projected_copy::aarch64_stack_store(
                16,
                offset.checked_add(u32::from(index))?,
                1,
            )?
            .to_le_bytes(),
        );
        if index + 1 != width {
            bytes.extend_from_slice(&(0xd340_fc00_u32 | (8 << 16) | (16 << 5) | 16).to_le_bytes());
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_three_byte_fragments_replay_exact_accesses_and_shifts() {
        let mut bytes = Vec::new();
        x86_store(&mut bytes, 0, 16, 3).unwrap();
        assert_eq!(
            bytes,
            [
                0x49, 0x89, 0xc2, 0x44, 0x88, 0x54, 0x24, 16, 0x49, 0xc1, 0xea, 8, 0x44, 0x88,
                0x54, 0x24, 17, 0x49, 0xc1, 0xea, 8, 0x44, 0x88, 0x54, 0x24, 18,
            ]
        );
        bytes.clear();
        x86_load(&mut bytes, 0, 16, 3, false).unwrap();
        assert_eq!(
            bytes,
            [
                0x40, 0x0f, 0xb6, 0x44, 0x24, 16, 0x44, 0x0f, 0xb6, 0x54, 0x24, 17, 0x49, 0xc1,
                0xe2, 8, 0x4c, 0x09, 0xd0, 0x44, 0x0f, 0xb6, 0x54, 0x24, 18, 0x49, 0xc1, 0xe2, 16,
                0x4c, 0x09, 0xd0,
            ]
        );
        bytes.clear();
        aarch64_store(&mut bytes, 0, 16, 3).unwrap();
        assert_eq!(
            bytes,
            [
                0xaa00_03f0_u32,
                0x3900_43f0,
                0xd348_fe10,
                0x3900_47f0,
                0xd348_fe10,
                0x3900_4bf0
            ]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
        );
        bytes.clear();
        aarch64_load(&mut bytes, 0, 16, 3, false).unwrap();
        assert_eq!(
            bytes,
            [
                0x3940_43e0_u32,
                0x3940_47f0,
                0xaa10_2000,
                0x3940_4bf0,
                0xaa10_4000
            ]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn packed_fragments_reject_scratch_and_source_base_aliases() {
        for width in [3, 5, 6, 7] {
            assert!(x86_load(&mut Vec::new(), 10, 0, width, false).is_none());
            assert!(x86_load(&mut Vec::new(), 11, 0, width, true).is_none());
            assert!(x86_store(&mut Vec::new(), 10, 0, width).is_none());
            assert!(aarch64_load(&mut Vec::new(), 16, 0, width, false).is_none());
            assert!(aarch64_load(&mut Vec::new(), 9, 0, width, true).is_none());
            assert!(aarch64_store(&mut Vec::new(), 16, 0, width).is_none());
        }
        for width in [0, 1, 2, 4, 8, 9] {
            assert!(!is_packed(width));
            assert!(x86_store(&mut Vec::new(), 0, 0, width).is_none());
            assert!(aarch64_store(&mut Vec::new(), 0, 0, width).is_none());
        }
    }
}
