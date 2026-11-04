//! Insert selected source bits while preserving the rest of a destination container.
//! Register roles are explicit because callers may have live ABI arguments.

use calling_conventions::{MachineRegister, gpr_code};
use layout_plans::LayoutPlacementReport;

pub(crate) fn append_fragment(
    bytes: &mut Vec<u8>,
    source: MachineRegister,
    root: MachineRegister,
    fragment: MachineRegister,
    container: MachineRegister,
    placement: LayoutPlacementReport,
) -> Option<()> {
    let source = gpr_code(source)?;
    let root = gpr_code(root)?;
    let fragment = gpr_code(fragment)?;
    let container = gpr_code(container)?;
    let registers = [source, root, fragment, container];
    if registers
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
        i32::MAX as usize,
        "private address",
        offset,
        u16::try_from(container_width).ok()?,
        u16::try_from(destination_lsb).ok()?,
        u16::try_from(source_lsb).ok()?,
        u16::try_from(width).ok()?,
    )
    .ok()?;
    let offset = u32::try_from(offset).ok()?;
    let container_bytes = (container_width / 8) as u8;
    // Whole containers need no read. Partial containers are read before the
    // source is copied, so the fragment temporary can assemble unusual widths.
    let whole = destination_lsb == 0 && width == container_width;
    if !whole {
        load(bytes, container, fragment, root, offset, container_bytes);
        shift(bytes, container, 1, destination_lsb as u8); // ror
        shift(bytes, container, 5, width as u8); // shr
        shift(bytes, container, 4, width as u8); // shl
        shift(bytes, container, 0, destination_lsb as u8); // rol
    }
    register_binary(bytes, 0x89, source, fragment); // mov fragment, source
    shift(bytes, fragment, 5, source_lsb as u8);
    if width < 64 {
        shift(bytes, fragment, 4, (64 - width) as u8);
        shift(bytes, fragment, 5, (64 - width) as u8);
    }
    shift(bytes, fragment, 4, destination_lsb as u8);
    let result = if whole {
        fragment
    } else {
        register_binary(bytes, 0x09, fragment, container); // or container, fragment
        container
    };
    store(bytes, result, root, offset, container_bytes);
    Some(())
}

fn shift(bytes: &mut Vec<u8>, register: u8, operation: u8, amount: u8) {
    if amount != 0 {
        bytes.extend([
            0x48 | u8::from(register >= 8),
            0xc1,
            0xc0 | (operation << 3) | (register & 7),
            amount,
        ]);
    }
}

fn register_binary(bytes: &mut Vec<u8>, opcode: u8, source: u8, destination: u8) {
    bytes.extend([
        0x48 | (u8::from(source >= 8) << 2) | u8::from(destination >= 8),
        opcode,
        0xc0 | ((source & 7) << 3) | (destination & 7),
    ]);
}

fn memory(bytes: &mut Vec<u8>, opcode: &[u8], wide: bool, register: u8, root: u8, offset: u32) {
    bytes.push(0x40 | (u8::from(wide) << 3) | (u8::from(register >= 8) << 2) | u8::from(root >= 8));
    bytes.extend_from_slice(opcode);
    bytes.push(0x80 | ((register & 7) << 3) | (root & 7));
    if root & 7 == 4 {
        bytes.push(0x24);
    }
    bytes.extend_from_slice(&offset.to_le_bytes());
}

fn load(bytes: &mut Vec<u8>, result: u8, temporary: u8, root: u8, offset: u32, count: u8) {
    match count {
        1 => memory(bytes, &[0x0f, 0xb6], true, result, root, offset),
        2 => memory(bytes, &[0x0f, 0xb7], true, result, root, offset),
        4 => memory(bytes, &[0x8b], false, result, root, offset),
        8 => memory(bytes, &[0x8b], true, result, root, offset),
        _ => {
            register_binary(bytes, 0x31, result, result);
            for index in (0..count).rev() {
                shift(bytes, result, 4, 8);
                memory(
                    bytes,
                    &[0x0f, 0xb6],
                    true,
                    temporary,
                    root,
                    offset + u32::from(index),
                );
                register_binary(bytes, 0x09, temporary, result);
            }
        }
    }
}

fn store(bytes: &mut Vec<u8>, source: u8, root: u8, offset: u32, count: u8) {
    match count {
        1 => memory(bytes, &[0x88], false, source, root, offset),
        2 => {
            bytes.push(0x66);
            memory(bytes, &[0x89], false, source, root, offset);
        }
        4 => memory(bytes, &[0x89], false, source, root, offset),
        8 => memory(bytes, &[0x89], true, source, root, offset),
        _ => {
            for index in 0..count {
                memory(
                    bytes,
                    &[0x88],
                    false,
                    source,
                    root,
                    offset + u32::from(index),
                );
                if index + 1 < count {
                    shift(bytes, source, 5, 8);
                }
            }
        }
    }
}
