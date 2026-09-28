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
    let invalid = || invalid(target::NativeTarget::macos_arm64());
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
        if address_pair(
            words[instruction],
            words[instruction + 1],
            pc,
            register,
            &invalid,
        )? != expected
        {
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

fn address_pair(
    page: u32,
    add: u32,
    pc: u64,
    register: u32,
    invalid: &dyn Fn() -> Diagnostic,
) -> Result<u64, Diagnostic> {
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

/// Independent reader for the Linux x86-64 hosted receiver bridge.
///
/// The emitted text is exactly 37 bytes:
///
/// ```text
/// 0:  48 89 25 <rel32>   mov [rip+disp], rsp     -> saved continuation slot
/// 7:  48 8d 25 <rel32>   lea rsp, [rip+disp]     -> private stack top
/// 14: 48 8d 3d <rel32>   lea rdi, [rip+disp]     -> receiver storage
/// 21: e8 <rel32>         call semantic entry
/// 26: 31 ff              xor edi, edi            -> Unit result -> status zero
/// 28: b8 e7 00 00 00     mov eax, 231            -> exit_group
/// 33: 0f 05              syscall
/// 35: 0f 0b              ud2 if the nonreturning call ever returns
/// ```
///
/// The reader resolves each RIP-relative displacement against the final image
/// addresses of the exact bridge partitions and the semantic entry. The kernel
/// arrival stack image in rsp is preserved in the saved-continuation residence
/// before rsp switches, so no incoming-stack access is ever assumed.
pub(super) fn validate_x86_64(
    bytes: &[u8],
    address: u64,
    selected_entry: u64,
    bss_address: u64,
    partitions: HostedReceiverPartitions,
) -> Result<(), Diagnostic> {
    let invalid = || invalid(target::NativeTarget::linux_x64());
    if bytes.len() != 37 {
        return Err(invalid());
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
    // Every rel32 displacement is resolved from the address immediately after
    // the four-byte relocation field, matching the owned x86-64 relocator.
    let rip_relative = |field: usize| -> Option<u64> {
        let field_address = address.checked_add(field as u64)?;
        let displacement = bytes
            .get(field..field.checked_add(4)?)
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
            .map(i32::from_le_bytes)?;
        field_address
            .checked_add(4)?
            .checked_add_signed(i64::from(displacement))
    };
    for (field, expected) in [(3, continuation), (10, stack_top), (17, receiver)] {
        if rip_relative(field) != Some(expected) {
            return Err(invalid());
        }
    }
    if bytes[0..3] != [0x48, 0x89, 0x25]
        || bytes[7..10] != [0x48, 0x8d, 0x25]
        || bytes[14..17] != [0x48, 0x8d, 0x3d]
        || bytes[21] != 0xe8
        || bytes[26..37]
            != [
                0x31, 0xff, 0xb8, 0xe7, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x0f, 0x0b,
            ]
    {
        return Err(invalid());
    }
    if rip_relative(22) != Some(selected_entry) {
        return Err(invalid());
    }
    Ok(())
}

/// Independent reader for the Windows x86-64 hosted receiver bridge.
///
/// The emitted text is exactly 40 bytes:
///
/// ```text
/// 0:  48 89 25 <rel32>   mov [rip+disp], rsp     -> saved continuation slot
/// 7:  48 8d 25 <rel32>   lea rsp, [rip+disp]     -> private stack top
/// 14: 48 83 ec 20        sub rsp, 32             -> callee shadow space
/// 18: 48 8d 0d <rel32>   lea rcx, [rip+disp]     -> receiver storage
/// 25: e8 <rel32>         call semantic entry
/// 30: 31 c0              xor eax, eax            -> Unit result -> status zero
/// 32: 48 8b 25 <rel32>   mov rsp, [rip+disp]     -> saved continuation slot
/// 39: c3                 ret                     -> loader continuation
/// ```
///
/// The loader arrival is an ordinary Microsoft x64 call: the incoming stack
/// pointer is the continuation and its return address stays on that suspended
/// stack, never on the private one. The reader proves the continuation slot is
/// written before the switch, the callee sees the receiver in rcx over its
/// reserved shadow space, and normal return restores the exact incoming stack
/// pointer before the loader-facing `ret`.
pub(super) fn validate_windows_x86_64(
    bytes: &[u8],
    address: u64,
    selected_entry: u64,
    bss_address: u64,
    partitions: HostedReceiverPartitions,
) -> Result<(), Diagnostic> {
    let invalid = || invalid(target::NativeTarget::windows_x64());
    if bytes.len() != 40 {
        return Err(invalid());
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
    // Every rel32 displacement is resolved from the address immediately after
    // the four-byte relocation field, matching the owned x86-64 relocator.
    let rip_relative = |field: usize| -> Option<u64> {
        let field_address = address.checked_add(field as u64)?;
        let displacement = bytes
            .get(field..field.checked_add(4)?)
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
            .map(i32::from_le_bytes)?;
        field_address
            .checked_add(4)?
            .checked_add_signed(i64::from(displacement))
    };
    for (field, expected) in [
        (3, continuation),
        (10, stack_top),
        (21, receiver),
        (35, continuation),
    ] {
        if rip_relative(field) != Some(expected) {
            return Err(invalid());
        }
    }
    if bytes[0..3] != [0x48, 0x89, 0x25]
        || bytes[7..10] != [0x48, 0x8d, 0x25]
        || bytes[14..18] != [0x48, 0x83, 0xec, 0x20]
        || bytes[18..21] != [0x48, 0x8d, 0x0d]
        || bytes[25] != 0xe8
        || bytes[30..32] != [0x31, 0xc0]
        || bytes[32..35] != [0x48, 0x8b, 0x25]
        || bytes[39] != 0xc3
    {
        return Err(invalid());
    }
    if rip_relative(26) != Some(selected_entry) {
        return Err(invalid());
    }
    Ok(())
}

/// Independent reader for the Linux ARM64 hosted receiver bridge.
///
/// The emitted text is exactly 60 bytes (fifteen instructions):
///
/// ```text
/// 0:  adrp x9, scratch ; add x9, x9, lo12   -> saved continuation residence
/// 2:  mov x10, sp                          -> incoming stack image base
/// 3:  ldr x11, [x10]                       -> authored head-word input (argc)
/// 4:  stp x10, x11, [x9]                   -> preserve continuation + input
/// 5:  adrp x10, stack_top ; add x10, lo12  -> private stack top
/// 7:  mov sp, x10                          -> switch before any spill
/// 8:  adrp x0, receiver ; add x0, lo12     -> provisioned receiver
/// 10: bl <rel26>                           -> exact semantic continuation
/// 11: mov w0, #0                           -> Unit result -> status zero
/// 12: movz x8, #94                         -> exit_group
/// 13: svc #0
/// 14: brk #0                               -> fence if the call returned
/// ```
///
/// Kernel arrival on AArch64 supplies the stack image through sp and no
/// return continuation. The reader proves the continuation pointer and the
/// authored contract input are stored in their own image partition before sp
/// switches, that the callee sees the provisioned receiver in x0, and that
/// normal completion is the nonreturning exit_group supervisor call.
pub(super) fn validate_linux_arm64(
    bytes: &[u8],
    address: u64,
    selected_entry: u64,
    bss_address: u64,
    partitions: HostedReceiverPartitions,
) -> Result<(), Diagnostic> {
    let invalid = || invalid(target::NativeTarget::linux_arm64());
    let (encoded, remainder) = bytes.as_chunks::<4>();
    if encoded.len() != 15 || !remainder.is_empty() || !address.is_multiple_of(4) {
        return Err(invalid());
    }
    let mut words = [0; 15];
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
    for (instruction, register, expected) in
        [(0, 9, continuation), (5, 10, stack_top), (8, 0, receiver)]
    {
        let pc = address
            .checked_add((instruction * 4) as u64)
            .ok_or_else(invalid)?;
        if address_pair(
            words[instruction],
            words[instruction + 1],
            pc,
            register,
            &invalid,
        )? != expected
        {
            return Err(invalid());
        }
    }
    for (instruction, expected) in [
        (2, 0x9100_03ea),  // Incoming SP -> x10; the arrival image stays observable.
        (3, 0xf940_014b),  // The contract's head-word input loads through x10.
        (4, 0xa900_2d2a),  // Store x10/x11 through x9, exactly sixteen bytes.
        (7, 0x9100_015f),  // Private stack top -> SP before the call.
        (11, 0x5280_0000), // Semantic Unit normal return -> physical zero.
        (12, 0xd280_0bc8), // exit_group -> x8.
        (13, 0xd400_0001), // svc #0.
        (14, 0xd420_0000), // brk #0 if the nonreturning call ever returned.
    ] {
        if words[instruction] != expected {
            return Err(invalid());
        }
    }
    if words[10] & 0xfc00_0000 != 0x9400_0000 {
        return Err(invalid());
    }
    let signed_words = (i64::from(words[10] & 0x03ff_ffff) << 38) >> 38;
    let call_address = address.checked_add(40).ok_or_else(invalid)?;
    if call_address.checked_add_signed(signed_words * 4) != Some(selected_entry) {
        return Err(invalid());
    }
    Ok(())
}
