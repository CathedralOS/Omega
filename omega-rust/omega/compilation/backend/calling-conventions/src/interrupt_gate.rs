//! Join the complete private address recipe to one x86-64 gate. Container
//! boundaries may vary; every semantic address bit must occupy its hardware bit.
use layout_plans::LayoutPlacementReport;
use semantic_vocabulary::{AuthoredBitFragment, AuthoredBitFragments};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptGateAddress {
    pub fragments: AuthoredBitFragments,
    pub envelope_offset: u32,
    pub gate_byte_offset: u32,
}
pub fn interrupt_gate_address(
    placements: &[LayoutPlacementReport],
    table_bytes: u32,
) -> Result<InterruptGateAddress, &'static str> {
    let mut fragments = Vec::with_capacity(placements.len());
    let mut covered = 0u64;
    let mut gate = None;
    for placement in placements {
        let LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } = *placement
        else {
            return Err("interrupt entry address must retain its scattered hardware fragments");
        };
        if width == 0
            || container_width == 0
            || container_width > 64
            || container_width % 8 != 0
            || destination_lsb
                .checked_add(width)
                .is_none_or(|end| end > container_width)
            || source_lsb.checked_add(width).is_none_or(|end| end > 64)
            || container
                .checked_add(container_width / 8)
                .is_none_or(|end| end > u64::from(table_bytes))
        {
            return Err("interrupt address fragment exceeds its source or table container");
        }
        for bit in 0..width {
            let source = source_lsb + bit;
            let flag = 1u64 << source;
            if covered & flag != 0 {
                return Err("interrupt address source bit is duplicated");
            }
            covered |= flag;
            let hardware_bit = match source {
                0..=15 => source,
                _ => source + 32,
            };
            let physical = container
                .checked_mul(8)
                .and_then(|base| base.checked_add(destination_lsb + bit))
                .ok_or("interrupt address bit coordinate overflowed")?;
            let base = physical
                .checked_sub(hardware_bit)
                .ok_or("interrupt address fragment precedes its gate")?;
            if base % 128 != 0 || gate.is_some_and(|old| old != base) {
                return Err("private address fragments do not describe one aligned interrupt gate");
            }
            gate = Some(base);
        }
        fragments.push(AuthoredBitFragment {
            container_byte_offset: container,
            container_width_bits: container_width,
            destination_lsb,
            source_lsb,
            width,
        });
    }
    if covered != u64::MAX {
        return Err("interrupt address does not supply all 64 bits");
    }
    let gate_byte_offset = u32::try_from(gate.ok_or("interrupt address has no gate")? / 8)
        .map_err(|_| "interrupt gate offset exceeds native storage")?;
    if gate_byte_offset
        .checked_add(16)
        .is_none_or(|end| end > table_bytes)
    {
        return Err("interrupt gate exceeds the supplied table");
    }
    let fragments = AuthoredBitFragments::new(fragments);
    let envelope_offset = u32::try_from(
        fragments
            .byte_envelope()
            .ok_or("interrupt address has no container envelope")?
            .0,
    )
    .map_err(|_| "interrupt address envelope exceeds native storage")?;
    Ok(InterruptGateAddress {
        fragments,
        envelope_offset,
        gate_byte_offset,
    })
}
