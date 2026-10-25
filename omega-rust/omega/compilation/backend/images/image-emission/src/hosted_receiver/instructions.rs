//! Independent semantics of the fixed hosted entry bridge.
//!
//! This reader does not call the writer or relocation patcher. It reconstructs
//! effective addresses and the call target from the final instructions. The
//! fixed register sequence proves that incoming SP/LR are saved in their own
//! image partition, SP switches before the application can spill, and normal
//! return restores that exact continuation without touching its suspended stack.
//! The wrapper itself has zero private-stack occupancy; the callee's complete
//! checked demand belongs to the separately validated stack partition.

mod macos_arm64;
pub(super) use macos_arm64::validate as validate_macos_arm64;

use diagnostics::Diagnostic;

use super::{HostedReceiverPartitions, invalid};

fn expected_addresses(
    bss_address: u64,
    partitions: HostedReceiverPartitions,
    invalid: &dyn Fn() -> Diagnostic,
) -> Result<(u64, u64, u64), Diagnostic> {
    Ok((
        bss_address
            .checked_add(partitions.saved_continuation_offset)
            .ok_or_else(invalid)?,
        bss_address
            .checked_add(partitions.stack_offset)
            .and_then(|v| v.checked_add(partitions.stack_byte_count))
            .ok_or_else(invalid)?,
        bss_address
            .checked_add(partitions.allocation_offset)
            .ok_or_else(invalid)?,
    ))
}

fn rip_relative(bytes: &[u8], address: u64, field: usize) -> Option<u64> {
    let field_address = address.checked_add(field as u64)?;
    let displacement = bytes
        .get(field..field.checked_add(4)?)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(i32::from_le_bytes)?;
    field_address
        .checked_add(4)?
        .checked_add_signed(i64::from(displacement))
}

/// Replay the Linux System V installed argument bridge, including every exact
/// original byte, declared argument pointer and normal-return observation.
pub(super) fn validate_x86_64(
    bytes: &[u8],
    address: u64,
    selected_entry: u64,
    bss_address: u64,
    partitions: HostedReceiverPartitions,
    private_entry_ram: &[terminal_installation::CheckedPrivateEntryRamRecipe],
) -> Result<(), Diagnostic> {
    let invalid = || invalid(target::NativeTarget::linux_x64());
    let (continuation, stack_top, receiver) =
        expected_addresses(bss_address, partitions, &invalid)?;
    if bytes.get(0..3) != Some(&[0x48, 0x89, 0x25])
        || bytes.get(7..10) != Some(&[0x48, 0x8d, 0x25])
        || rip_relative(bytes, address, 3) != Some(continuation)
        || rip_relative(bytes, address, 10) != Some(stack_top)
    {
        return Err(invalid());
    }
    let mut offset = 14;
    let mut end = receiver;
    let mut originals = Vec::new();
    let mut headers = Vec::new();
    for recipe in private_entry_ram {
        let alignment = recipe.alignment();
        if !alignment.is_power_of_two() {
            return Err(invalid());
        }
        let start = end
            .checked_add(alignment - 1)
            .map(|value| value & !(alignment - 1))
            .ok_or_else(invalid)?;
        let header = start
            .checked_add(recipe.contents().len() as u64)
            .ok_or_else(invalid)?;
        headers.push(header);
        end = header
            .checked_add(recipe.observation_header().len() as u64)
            .ok_or_else(invalid)?;
        originals.push(start);
        for (index, byte) in recipe.contents().iter().enumerate() {
            // C6 /0 stores one supplied byte to its exact original residence.
            // Its next IP includes the immediate byte after the disp32 field.
            if bytes.get(offset..offset + 2) != Some(&[0xc6, 0x05])
                || bytes.get(offset + 6) != Some(byte)
                || rip_relative(bytes, address, offset + 2).and_then(|target| target.checked_add(1))
                    != start.checked_add(index as u64)
            {
                return Err(invalid());
            }
            offset += 7;
        }
    }
    if end
        != bss_address
            .checked_add(partitions.end_offset)
            .ok_or_else(invalid)?
    {
        return Err(invalid());
    }
    let argument_encodings = [
        (0x48, 0x3d),
        (0x48, 0x35),
        (0x48, 0x15),
        (0x48, 0x0d),
        (0x4c, 0x05),
        (0x4c, 0x0d),
    ];
    for ordinal in 0..private_entry_ram.len() {
        let index = private_entry_ram
            .iter()
            .position(|recipe| recipe.input().runtime_position as usize == ordinal)
            .ok_or_else(invalid)?;
        let expected = originals[index];
        let (rex, modrm) = argument_encodings
            .get(ordinal)
            .copied()
            .ok_or_else(invalid)?;
        if bytes.get(offset..offset + 3) != Some(&[rex, 0x8d, modrm])
            || rip_relative(bytes, address, offset + 3) != Some(expected)
        {
            return Err(invalid());
        }
        offset += 7;
    }
    if bytes.get(offset) != Some(&0xe8)
        || rip_relative(bytes, address, offset + 1) != Some(selected_entry)
    {
        return Err(invalid());
    }
    offset += 5;
    // Metadata is constructed by the runtime after normal return; guest
    // stderr contributes no bytes to this dedicated, exact sealed-row lane.
    for ((recipe, original), header_address) in
        private_entry_ram.iter().zip(&originals).zip(&headers)
    {
        let header = recipe.observation_header();
        for (index, byte) in header.iter().enumerate() {
            if bytes.get(offset..offset + 2) != Some(&[0xc6, 0x05])
                || bytes.get(offset + 6) != Some(byte)
                || rip_relative(bytes, address, offset + 2).and_then(|target| target.checked_add(1))
                    != header_address.checked_add(index as u64)
            {
                return Err(invalid());
            }
            offset += 7;
        }
        for (source, length) in [
            (*header_address, header.len()),
            (*original, recipe.contents().len()),
        ] {
            if bytes.get(offset..offset + 10) != Some(&[0xb8, 1, 0, 0, 0, 0xbf, 3, 0, 0, 0])
                || bytes.get(offset + 10..offset + 13) != Some(&[0x48, 0x8d, 0x35])
                || rip_relative(bytes, address, offset + 13) != Some(source)
                || bytes.get(offset + 17) != Some(&0xba)
                || bytes.get(offset + 18..offset + 22) != Some(&(length as u32).to_le_bytes())
                || bytes.get(offset + 22..offset + 26) != Some(&[0x0f, 0x05, 0x48, 0x3d])
                || bytes.get(offset + 26..offset + 30) != Some(&(length as u32).to_le_bytes())
                || bytes.get(offset + 30..offset + 46)
                    != Some(&[
                        0x74, 14, 0xbf, 200, 0, 0, 0, 0xb8, 0xe7, 0, 0, 0, 0x0f, 0x05, 0x0f, 0x0b,
                    ])
            {
                return Err(invalid());
            }
            offset += 46;
        }
    }
    if bytes.get(offset..) != Some(&[0x31, 0xff, 0xb8, 0xe7, 0, 0, 0, 0x0f, 0x05, 0x0f, 0x0b]) {
        return Err(invalid());
    }
    Ok(())
}
