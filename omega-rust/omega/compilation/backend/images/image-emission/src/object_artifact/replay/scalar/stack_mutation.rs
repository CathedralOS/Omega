//! Exact scalar stack-mutation byte validation and depth replay.
//!
//! This module recognizes only the target instructions admitted by retained
//! scalar stack evidence and recomputes their depth effects, and it owns the
//! one call-site replay step the linear, division, and conditional walks
//! share. It does not emit instructions or select stack layouts.

use std::collections::BTreeMap;

use machine_code::{
    InternalCallRelocation, ScalarStackEvidence, ScalarStackMutation, ScalarStackMutationKind,
    UnitAffineCleanupRecord,
};
use semantic_vocabulary::MachineId;
use target_operations::CallSiteOwner;

use super::super::unit::stack::{x86_64_stack_adjustment, x86_64_stack_release_preserving_flags};
use super::call_stack::scalar_call_stack_at;
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
        BTreeMap<usize, ScalarStackMutation>,
        BTreeMap<usize, InternalCallRelocation>,
    ),
    ObjectError,
> {
    let claimed = evidence
        .mutations
        .iter()
        .map(|mutation| (mutation.offset, *mutation))
        .collect::<BTreeMap<_, _>>();
    if claimed.len() != evidence.mutations.len() {
        return Err(ObjectError::NonCanonicalScalarStackMutationOrder(machine));
    }
    let mut call_sites = BTreeMap::new();
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

/// The one scalar stack replay ledger: the fixed frame each walk replays
/// against, the claimed mutations and call sites it drains, and the validated
/// calls it produces. The linear, division, and conditional walks claim
/// mutations and replay call sites only through this owner, so the arm policy
/// and the caller-live-byte fold are spelled once and the walks differ only in
/// instruction recognition.
pub(crate) struct ScalarReplay<'a> {
    architecture: Architecture,
    machine: MachineId,
    bytes: &'a [u8],
    evidence: &'a ScalarStackEvidence,
    claimed: BTreeMap<usize, ScalarStackMutation>,
    call_sites: BTreeMap<usize, InternalCallRelocation>,
    calls: Vec<ObjectScalarCallStack>,
}

impl<'a> ScalarReplay<'a> {
    pub(crate) fn new(
        architecture: Architecture,
        machine: MachineId,
        bytes: &'a [u8],
        evidence: &'a ScalarStackEvidence,
        calls: &[InternalCallRelocation],
    ) -> Result<Self, ObjectError> {
        let (claimed, call_sites) =
            claimed_mutations_and_call_sites(architecture, machine, bytes, evidence, calls)?;
        Ok(Self {
            architecture,
            machine,
            bytes,
            evidence,
            claimed,
            call_sites,
            calls: Vec::with_capacity(calls.len()),
        })
    }

    /// Claims the mutation evidence at `offset`; every stack-mutating
    /// instruction removes exactly one row.
    pub(crate) fn claim(&mut self, offset: usize) -> Result<ScalarStackMutation, ObjectError> {
        self.claimed
            .remove(&offset)
            .ok_or(ObjectError::UnclaimedScalarStackMutation {
                machine: self.machine,
                offset,
            })
    }

    /// Replays the call site at `offset` against the enclosing arm's policy:
    /// the relocation must be claimed, admitted by the arm, and its measured
    /// caller-live bytes raise `peak`.
    pub(crate) fn call(
        &mut self,
        offset: usize,
        allow_calls: bool,
        cleanup: Option<&UnitAffineCleanupRecord>,
        depth: u32,
        peak: &mut u32,
    ) -> Result<(), ObjectError> {
        let machine = self.machine;
        let call = self
            .call_sites
            .remove(&offset)
            .ok_or(ObjectError::UntypedScalarInternalCall { machine, offset })?;
        match call.owner {
            CallSiteOwner::Operation(operation) if !allow_calls => {
                return Err(ObjectError::ScalarConditionalCallOutsideArm {
                    machine,
                    operation,
                    offset,
                });
            }
            CallSiteOwner::Operation(_) if allow_calls => {}
            CallSiteOwner::CleanupAction { edge, .. }
                if allow_calls && cleanup.is_some_and(|cleanup| cleanup.psi_edge == edge) => {}
            _ => return Err(ObjectError::UntypedScalarInternalCall { machine, offset }),
        }
        let validated = scalar_call_stack_at(
            self.architecture,
            machine,
            self.bytes,
            call,
            self.evidence,
            depth,
            cleanup,
        )?;
        *peak = (*peak).max(validated.caller_live_bytes);
        self.calls.push(validated);
        Ok(())
    }

    /// Drains both ledgers into the validated scalar stack answer, ranking any
    /// replay-specific failure behind them.
    pub(crate) fn finish(
        self,
        peak: u32,
        late_error: Option<ObjectError>,
    ) -> Result<(ObjectScalarStack, Vec<ObjectScalarCallStack>), ObjectError> {
        scalar_stack_result(
            &self.claimed,
            &self.call_sites,
            self.machine,
            peak,
            self.evidence.stack_alignment,
            self.calls,
            late_error,
        )
    }
}

/// Drain the claimed-mutation and call-site ledgers into the validated
/// scalar stack answer: the first unclaimed mutation is invalid scalar
/// stack evidence and the first unclaimed call site an invalid internal
/// call site, so empty ledgers mean every observed byte and relocation
/// joined the replay. `late_error` carries any replay-specific failure
/// (e.g. unbalanced returns) that still ranks behind the ledger drains.
pub(crate) fn scalar_stack_result(
    claimed: &BTreeMap<usize, ScalarStackMutation>,
    call_sites: &BTreeMap<usize, InternalCallRelocation>,
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
