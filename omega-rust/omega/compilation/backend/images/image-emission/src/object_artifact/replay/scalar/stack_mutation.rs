//! Exact scalar stack-mutation byte validation and depth replay.
//!
//! This module recognizes only the target instructions admitted by retained
//! scalar stack evidence and recomputes their depth effects. It does not emit
//! instructions or select stack layouts.

use machine_code::{ScalarStackEvidence, ScalarStackMutation, ScalarStackMutationKind};
use semantic_vocabulary::MachineId;

use super::super::unit::stack::{x86_64_stack_adjustment, x86_64_stack_release_preserving_flags};
use crate::{ObjectError, ObjectScalarCallStack, ObjectScalarStack};
use target::Architecture;

pub(crate) fn validate_x86_scalar_mutation(
    machine: MachineId,
    bytes: &[u8],
    instruction: &iced_x86::Instruction,
    mutation: ScalarStackMutation,
) -> Result<(), ObjectError> {
    let offset = mutation.offset;
    let exact = match mutation.kind {
        ScalarStackMutationKind::Allocate { byte_size } => {
            instruction.mnemonic() == iced_x86::Mnemonic::Sub
                && instruction.op0_register() == iced_x86::Register::RSP
                && bytes.get(offset..offset.saturating_add(instruction.len()))
                    == Some(x86_64_stack_adjustment(byte_size, false).as_slice())
        }
        ScalarStackMutationKind::Release { byte_size } => {
            instruction.mnemonic() == iced_x86::Mnemonic::Add
                && instruction.op0_register() == iced_x86::Register::RSP
                && bytes.get(offset..offset.saturating_add(instruction.len()))
                    == Some(x86_64_stack_adjustment(byte_size, true).as_slice())
        }
        ScalarStackMutationKind::X86ReleasePreservingFlags { byte_size } => {
            instruction.mnemonic() == iced_x86::Mnemonic::Lea
                && instruction.op0_register() == iced_x86::Register::RSP
                && bytes.get(offset..offset.saturating_add(instruction.len()))
                    == Some(x86_64_stack_release_preserving_flags(byte_size).as_slice())
        }
        ScalarStackMutationKind::X86Push => {
            instruction.mnemonic() == iced_x86::Mnemonic::Push
                && instruction.op0_kind() == iced_x86::OpKind::Register
        }
        ScalarStackMutationKind::X86Pop => {
            instruction.mnemonic() == iced_x86::Mnemonic::Pop
                && instruction.op0_kind() == iced_x86::OpKind::Register
        }
    };
    if !exact || mutation.byte_count != instruction.len() {
        return Err(ObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    Ok(())
}

pub(crate) fn validate_aarch64_scalar_mutation(
    machine: MachineId,
    encoded: u32,
    mutation: ScalarStackMutation,
) -> Result<(), ObjectError> {
    let expected = match mutation.kind {
        ScalarStackMutationKind::Allocate { byte_size }
            if byte_size <= 0xfff && byte_size.is_multiple_of(16) =>
        {
            Some(0xd100_03ff | (byte_size << 10))
        }
        ScalarStackMutationKind::Release { byte_size }
            if byte_size <= 0xfff && byte_size.is_multiple_of(16) =>
        {
            Some(0x9100_03ff | (byte_size << 10))
        }
        _ => None,
    };
    if mutation.byte_count != 4 || expected != Some(encoded) {
        return Err(ObjectError::InvalidScalarStackEvidence {
            machine,
            offset: mutation.offset,
        });
    }
    Ok(())
}

pub(crate) fn replay_scalar_mutation(
    machine: MachineId,
    offset: usize,
    kind: ScalarStackMutationKind,
    depth: &mut u32,
    peak: &mut u32,
) -> Result<(), ObjectError> {
    let (allocate, byte_size) = match kind {
        ScalarStackMutationKind::Allocate { byte_size } => (true, byte_size),
        ScalarStackMutationKind::Release { byte_size } => (false, byte_size),
        ScalarStackMutationKind::X86ReleasePreservingFlags { byte_size } => (false, byte_size),
        ScalarStackMutationKind::X86Push => (true, 8),
        ScalarStackMutationKind::X86Pop => (false, 8),
    };
    if byte_size == 0 {
        return Err(ObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    if allocate {
        *depth = depth
            .checked_add(byte_size)
            .ok_or(ObjectError::ScalarStackArithmeticOverflow(machine))?;
        *peak = (*peak).max(*depth);
    } else {
        *depth = depth
            .checked_sub(byte_size)
            .ok_or(ObjectError::ScalarStackReleaseExceedsAllocation { machine, offset })?;
    }
    Ok(())
}

pub(crate) fn aarch64_control_flow_instruction(encoded: u32) -> bool {
    (encoded & 0x7c00_0000) == 0x1400_0000
        || (encoded & 0xff00_0010) == 0x5400_0000
        || (encoded & 0x7e00_0000) == 0x3400_0000
        || (encoded & 0x7e00_0000) == 0x3600_0000
        || (encoded & 0xfe00_0000) == 0xd600_0000
        || (encoded & 0xff00_0000) == 0xd400_0000
}

pub(crate) fn aarch64_unsupported_sp_write(encoded: u32) -> bool {
    // ADD/SUB extended-register forms may name SP as destination. Scalar
    // emission never uses them for stack allocation.
    matches!(encoded & 0xff20_001f, 0x8b20_001f | 0xcb20_001f)
        // Single-register immediate pre/post-indexed loads and stores update
        // their SP base. Scalar emission uses only unsigned-offset accesses.
        || ((encoded & 0x3b20_0000) == 0x3800_0000
            && matches!((encoded >> 10) & 3, 1 | 3)
            && ((encoded >> 5) & 31) == 31)
        // Pair pre/post-indexed loads and stores likewise update the base.
        || ((encoded & 0x3a00_0000) == 0x2800_0000
            && matches!((encoded >> 23) & 3, 1 | 3)
            && ((encoded >> 5) & 31) == 31)
}

/// The canonical replay prelude: stack mutations keyed by offset (rejecting
/// duplicate offsets, which the canonical evidence order already forbids) and
/// validated call sites keyed by their architecture-specific call start.
pub(crate) fn claimed_mutations_and_call_sites(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    evidence: &ScalarStackEvidence,
    calls: &[machine_code::InternalCallRelocation],
) -> Result<
    (
        std::collections::BTreeMap<usize, ScalarStackMutation>,
        std::collections::BTreeMap<usize, machine_code::InternalCallRelocation>,
    ),
    ObjectError,
> {
    let claimed = evidence
        .mutations
        .iter()
        .map(|mutation| (mutation.offset, *mutation))
        .collect::<std::collections::BTreeMap<_, _>>();
    if claimed.len() != evidence.mutations.len() {
        return Err(ObjectError::NonCanonicalScalarStackMutationOrder(machine));
    }
    let mut call_sites = std::collections::BTreeMap::new();
    for call in calls {
        crate::object_artifact::call_sites::validate_internal_call_site(
            architecture,
            machine,
            bytes,
            *call,
        )?;
        let call_start = match architecture {
            Architecture::X86_64 => call.offset - 1,
            Architecture::Aarch64 => call.offset,
        };
        call_sites.insert(call_start, *call);
    }
    Ok((claimed, call_sites))
}

/// Drain the claimed-mutation and call-site ledgers into the validated
/// scalar stack answer: the first unclaimed mutation is invalid scalar
/// stack evidence and the first unclaimed call site an invalid internal
/// call site, so empty ledgers mean every observed byte and relocation
/// joined the replay. `late_error` carries any replay-specific failure
/// (e.g. unbalanced returns) that still ranks behind the ledger drains.
pub(crate) fn scalar_stack_result(
    claimed: &std::collections::BTreeMap<usize, ScalarStackMutation>,
    call_sites: &std::collections::BTreeMap<usize, machine_code::InternalCallRelocation>,
    machine: MachineId,
    peak: u32,
    stack_alignment: u32,
    validated_calls: Vec<ObjectScalarCallStack>,
    late_error: Option<ObjectError>,
) -> Result<(ObjectScalarStack, Vec<ObjectScalarCallStack>), ObjectError> {
    if let Some((&offset, _)) = claimed.first_key_value() {
        return Err(ObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    if let Some((&offset, call)) = call_sites.first_key_value() {
        return Err(ObjectError::InvalidInternalCallSite {
            caller: machine,
            owner: call.owner,
            offset,
        });
    }
    if let Some(error) = late_error {
        return Err(error);
    }
    Ok((
        ObjectScalarStack {
            local_peak_bytes: peak,
            stack_alignment,
        },
        validated_calls,
    ))
}
