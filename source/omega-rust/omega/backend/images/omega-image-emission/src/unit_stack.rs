//! Exact Unit-stack evidence validation and instruction replay.
//!
//! This module owns function/call frame validation, complete stack-mutation
//! accounting, and the canonical x86-64/AArch64 adjustment and return-link
//! encodings shared by neighboring scalar replay. It does not construct stack
//! demand or emit executable images.

use omega_machine_code::{
    ForeignCallRelocation, StackAdjustmentPair, UnitCallStackEvidence, UnitStackEvidence,
};
use omega_target::Architecture;
use omega_target_operations::CallSiteOwner;
use psi_core::MachineId;

use super::{
    ObjectError, ObjectUnitCallStack, ObjectUnitStack, validate_foreign_call_site,
    validate_internal_call_site,
};

pub(super) fn validate_unit_function_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    evidence: UnitStackEvidence,
    frame_start: usize,
) -> Result<ObjectUnitStack, ObjectError> {
    if evidence.stack_alignment != 16 {
        return Err(ObjectError::InvalidUnitStackAlignment {
            machine,
            alignment: evidence.stack_alignment,
        });
    }
    let frame_bytes = match evidence.frame {
        Some(frame) => {
            validate_stack_adjustment_pair(architecture, machine, None, bytes, frame)?;
            if frame.allocation_offset != frame_start {
                return Err(ObjectError::InvalidUnitStackEncoding {
                    machine,
                    owner: None,
                    offset: frame.allocation_offset,
                });
            }
            frame.byte_size
        }
        None => 0,
    };
    match architecture {
        Architecture::X86_64 => {
            if evidence.aarch64_return_link.is_some()
                || evidence.frame.is_some_and(|frame| {
                    frame
                        .release_offset
                        .checked_add(frame.release_byte_count)
                        .and_then(|end| end.checked_add(1))
                        != Some(bytes.len())
                })
                || bytes.last() != Some(&0xc3)
            {
                return Err(ObjectError::InvalidUnitStackEncoding {
                    machine,
                    owner: None,
                    offset: bytes.len().saturating_sub(1),
                });
            }
        }
        Architecture::Aarch64 => {
            let frame = evidence
                .frame
                .ok_or(ObjectError::MissingAarch64UnitReturnLink {
                    caller: machine,
                    operation: None,
                })?;
            let link =
                evidence
                    .aarch64_return_link
                    .ok_or(ObjectError::MissingAarch64UnitReturnLink {
                        caller: machine,
                        operation: None,
                    })?;
            let expected_store = aarch64_unit_link_instruction(false, link.frame_byte_offset);
            let expected_load = aarch64_unit_link_instruction(true, link.frame_byte_offset);
            if frame_bytes < 16
                || link.store_offset != frame.allocation_offset + frame.allocation_byte_count
                || link.load_offset + 4 != frame.release_offset
                || frame.release_offset + frame.release_byte_count + 4 != bytes.len()
                || bytes.get(link.store_offset..link.store_offset + 4)
                    != Some(&expected_store.to_le_bytes())
                || bytes.get(link.load_offset..link.load_offset + 4)
                    != Some(&expected_load.to_le_bytes())
                || bytes.get(bytes.len().saturating_sub(4)..)
                    != Some(&0xd65f_03c0_u32.to_le_bytes())
            {
                return Err(ObjectError::MissingAarch64UnitReturnLink {
                    caller: machine,
                    operation: None,
                });
            }
        }
    }
    Ok(ObjectUnitStack {
        frame_bytes,
        local_peak_bytes: frame_bytes,
        stack_alignment: evidence.stack_alignment,
    })
}

