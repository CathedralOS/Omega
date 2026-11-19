//! Byte widths of private callback field materialization.
use calling_conventions::MachineRegister;
use layout_plans::{LayoutPlacementReport, private_address_fragment};

pub fn callback_field_bytes(
    root: MachineRegister,
    placements: &[LayoutPlacementReport],
) -> Option<u16> {
    if placements.is_empty() {
        return None;
    }
    let index = calling_conventions::gpr_code(root)?;
    if matches!(
        root,
        MachineRegister::X86R11 | MachineRegister::X86R10 | MachineRegister::X86Rax
    ) {
        return None;
    }
    // REX, opcode, ModR/M and disp32. RSP and R12 also require a SIB byte.
    let memory = 7 + u16::from(index & 7 == 4);
    // A RIP-relative LEA loads the callback address into r11.
    placements.iter().try_fold(7_u16, |total, &placement| {
        let fragment = private_address_fragment(placement, i32::MAX as usize)?;
        let count = fragment.container_width_bits / 8;
        let whole =
            fragment.destination_lsb == 0 && fragment.width == fragment.container_width_bits;
        let (load, store) = match count {
            1 => (memory + 1, memory),
            2 => (memory + 1, memory + 1),
            4 | 8 => (memory, memory),
            // XOR initializes the load. Each byte uses a shift, a load and an OR.
            // The final store omits the shift to the next byte.
            _ => (3 + count * (memory + 8), count * memory + (count - 1) * 4),
        };
        let destination_shift = 4 * u16::from(fragment.destination_lsb != 0);
        let source_shift = 4 * u16::from(fragment.source_lsb != 0);
        let truncate = 8 * u16::from(fragment.width < 64);
        let insert = if whole {
            0
        } else {
            load + 8 + 2 * destination_shift + 3
        };
        total.checked_add(3 + source_shift + truncate + destination_shift + insert + store)
    })
}
