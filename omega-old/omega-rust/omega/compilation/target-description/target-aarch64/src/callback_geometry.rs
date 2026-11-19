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
    let MachineRegister::Aarch64X(root) = root else {
        return None;
    };
    if root > 30 || matches!(root, 15..=17) {
        return None;
    }
    // ADRP and ADD form the callback address in x16.
    placements.iter().try_fold(8_u16, |total, &placement| {
        let fragment = private_address_fragment(placement, usize::MAX)?;
        let count = fragment.container_width_bits / 8;
        let offset = fragment.container_byte_offset;
        let whole =
            fragment.destination_lsb == 0 && fragment.width == fragment.container_width_bits;
        let (load, store) = match count {
            1 | 2 | 4 | 8 => {
                // Every fragment ends with a scaled, unsigned store.
                let stride = u64::from(count);
                if !offset.is_multiple_of(stride) || offset / stride > 4095 {
                    return None;
                }
                (1, 1)
            }
            _ => {
                if offset.checked_add(u64::from(count) - 1)? > 4095 {
                    return None;
                }
                // Partial storage assembles the old container and writes it one byte at a time.
                (1 + 2 * count, 2 * count - 1)
            }
        };
        let words = if whole { 1 + store } else { load + 2 + store };
        total.checked_add(words * 4)
    })
}