pub(super) fn validate_unit_call_stack(
    architecture: Architecture,
    caller: MachineId,
    bytes: &[u8],
    relocation: omega_machine_code::InternalCallRelocation,
    function_evidence: UnitStackEvidence,
    function: ObjectUnitStack,
    call: UnitCallStackEvidence,
) -> Result<ObjectUnitCallStack, ObjectError> {
    validate_internal_call_site(architecture, caller, bytes, relocation)?;
    let owner = relocation.owner;
    let outbound_bytes = match call.outbound {
        Some(outbound) => {
            validate_stack_adjustment_pair(architecture, caller, Some(owner), bytes, outbound)?;
            outbound.byte_size
        }
        None => 0,
    };
    let (call_start, call_end, linkage_bytes) = match architecture {
        Architecture::X86_64 => (
            relocation.offset.saturating_sub(1),
            relocation.offset.saturating_add(4),
            8,
        ),
        Architecture::Aarch64 => (relocation.offset, relocation.offset.saturating_add(4), 0),
    };
    if architecture == Architecture::X86_64 && call.outbound.is_none() {
        return Err(ObjectError::MissingX86UnitCallStackAdjustment { caller, owner });
    }
    if let Some(outbound) = call.outbound {
        let allocation_end = outbound
            .allocation_offset
            .checked_add(outbound.allocation_byte_count)
            .ok_or(ObjectError::UnitCallStackArithmeticOverflow { caller, owner })?;
        let frame_release = function_evidence.frame.map(|frame| frame.release_offset);
        if allocation_end > call_start
            || outbound.release_offset != call_end
            || frame_release.is_some_and(|release| {
                outbound
                    .release_offset
                    .checked_add(outbound.release_byte_count)
                    .is_none_or(|end| end > release)
            })
        {
            return Err(ObjectError::InvalidUnitStackEncoding {
                machine: caller,
                owner: Some(owner),
                offset: outbound.allocation_offset,
            });
        }
    }
    let transient_bytes = outbound_bytes
        .checked_add(linkage_bytes)
        .ok_or(ObjectError::UnitCallStackArithmeticOverflow { caller, owner })?;
    let caller_live_bytes = function
        .frame_bytes
        .checked_add(transient_bytes)
        .ok_or(ObjectError::UnitCallStackArithmeticOverflow { caller, owner })?;
    if !caller_live_bytes.is_multiple_of(function.stack_alignment) {
        return Err(ObjectError::MisalignedUnitCalleeEntry {
            caller,
            owner,
            caller_live_bytes,
        });
    }
    Ok(ObjectUnitCallStack {
        owner,
        target: relocation.target,
        text_offset: relocation.offset,
        active_frame_bytes: function.frame_bytes,
        transient_bytes,
        caller_live_bytes,
    })
}

pub(super) fn validate_foreign_unit_call_stack(
    architecture: Architecture,
    caller: MachineId,
    bytes: &[u8],
    relocation: &ForeignCallRelocation,
    function_evidence: UnitStackEvidence,
    function: ObjectUnitStack,
) -> Result<u32, ObjectError> {
    validate_foreign_call_site(architecture, caller, bytes, relocation)?;
    let owner = relocation.owner;
    let outbound_bytes = match relocation.unit_stack.outbound {
        Some(outbound) => {
            validate_stack_adjustment_pair(architecture, caller, Some(owner), bytes, outbound)?;
            outbound.byte_size
        }
        None => 0,
    };
    let (call_start, call_end, linkage_bytes) = match architecture {
        Architecture::X86_64 => (
            relocation.offset.saturating_sub(1),
            relocation.offset.saturating_add(4),
            8,
        ),
        Architecture::Aarch64 => (relocation.offset, relocation.offset.saturating_add(4), 0),
    };
    if architecture == Architecture::X86_64 && relocation.unit_stack.outbound.is_none() {
        return Err(ObjectError::MissingX86UnitCallStackAdjustment { caller, owner });
    }
    if let Some(outbound) = relocation.unit_stack.outbound {
        let allocation_end = outbound
            .allocation_offset
            .checked_add(outbound.allocation_byte_count)
            .ok_or(ObjectError::UnitCallStackArithmeticOverflow { caller, owner })?;
        let frame_release = function_evidence.frame.map(|frame| frame.release_offset);
        if allocation_end > call_start
            || outbound.release_offset != call_end
            || frame_release.is_some_and(|release| {
                outbound
                    .release_offset
                    .checked_add(outbound.release_byte_count)
                    .is_none_or(|end| end > release)
            })
        {
            return Err(ObjectError::InvalidUnitStackEncoding {
                machine: caller,
                owner: Some(owner),
                offset: outbound.allocation_offset,
            });
        }
    }
    let transient_bytes = outbound_bytes
        .checked_add(linkage_bytes)
        .ok_or(ObjectError::UnitCallStackArithmeticOverflow { caller, owner })?;
    let caller_live_bytes = function
        .frame_bytes
        .checked_add(transient_bytes)
        .ok_or(ObjectError::UnitCallStackArithmeticOverflow { caller, owner })?;
    if !caller_live_bytes.is_multiple_of(function.stack_alignment) {
        return Err(ObjectError::MisalignedUnitCalleeEntry {
            caller,
            owner,
            caller_live_bytes,
        });
    }
    Ok(caller_live_bytes)
}

