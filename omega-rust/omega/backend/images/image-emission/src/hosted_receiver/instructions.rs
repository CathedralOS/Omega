//! Independent semantics of the fixed hosted entry bridge.
//!
//! This reader does not call the writer or relocation patcher. It reconstructs
//! effective addresses and the call target from the final instructions. The
//! fixed register sequence proves that incoming SP/LR are saved in their own
//! image partition, SP switches before the application can spill, and normal
//! return restores that exact continuation without touching its suspended stack.
//! The wrapper itself has zero private-stack occupancy; the callee's complete
//! checked demand belongs to the separately validated stack partition.

use diagnostics::Diagnostic;

use super::{HostedReceiverPartitions, invalid};

#[cfg(test)]
mod tests;

pub(super) fn validate(
    bytes: &[u8],
    address: u64,
    selected_entry: u64,
    bss_address: u64,
    partitions: HostedReceiverPartitions,
) -> Result<(), Diagnostic> {
    let (encoded, remainder) = bytes.as_chunks::<4>();
    if encoded.len() != 16 || !remainder.is_empty() || !address.is_multiple_of(4) {
        return Err(invalid());
    }
    let mut words = [0; 16];
    for (word, encoded) in words.iter_mut().zip(encoded) {
        *word = u32::from_le_bytes(*encoded);
    }
    let continuation = bss_address
        .checked_add(partitions.saved_continuation_offset)
        .ok_or_else(invalid)?;
    let stack_top = bss_address
        .checked_add(partitions.stack_offset)
        .and_then(|low| low.checked_add(partitions.stack_byte_count))
        .ok_or_else(invalid)?;
    let receiver = bss_address
        .checked_add(partitions.receiver_offset)
        .ok_or_else(invalid)?;
    for (instruction, register, expected) in [
        (0, 9, continuation),
        (4, 10, stack_top),
        (7, 0, receiver),
        (10, 9, continuation),
    ] {
        let pc = address
            .checked_add((instruction * 4) as u64)
            .ok_or_else(invalid)?;
        if address_pair(words[instruction], words[instruction + 1], pc, register)? != expected {
            return Err(invalid());
        }
    }
    for (instruction, expected) in [
        (2, 0x9100_03ea),  // Incoming SP -> x10; no incoming-stack access.
        (3, 0xa900_792a),  // Store x10/LR through x9, exactly sixteen bytes.
        (6, 0x9100_015f),  // Private stack top -> SP before the call.
        (12, 0xa940_792a), // Reload SP/LR through the reconstructed x9.
        (13, 0x9100_015f), // Restore incoming SP; no later stack access.
        (14, 0x5280_0000), // Semantic Unit normal return -> physical zero.
        (15, 0xd65f_03c0), // Return through the restored LR.
    ] {
        if words[instruction] != expected {
            return Err(invalid());
        }
    }
    if words[9] & 0xfc00_0000 != 0x9400_0000 {
        return Err(invalid());
    }
    let signed_words = (i64::from(words[9] & 0x03ff_ffff) << 38) >> 38;
    let call_address = address.checked_add(36).ok_or_else(invalid)?;
    if call_address.checked_add_signed(signed_words * 4) != Some(selected_entry) {
        return Err(invalid());
    }
    Ok(())
}

fn address_pair(page: u32, add: u32, pc: u64, register: u32) -> Result<u64, Diagnostic> {
    if page & 0x9f00_001f != 0x9000_0000 | register
        || add & 0xffc0_03ff != 0x9100_0000 | (register << 5) | register
    {
        return Err(invalid());
    }
    let immediate = ((page >> 5) & 0x7ffff) << 2 | ((page >> 29) & 3);
    let signed_pages = (i64::from(immediate) << 43) >> 43;
    (pc & !0xfff)
        .checked_add_signed(signed_pages * 4096)
        .and_then(|base| base.checked_add(u64::from((add >> 10) & 0xfff)))
        .ok_or_else(invalid)
}
