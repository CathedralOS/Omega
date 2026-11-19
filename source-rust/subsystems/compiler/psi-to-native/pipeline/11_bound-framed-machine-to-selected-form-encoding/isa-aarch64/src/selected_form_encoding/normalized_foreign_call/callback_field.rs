//! Place a private callback address into one field of an outgoing foreign call.

use layout_plans::LayoutPlacementReport;

pub(super) fn append_fragment(
    bytes: &mut Vec<u8>,
    source: u8,
    root: u8,
    fragment: u8,
    container: u8,
    placement: LayoutPlacementReport,
) -> Option<()> {
    let registers = [source, root, fragment, container];
    if registers.iter().any(|register| *register > 30)
        || registers
            .iter()
            .enumerate()
            .any(|(index, register)| registers[..index].contains(register))
    {
        return None;
    }
    let layout_plans::ScalarFragment {
        container_byte_offset: offset,
        container_width_bits: container_width,
        destination_lsb,
        source_lsb,
        width,
    } = layout_plans::private_address_fragment(placement, usize::MAX)?;
    let offset = usize::try_from(offset).ok()?;
    let count = usize::from(container_width / 8);
    let whole = destination_lsb == 0 && width == container_width;
    if !whole {
        load(bytes, container, fragment, root, offset, count)?;
    }
    instruction(
        bytes,
        0xd340_0000
            | ((source_lsb as u32) << 16)
            | (((source_lsb + width - 1) as u32) << 10)
            | (u32::from(source) << 5)
            | u32::from(fragment),
    );
    let result = if whole {
        fragment
    } else {
        instruction(
            bytes,
            0xb340_0000
                | (((64 - destination_lsb) as u32 % 64) << 16)
                | (((width - 1) as u32) << 10)
                | (u32::from(fragment) << 5)
                | u32::from(container),
        );
        container
    };
    store(bytes, result, root, offset, count)?;
    Some(())
}

fn instruction(bytes: &mut Vec<u8>, word: u32) {
    bytes.extend_from_slice(&word.to_le_bytes());
}

fn load(
    bytes: &mut Vec<u8>,
    result: u8,
    temporary: u8,
    root: u8,
    offset: usize,
    count: usize,
) -> Option<()> {
    match count {
        1 | 2 | 4 | 8 => instruction(bytes, memory_word(result, root, offset, count, true)?),
        _ => {
            instruction(bytes, 0xd280_0000 | u32::from(result));
            for index in 0..count {
                instruction(
                    bytes,
                    memory_word(temporary, root, offset + index, 1, true)?,
                );
                instruction(
                    bytes,
                    0xaa00_0000
                        | (u32::from(temporary) << 16)
                        | ((index as u32 * 8) << 10)
                        | (u32::from(result) << 5)
                        | u32::from(result),
                );
            }
        }
    }
    Some(())
}

fn store(bytes: &mut Vec<u8>, source: u8, root: u8, offset: usize, count: usize) -> Option<()> {
    match count {
        1 | 2 | 4 | 8 => instruction(bytes, memory_word(source, root, offset, count, false)?),
        _ => {
            for index in 0..count {
                instruction(bytes, memory_word(source, root, offset + index, 1, false)?);
                if index + 1 < count {
                    instruction(
                        bytes,
                        0xd340_fc00 | (8 << 16) | (u32::from(source) << 5) | u32::from(source),
                    );
                }
            }
        }
    }
    Some(())
}

fn memory_word(register: u8, base: u8, offset: usize, size: usize, load: bool) -> Option<u32> {
    let registers = (u32::from(base) << 5) | u32::from(register);
    if offset.is_multiple_of(size) && offset / size <= 4095 {
        let store = match size {
            1 => 0x3900_0000,
            2 => 0x7900_0000,
            4 => 0xb900_0000,
            8 => 0xf900_0000,
            _ => return None,
        };
        return Some(
            store | (u32::from(load) << 22) | (((offset / size) as u32) << 10) | registers,
        );
    }
    // The 64-bit load also admits an unscaled positive displacement.
    (load && size == 8 && offset <= 255).then(|| 0xf840_0000 | ((offset as u32) << 12) | registers)
}