pub(super) fn validate_stack_adjustment_pair(
    architecture: Architecture,
    machine: MachineId,
    owner: Option<CallSiteOwner>,
    bytes: &[u8],
    pair: StackAdjustmentPair,
) -> Result<(), ObjectError> {
    if pair.allocation_offset >= pair.release_offset {
        return Err(ObjectError::InvalidUnitStackEncoding {
            machine,
            owner,
            offset: pair.allocation_offset,
        });
    }
    let (allocation, release) = match architecture {
        Architecture::X86_64 => (
            x86_64_stack_adjustment(pair.byte_size, false),
            x86_64_stack_adjustment(pair.byte_size, true),
        ),
        Architecture::Aarch64 => {
            if pair.byte_size > 0xfff {
                return Err(ObjectError::InvalidUnitStackEncoding {
                    machine,
                    owner,
                    offset: pair.allocation_offset,
                });
            }
            (
                (0xd100_03ff_u32 | (pair.byte_size << 10))
                    .to_le_bytes()
                    .to_vec(),
                (0x9100_03ff_u32 | (pair.byte_size << 10))
                    .to_le_bytes()
                    .to_vec(),
            )
        }
    };
    if pair.byte_size == 0
        || pair.allocation_byte_count != allocation.len()
        || pair.release_byte_count != release.len()
        || bytes
            .get(pair.allocation_offset..pair.allocation_offset.saturating_add(allocation.len()))
            != Some(allocation.as_slice())
        || bytes.get(pair.release_offset..pair.release_offset.saturating_add(release.len()))
            != Some(release.as_slice())
    {
        return Err(ObjectError::InvalidUnitStackEncoding {
            machine,
            owner,
            offset: pair.allocation_offset,
        });
    }
    Ok(())
}

pub(super) fn validate_complete_unit_stack_evidence(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    function: UnitStackEvidence,
    calls: &[omega_machine_code::InternalCallRelocation],
    foreign_calls: &[ForeignCallRelocation],
    dynamic_calls: &[omega_machine_code::DynamicScalarCallRecord],
    inline_data: &[std::ops::Range<usize>],
) -> Result<(), ObjectError> {
    let mut claimed = std::collections::BTreeMap::new();
    let mut claim_pair = |pair: StackAdjustmentPair| {
        claimed
            .insert(pair.allocation_offset, pair.allocation_byte_count)
            .is_none()
            && claimed
                .insert(pair.release_offset, pair.release_byte_count)
                .is_none()
    };
    if function.frame.is_some_and(|frame| !claim_pair(frame)) {
        return Err(ObjectError::DuplicateUnitStackAdjustment(machine));
    }
    for call in calls {
        if let Some(outbound) = call.unit_stack.and_then(|stack| stack.outbound)
            && !claim_pair(outbound)
        {
            return Err(ObjectError::DuplicateUnitStackAdjustment(machine));
        }
    }
    for call in foreign_calls {
        if let Some(outbound) = call.unit_stack.outbound
            && !claim_pair(outbound)
        {
            return Err(ObjectError::DuplicateUnitStackAdjustment(machine));
        }
    }
    for call in dynamic_calls {
        if let Some(outbound) = call.unit_stack.outbound
            && !claim_pair(outbound)
        {
            return Err(ObjectError::DuplicateUnitStackAdjustment(machine));
        }
    }

    match architecture {
        Architecture::X86_64 => {
            let mut info_factory = iced_x86::InstructionInfoFactory::new();
            let call_starts = calls
                .iter()
                .map(|call| call.offset.saturating_sub(1))
                .chain(
                    foreign_calls
                        .iter()
                        .map(|call| call.offset.saturating_sub(1)),
                )
                .chain(dynamic_calls.iter().map(|call| call.indirect_call_offset))
                .collect::<std::collections::BTreeSet<_>>();
            for code in code_ranges(bytes.len(), inline_data) {
                let mut decoder = iced_x86::Decoder::with_ip(
                    64,
                    &bytes[code.clone()],
                    code.start as u64,
                    iced_x86::DecoderOptions::NONE,
                );
                while decoder.can_decode() {
                    let instruction = decoder.decode();
                    let offset =
                        usize::try_from(instruction.ip()).expect("function-relative x86 IP");
                    if instruction.is_invalid() {
                        return Err(ObjectError::InvalidUnitInstructionEncoding {
                            machine,
                            offset,
                        });
                    }
                    if is_x86_64_rsp_adjustment(&instruction) {
                        if claimed.remove(&offset) != Some(instruction.len()) {
                            return Err(ObjectError::UnclaimedUnitStackAdjustment {
                                machine,
                                offset,
                            });
                        }
                        continue;
                    }
                    let info = info_factory.info(&instruction);
                    let writes_stack_pointer = info.used_registers().iter().any(|register| {
                        matches!(
                            register.register(),
                            iced_x86::Register::RSP
                                | iced_x86::Register::ESP
                                | iced_x86::Register::SP
                                | iced_x86::Register::SPL
                        ) && matches!(
                            register.access(),
                            iced_x86::OpAccess::Write
                                | iced_x86::OpAccess::CondWrite
                                | iced_x86::OpAccess::ReadWrite
                                | iced_x86::OpAccess::ReadCondWrite
                        )
                    });
                    if writes_stack_pointer
                        && !is_expected_x86_64_linkage_instruction(
                            &instruction,
                            offset,
                            bytes.len(),
                            &call_starts,
                        )
                    {
                        return Err(ObjectError::UnclaimedUnitStackMutation { machine, offset });
                    }
                }
            }
        }
        Architecture::Aarch64 => {
            if !bytes.len().is_multiple_of(4) {
                return Err(ObjectError::InvalidUnitInstructionEncoding {
                    machine,
                    offset: bytes.len() - (bytes.len() % 4),
                });
            }
            for offset in (0..bytes.len()).step_by(4) {
                if inline_data
                    .iter()
                    .any(|data| offset < data.end && data.start < offset + 4)
                {
                    continue;
                }
                if aarch64_stack_adjustment_at(bytes, offset) {
                    if claimed.remove(&offset) != Some(4) {
                        return Err(ObjectError::UnclaimedUnitStackAdjustment { machine, offset });
                    }
                }
            }
        }
    }
    if let Some((offset, _)) = claimed.into_iter().next() {
        return Err(ObjectError::InvalidUnitStackEncoding {
            machine,
            owner: None,
            offset,
        });
    }
    Ok(())
}

