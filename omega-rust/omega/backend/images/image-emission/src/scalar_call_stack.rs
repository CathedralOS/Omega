//! Exact scalar internal-call stack replay.
//!
//! This module validates retained outbound-frame and AArch64 return-link
//! evidence around one scalar call, then reconstructs its aligned caller-live
//! depth. It does not select calls, mutate stack evidence, or emit bytes.

use machine_code::{
    InternalCallRelocation, ScalarCallStackEvidence, ScalarStackEvidence, ScalarStackMutationKind,
    UnitAffineCleanupRecord,
};
use semantic_vocabulary::MachineId;
use target::Architecture;
use target_operations::CallSiteOwner;

use super::unit_stack::{aarch64_unit_link_instruction, validate_stack_adjustment_pair};
use super::{ObjectError, ObjectScalarCallStack};

pub(super) fn validate_scalar_call_stack(
    architecture: Architecture,
    caller: MachineId,
    bytes: &[u8],
    relocation: InternalCallRelocation,
    call: ScalarCallStackEvidence,
    function: &ScalarStackEvidence,
    replay_depth: u32,
    scalar_affine_cleanup: Option<&UnitAffineCleanupRecord>,
) -> Result<ObjectScalarCallStack, ObjectError> {
    let owner = relocation.owner;
    let (call_start, call_end) = match architecture {
        Architecture::X86_64 => (relocation.offset - 1, relocation.offset + 4),
        Architecture::Aarch64 => (relocation.offset, relocation.offset + 4),
    };
    if let Some(outbound) = call.outbound {
        validate_stack_adjustment_pair(architecture, caller, Some(owner), bytes, outbound)
            .map_err(|_| ObjectError::InvalidScalarCallStackEvidence {
                caller,
                owner,
                offset: outbound.allocation_offset,
            })?;
        let allocation = function
            .mutations
            .iter()
            .find(|mutation| mutation.offset == outbound.allocation_offset);
        let release = function
            .mutations
            .iter()
            .find(|mutation| mutation.offset == outbound.release_offset);
        if allocation.is_none_or(|mutation| {
            mutation.byte_count != outbound.allocation_byte_count
                || mutation.kind
                    != ScalarStackMutationKind::Allocate {
                        byte_size: outbound.byte_size,
                    }
        }) || release.is_none_or(|mutation| {
            mutation.byte_count != outbound.release_byte_count
                || mutation.kind
                    != ScalarStackMutationKind::Release {
                        byte_size: outbound.byte_size,
                    }
        }) {
            return Err(ObjectError::InvalidScalarCallStackEvidence {
                caller,
                owner,
                offset: outbound.allocation_offset,
            });
        }
        let allocation_end = outbound
            .allocation_offset
            .checked_add(outbound.allocation_byte_count)
            .ok_or(ObjectError::ScalarStackArithmeticOverflow(caller))?;
        if allocation_end > call_start
            || (architecture == Architecture::X86_64 && outbound.release_offset != call_end)
        {
            return Err(ObjectError::InvalidScalarCallStackEvidence {
                caller,
                owner,
                offset: outbound.allocation_offset,
            });
        }
    }
    match architecture {
        Architecture::X86_64 => {
            if call.aarch64_return_link.is_some() {
                return Err(ObjectError::InvalidScalarCallStackEvidence {
                    caller,
                    owner,
                    offset: call_start,
                });
            }
        }
        Architecture::Aarch64 => {
            let cleanup_lifetime_link = matches!(owner, CallSiteOwner::CleanupAction { .. })
                && call.outbound.is_none()
                && call.aarch64_return_link.is_none()
                && replay_depth >= function.stack_alignment
                && replay_depth.is_multiple_of(function.stack_alignment)
                && scalar_affine_cleanup.is_some_and(|cleanup| {
                    cleanup.code_offset <= call_start
                        && cleanup
                            .code_offset
                            .checked_add(cleanup.byte_count)
                            .is_some_and(|end| call_end <= end)
                });
            if cleanup_lifetime_link {
                // The composite scalar-return carrier keeps X30 in its
                // function-lifetime cleanup frame, so this call requires no
                // second per-call link slot.
            } else {
                let outbound =
                    call.outbound
                        .ok_or(ObjectError::InvalidScalarCallStackEvidence {
                            caller,
                            owner,
                            offset: call_start,
                        })?;
                let link = call.aarch64_return_link.ok_or(
                    ObjectError::InvalidScalarCallStackEvidence {
                        caller,
                        owner,
                        offset: call_start,
                    },
                )?;
                let link_end = link
                    .frame_byte_offset
                    .checked_add(8)
                    .ok_or(ObjectError::ScalarStackArithmeticOverflow(caller))?;
                let link_area_end = link
                    .frame_byte_offset
                    .checked_add(16)
                    .ok_or(ObjectError::ScalarStackArithmeticOverflow(caller))?;
                let allocation_end = outbound.allocation_offset + outbound.allocation_byte_count;
                if !link.frame_byte_offset.is_multiple_of(8)
                    || link_end > outbound.byte_size
                    || link_area_end != outbound.byte_size
                    || link.store_offset != allocation_end
                    || link.store_offset >= call_start
                    || link.load_offset != call_end
                    || outbound.release_offset != link.load_offset + 4
                    || bytes.get(link.store_offset..link.store_offset + 4)
                        != Some(
                            &aarch64_unit_link_instruction(false, link.frame_byte_offset)
                                .to_le_bytes(),
                        )
                    || bytes.get(link.load_offset..link.load_offset + 4)
                        != Some(
                            &aarch64_unit_link_instruction(true, link.frame_byte_offset)
                                .to_le_bytes(),
                        )
                {
                    return Err(ObjectError::InvalidScalarCallStackEvidence {
                        caller,
                        owner,
                        offset: link.store_offset,
                    });
                }
            }
        }
    }
    let caller_live_bytes = replay_depth
        .checked_add(if architecture == Architecture::X86_64 {
            8
        } else {
            0
        })
        .ok_or(ObjectError::ScalarStackArithmeticOverflow(caller))?;
    if !caller_live_bytes.is_multiple_of(function.stack_alignment) {
        return Err(ObjectError::MisalignedScalarCalleeEntry {
            caller,
            owner,
            caller_live_bytes,
        });
    }
    Ok(ObjectScalarCallStack {
        owner,
        target: relocation.target,
        text_offset: relocation.offset,
        caller_live_bytes,
    })
}
