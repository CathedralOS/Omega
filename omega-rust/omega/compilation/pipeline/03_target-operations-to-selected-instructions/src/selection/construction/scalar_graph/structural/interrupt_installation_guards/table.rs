//! Check every IDT gate and the selected code descriptor before publication.
use super::*;
pub(super) fn validate_table(
    builder: &mut Builder<'_>,
    checks: &Checks<'_>,
    table: VirtualRegisterId,
    prefix: u32,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    bytes: u32,
    entries: &[GuardEntry<'_>],
) -> Result<(), SelectedInstructionError> {
    let count = checks.constant(builder, u64::from(bytes / 16))?;
    checks.iterate(builder, GATE_INDEX, count, |builder, index| {
        let pointer = checks.address(builder, table, index, 16)?;
        // The complete table is a single admitted source backing. Dynamic gate
        // reads retain that origin; exact index bounds are enforced above.
        let attributes = checks.load(
            builder,
            checks.anchor,
            pointer,
            5,
            1,
            StorageAccess::Backing,
        )?;
        let mask = checks.constant(builder, 0x80)?;
        let present = checks.binary(
            builder,
            SelectedInstructionKind::BitwiseAndI64,
            attributes,
            mask,
        )?;
        let mut selected = checks.constant(builder, 0)?;
        for entry in entries {
            let expected = checks.constant(builder, u64::from(entry.gate_byte_offset / 16))?;
            let matches = checks.equal(builder, index, expected)?;
            selected = checks.binary(
                builder,
                SelectedInstructionKind::BitwiseOrI64,
                selected,
                matches,
            )?;
        }
        let zero = checks.constant(builder, 0)?;
        let absent = checks.equal(builder, present, zero)?;
        let permitted = checks.binary(
            builder,
            SelectedInstructionKind::BitwiseOrI64,
            selected,
            absent,
        )?;
        checks.require(builder, permitted)
    })?;
    for entry in entries {
        let access = StorageAccess::Place {
            indices,
            byte_offset: prefix,
        };
        let offset = entry.gate_byte_offset;
        let attributes = checks.load(builder, checks.anchor, table, offset + 5, 1, access)?;
        // Present, DPL zero, 64-bit interrupt gate. Masked entry state would not be
        // true for a trap gate, even if its destination address were identical.
        checks.equal_constant(builder, attributes, 0x8e)?;
        let reserved = checks.load(builder, checks.anchor, table, offset + 12, 4, access)?;
        checks.equal_constant(builder, reserved, 0)?;
        let ist = checks.load(builder, checks.anchor, table, offset + 4, 1, access)?;
        match entry.entry.entry().inbound().state.stack {
            calling_conventions::EntryStack::Dedicated { .. } => {
                checks.require(builder, ist)?;
                let seven = checks.constant(builder, 7)?;
                checks.at_most(builder, ist, seven)?;
            }
            calling_conventions::EntryStack::Interrupted => {
                checks.equal_constant(builder, ist, 0)?
            }
            calling_conventions::EntryStack::ProviderSelected => {
                return Err(failure("gate has no selected stack domain"));
            }
        }
        let low = checks.load(builder, checks.anchor, table, offset, 2, access)?;
        let middle = checks.load(builder, checks.anchor, table, offset + 6, 2, access)?;
        let high = checks.load(builder, checks.anchor, table, offset + 8, 4, access)?;
        let middle = scalar::shift(builder, checks.row, checks.anchor, middle, 16, true)?;
        let high = scalar::shift(builder, checks.row, checks.anchor, high, 32, true)?;
        let address = checks.binary(builder, SelectedInstructionKind::BitwiseOrI64, low, middle)?;
        let address = checks.binary(
            builder,
            SelectedInstructionKind::BitwiseOrI64,
            address,
            high,
        )?;
        let same = checks.equal(builder, address, entry.address)?;
        checks.require(builder, same)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_code_selector(
    builder: &mut Builder<'_>,
    checks: &Checks<'_>,
    table: VirtualRegisterId,
    table_prefix: u32,
    table_indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    gdt: VirtualRegisterId,
    gdt_place: PlaceId,
    gdt_bytes: u32,
    entry: &GuardEntry<'_>,
) -> Result<(), SelectedInstructionError> {
    let selector = checks.load(
        builder,
        checks.anchor,
        table,
        entry.gate_byte_offset + 2,
        2,
        StorageAccess::Place {
            indices: table_indices,
            byte_offset: table_prefix,
        },
    )?;
    checks.require(builder, selector)?;
    let low_mask = checks.constant(builder, 7)?;
    let low = checks.binary(
        builder,
        SelectedInstructionKind::BitwiseAndI64,
        selector,
        low_mask,
    )?;
    checks.equal_constant(builder, low, 0)?;
    let last = checks.constant(builder, u64::from(gdt_bytes - 8))?;
    checks.at_most(builder, selector, last)?;
    let descriptor = checks.add(builder, gdt, selector)?;
    let access = checks.load(builder, gdt_place, descriptor, 5, 1, StorageAccess::Backing)?;
    let access_mask = checks.constant(builder, 0xfc)?;
    let access = checks.binary(
        builder,
        SelectedInstructionKind::BitwiseAndI64,
        access,
        access_mask,
    )?;
    // Present, DPL0, ordinary nonconforming executable segment. Readability
    // and the CPU's accessed bit do not change this entry's privilege contract.
    checks.equal_constant(builder, access, 0x98)?;
    let flags = checks.load(builder, gdt_place, descriptor, 6, 1, StorageAccess::Backing)?;
    let mode_mask = checks.constant(builder, 0x60)?;
    let mode = checks.binary(
        builder,
        SelectedInstructionKind::BitwiseAndI64,
        flags,
        mode_mask,
    )?;
    // L=1 and D=0 are required for the emitted 64-bit entry instructions.
    checks.equal_constant(builder, mode, 0x20)
}
