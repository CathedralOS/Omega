//! Insert source bits with explicit registers, leaving the source and base intact.

use super::writer_encoding::{
    encode_load_w_from_x, encode_load_x_from_x, encode_store_w_to_x, encode_store_x_to_x,
};
use layout_plans::LayoutPlacementReport;

pub(crate) fn append_fragment(
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
    let (offset, container_width, destination_lsb, source_lsb, width) = match placement {
        LayoutPlacementReport::At { offset } => (offset, 64, 0, 0, 64),
        LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } => (
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        ),
        LayoutPlacementReport::IntegerAt { .. } => return None,
    };
    layout_plans::validate_fragment(
        usize::MAX,
        "private address",
        offset,
        u16::try_from(container_width).ok()?,
        u16::try_from(destination_lsb).ok()?,
        u16::try_from(source_lsb).ok()?,
        u16::try_from(width).ok()?,
    )
    .ok()?;
    let offset = usize::try_from(offset).ok()?;
    let count = usize::try_from(container_width / 8).ok()?;
    let whole = destination_lsb == 0 && width == container_width;
    if !whole {
        load(bytes, container, fragment, root, offset, count)?;
    }
    // UBFX keeps the sealed source unchanged for subsequent fragments.
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
        // BFI replaces precisely width bits in the loaded container.
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
        1 | 2 | 4 => bytes.extend(encode_load_w_from_x(result, root, offset, count).ok()?),
        8 => bytes.extend(encode_load_x_from_x(result, root, offset).ok()?),
        _ => {
            instruction(bytes, 0xd280_0000 | u32::from(result));
            for index in 0..count {
                bytes.extend(encode_load_w_from_x(temporary, root, offset + index, 1).ok()?);
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
        1 | 2 | 4 => bytes.extend(encode_store_w_to_x(source, root, offset, count).ok()?),
        8 => bytes.extend(encode_store_x_to_x(source, root, offset).ok()?),
        _ => {
            for index in 0..count {
                bytes.extend(encode_store_w_to_x(source, root, offset + index, 1).ok()?);
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