fn code_ranges(
    byte_count: usize,
    inline_data: &[std::ops::Range<usize>],
) -> Vec<std::ops::Range<usize>> {
    let mut data = inline_data.to_vec();
    data.sort_unstable_by_key(|range| (range.start, range.end));
    let mut code = Vec::new();
    let mut cursor = 0;
    for range in data {
        if range.start > cursor {
            code.push(cursor..range.start.min(byte_count));
        }
        cursor = cursor.max(range.end.min(byte_count));
    }
    if cursor < byte_count {
        code.push(cursor..byte_count);
    }
    code
}

fn is_x86_64_rsp_adjustment(instruction: &iced_x86::Instruction) -> bool {
    matches!(
        instruction.mnemonic(),
        iced_x86::Mnemonic::Add | iced_x86::Mnemonic::Sub
    ) && instruction.op0_register() == iced_x86::Register::RSP
}

fn is_expected_x86_64_linkage_instruction(
    instruction: &iced_x86::Instruction,
    offset: usize,
    function_byte_count: usize,
    call_starts: &std::collections::BTreeSet<usize>,
) -> bool {
    match instruction.mnemonic() {
        iced_x86::Mnemonic::Call => call_starts.contains(&offset),
        iced_x86::Mnemonic::Ret => {
            offset.checked_add(instruction.len()) == Some(function_byte_count)
        }
        _ => false,
    }
}

pub(super) fn aarch64_stack_adjustment_at(bytes: &[u8], offset: usize) -> bool {
    let Some(encoded) = bytes
        .get(offset..offset.saturating_add(4))
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(u32::from_le_bytes)
    else {
        return false;
    };
    // ADD/SUB (immediate), 64-bit, without flags, whose destination register
    // is SP. This also catches the shifted-immediate form, which the Unit
    // emitter does not currently produce and therefore cannot claim.
    matches!(encoded & 0xff00_001f, 0xd100_001f | 0x9100_001f)
}

pub(super) fn x86_64_stack_adjustment(byte_size: u32, add: bool) -> Vec<u8> {
    if byte_size <= i8::MAX as u32 {
        vec![0x48, 0x83, if add { 0xc4 } else { 0xec }, byte_size as u8]
    } else {
        let mut bytes = vec![0x48, 0x81, if add { 0xc4 } else { 0xec }];
        bytes.extend_from_slice(&byte_size.to_le_bytes());
        bytes
    }
}

pub(super) fn x86_64_stack_release_preserving_flags(byte_size: u32) -> Vec<u8> {
    if byte_size <= i8::MAX as u32 {
        vec![0x48, 0x8d, 0x64, 0x24, byte_size as u8]
    } else {
        let mut bytes = vec![0x48, 0x8d, 0xa4, 0x24];
        bytes.extend_from_slice(&byte_size.to_le_bytes());
        bytes
    }
}

pub(super) fn aarch64_unit_link_instruction(load: bool, byte_offset: u32) -> u32 {
    let base = if load { 0xf940_0000 } else { 0xf900_0000 };
    base | ((byte_offset / 8) << 10) | (31 << 5) | 30
}
