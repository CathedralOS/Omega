//! Exact conditional scalar-stack replay.
//!
//! This module validates conditional-tree regions, terminal and crash leaves,
//! optional division subregions, and balanced stack/call evidence on both native
//! targets. It does not infer control flow, choose ABI placements, or emit
//! instructions.

use omega_target::Architecture;
use omega_terminal_machine_code::{
    TerminalScalarConditionalBranchEvidence, TerminalScalarConditionalCondition,
    TerminalScalarControlAffineCleanupRecord, TerminalScalarDivisionBranchEvidence,
    TerminalScalarStackEvidence, TerminalScalarStackMutation,
};
use omega_terminal_target_operations::TerminalCallSiteOwner;
use psi_core::MachineId;

use super::scalar_call_stack::validate_scalar_call_stack;
use super::scalar_conditional_regions::{
    collect_conditional_tree_regions, division_branches_in_region, validate_division_branch_regions,
};
use super::scalar_division_stack::replay_x86_scalar_division_region;
use super::scalar_stack_mutation::{
    aarch64_control_flow_instruction, aarch64_unsupported_sp_write, replay_scalar_mutation,
    validate_aarch64_scalar_mutation, validate_x86_scalar_mutation,
};
use super::unit_stack::aarch64_stack_adjustment_at;
use super::{
    TerminalObjectError, TerminalObjectScalarCallStack, TerminalObjectScalarStack,
    validate_internal_call_site,
};

#[allow(clippy::too_many_arguments)]
fn replay_scalar_conditional_region_with_divisions(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    require_return: bool,
    division_branches: &[TerminalScalarDivisionBranchEvidence],
    claimed: &mut std::collections::BTreeMap<usize, TerminalScalarStackMutation>,
    call_sites: &mut std::collections::BTreeMap<
        usize,
        omega_terminal_machine_code::TerminalInternalCallRelocation,
    >,
    allow_calls: bool,
    evidence: &TerminalScalarStackEvidence,
    validated_calls: &mut Vec<TerminalObjectScalarCallStack>,
) -> Result<u32, TerminalObjectError> {
    if division_branches.is_empty() {
        return replay_scalar_conditional_region(
            architecture,
            machine,
            bytes,
            start,
            end,
            require_return,
            claimed,
            call_sites,
            allow_calls,
            evidence,
            validated_calls,
            None,
        );
    }
    if architecture != Architecture::X86_64 || !allow_calls {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: start,
        });
    }
    replay_x86_scalar_division_region(
        machine,
        bytes,
        start,
        end,
        require_return,
        division_branches,
        claimed,
        call_sites,
        evidence,
        validated_calls,
    )
}

#[allow(clippy::too_many_arguments)]
fn replay_scalar_conditional_terminal_region(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    crash: bool,
    division_branches: &[TerminalScalarDivisionBranchEvidence],
    claimed: &mut std::collections::BTreeMap<usize, TerminalScalarStackMutation>,
    call_sites: &mut std::collections::BTreeMap<
        usize,
        omega_terminal_machine_code::TerminalInternalCallRelocation,
    >,
    evidence: &TerminalScalarStackEvidence,
    validated_calls: &mut Vec<TerminalObjectScalarCallStack>,
) -> Result<u32, TerminalObjectError> {
    if !crash {
        return replay_scalar_conditional_region_with_divisions(
            architecture,
            machine,
            bytes,
            start,
            end,
            true,
            division_branches,
            claimed,
            call_sites,
            true,
            evidence,
            validated_calls,
        );
    }
    if !division_branches.is_empty() {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: start,
        });
    }
    let crash_bytes: &[u8] = match architecture {
        Architecture::X86_64 => &[0x0f, 0x0b],
        Architecture::Aarch64 => &[0x00, 0x00, 0x20, 0xd4],
    };
    let crash_offset = end.checked_sub(crash_bytes.len()).ok_or(
        TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: start,
        },
    )?;
    if crash_offset < start || bytes.get(crash_offset..end) != Some(crash_bytes) {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: crash_offset,
        });
    }
    replay_scalar_conditional_region(
        architecture,
        machine,
        bytes,
        start,
        crash_offset,
        false,
        claimed,
        call_sites,
        true,
        evidence,
        validated_calls,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_conditional_tree_scalar_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    calls: &[omega_terminal_machine_code::TerminalInternalCallRelocation],
    evidence: &TerminalScalarStackEvidence,
    decisions: &[TerminalScalarConditionalBranchEvidence],
    crash_leaves: &[bool],
    division_branches: &[TerminalScalarDivisionBranchEvidence],
    cleanups: &[TerminalScalarControlAffineCleanupRecord],
) -> Result<
    (
        TerminalObjectScalarStack,
        Vec<TerminalObjectScalarCallStack>,
    ),
    TerminalObjectError,
> {
    if decisions.is_empty()
        || crash_leaves.len() != decisions.len() + 1
        || !cleanups.is_empty() && cleanups.len() != crash_leaves.len()
        || !cleanups.is_empty() && crash_leaves.iter().any(|crash| *crash)
        || !cleanups.is_empty() && !division_branches.is_empty()
        || evidence.cleanup_preservation.is_some()
        || decisions
            .windows(2)
            .any(|pair| pair[0].branch_offset >= pair[1].branch_offset)
        || evidence
            .mutations
            .windows(2)
            .any(|pair| pair[0].offset >= pair[1].offset)
    {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: decisions.first().map_or(0, |branch| branch.branch_offset),
        });
    }
    let mut prefixes = Vec::with_capacity(decisions.len());
    let mut leaves = Vec::with_capacity(crash_leaves.len());
    collect_conditional_tree_regions(
        architecture,
        machine,
        bytes,
        0,
        bytes.len(),
        decisions,
        &mut prefixes,
        &mut leaves,
    )?;
    if leaves.len() != crash_leaves.len() {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: decisions[0].branch_offset,
        });
    }
    let mut division_regions = prefixes
        .iter()
        .map(|(start, end, _)| (*start, *end))
        .collect::<Vec<_>>();
    division_regions.extend(leaves.iter().copied());
    validate_division_branch_regions(machine, division_branches, &division_regions)?;

    let mut claimed = evidence
        .mutations
        .iter()
        .map(|mutation| (mutation.offset, *mutation))
        .collect::<std::collections::BTreeMap<_, _>>();
    if claimed.len() != evidence.mutations.len() {
        return Err(TerminalObjectError::NonCanonicalScalarStackMutationOrder(
            machine,
        ));
    }
    let mut call_sites = std::collections::BTreeMap::new();
    for call in calls {
        validate_internal_call_site(architecture, machine, bytes, *call)?;
        let call_start = match architecture {
            Architecture::X86_64 => call.offset - 1,
            Architecture::Aarch64 => call.offset,
        };
        call_sites.insert(call_start, *call);
    }
    let mut validated_calls = Vec::with_capacity(calls.len());
    let mut peak = 0;
    for (start, end, condition) in prefixes {
        let prefix_peak = replay_scalar_conditional_region_with_divisions(
            architecture,
            machine,
            bytes,
            start,
            end,
            false,
            division_branches_in_region(division_branches, start, end),
            &mut claimed,
            &mut call_sites,
            condition == TerminalScalarConditionalCondition::Expression,
            evidence,
            &mut validated_calls,
        )?;
        if condition == TerminalScalarConditionalCondition::Parameter && prefix_peak != 0 {
            return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: end,
            });
        }
        peak = peak.max(prefix_peak);
    }
    for (index, (start, end)) in leaves.into_iter().enumerate() {
        let leaf_peak = if let Some(cleanup) = cleanups.get(index) {
            replay_scalar_conditional_region(
                architecture,
                machine,
                bytes,
                start,
                end,
                true,
                &mut claimed,
                &mut call_sites,
                true,
                evidence,
                &mut validated_calls,
                Some(&cleanup.cleanup),
            )?
        } else {
            replay_scalar_conditional_terminal_region(
                architecture,
                machine,
                bytes,
                start,
                end,
                crash_leaves[index],
                division_branches_in_region(division_branches, start, end),
                &mut claimed,
                &mut call_sites,
                evidence,
                &mut validated_calls,
            )?
        };
        peak = peak.max(leaf_peak);
    }
    if let Some((&offset, _)) = claimed.first_key_value() {
        return Err(TerminalObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    if let Some((&offset, call)) = call_sites.first_key_value() {
        return Err(TerminalObjectError::InvalidInternalCallSite {
            caller: machine,
            owner: call.owner,
            offset,
        });
    }
    Ok((
        TerminalObjectScalarStack {
            local_peak_bytes: peak,
            stack_alignment: evidence.stack_alignment,
        },
        validated_calls,
    ))
}

pub(super) fn replay_scalar_conditional_region(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    require_return: bool,
    claimed: &mut std::collections::BTreeMap<usize, TerminalScalarStackMutation>,
    call_sites: &mut std::collections::BTreeMap<
        usize,
        omega_terminal_machine_code::TerminalInternalCallRelocation,
    >,
    allow_calls: bool,
    evidence: &TerminalScalarStackEvidence,
    validated_calls: &mut Vec<TerminalObjectScalarCallStack>,
    scalar_affine_cleanup: Option<&omega_terminal_machine_code::TerminalUnitAffineCleanupRecord>,
) -> Result<u32, TerminalObjectError> {
    if start > end || end > bytes.len() {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: start,
        });
    }
    let mut depth = 0_u32;
    let mut peak = 0_u32;
    let mut saw_return = false;
    match architecture {
        Architecture::X86_64 => {
            let mut decoder = iced_x86::Decoder::with_ip(
                64,
                &bytes[start..end],
                start as u64,
                iced_x86::DecoderOptions::NONE,
            );
            let mut info_factory = iced_x86::InstructionInfoFactory::new();
            while decoder.can_decode() {
                let instruction = decoder.decode();
                let offset = usize::try_from(instruction.ip()).expect("function-relative x86 IP");
                if instruction.is_invalid() {
                    return Err(TerminalObjectError::InvalidScalarInstructionEncoding {
                        machine,
                        offset,
                    });
                }
                if instruction.mnemonic() == iced_x86::Mnemonic::Ret {
                    if !require_return
                        || offset.checked_add(instruction.len()) != Some(end)
                        || saw_return
                    {
                        return Err(TerminalObjectError::NonLinearScalarControlFlow {
                            machine,
                            offset,
                        });
                    }
                    saw_return = true;
                    continue;
                }
                if instruction.mnemonic() == iced_x86::Mnemonic::Call {
                    let call = call_sites.remove(&offset).ok_or(
                        TerminalObjectError::UntypedScalarInternalCall { machine, offset },
                    )?;
                    match call.owner {
                        TerminalCallSiteOwner::Operation(operation) if !allow_calls => {
                            return Err(TerminalObjectError::ScalarConditionalCallOutsideArm {
                                machine,
                                operation,
                                offset,
                            });
                        }
                        TerminalCallSiteOwner::Operation(_) if allow_calls => {}
                        TerminalCallSiteOwner::CleanupAction { edge, .. }
                            if allow_calls
                                && scalar_affine_cleanup
                                    .is_some_and(|cleanup| cleanup.psi_edge == edge) => {}
                        _ => {
                            return Err(TerminalObjectError::UntypedScalarInternalCall {
                                machine,
                                offset,
                            });
                        }
                    }
                    let call_evidence = call.scalar_stack.ok_or(
                        TerminalObjectError::MissingScalarCallStackEvidence {
                            caller: machine,
                            owner: call.owner,
                        },
                    )?;
                    let validated = validate_scalar_call_stack(
                        architecture,
                        machine,
                        bytes,
                        call,
                        call_evidence,
                        evidence,
                        depth,
                        scalar_affine_cleanup,
                    )?;
                    peak = peak.max(validated.caller_live_bytes);
                    validated_calls.push(validated);
                    continue;
                }
                if instruction.flow_control() != iced_x86::FlowControl::Next {
                    return Err(TerminalObjectError::NonLinearScalarControlFlow {
                        machine,
                        offset,
                    });
                }
                let stack_mutation = matches!(
                    instruction.mnemonic(),
                    iced_x86::Mnemonic::Push | iced_x86::Mnemonic::Pop
                ) || matches!(
                    instruction.mnemonic(),
                    iced_x86::Mnemonic::Add | iced_x86::Mnemonic::Sub
                ) && instruction.op0_register()
                    == iced_x86::Register::RSP
                    || instruction.mnemonic() == iced_x86::Mnemonic::Lea
                        && instruction.op0_register() == iced_x86::Register::RSP;
                if stack_mutation {
                    let mutation = claimed.remove(&offset).ok_or(
                        TerminalObjectError::UnclaimedScalarStackMutation { machine, offset },
                    )?;
                    validate_x86_scalar_mutation(machine, bytes, &instruction, mutation)?;
                    replay_scalar_mutation(machine, offset, mutation.kind, &mut depth, &mut peak)?;
                    continue;
                }
                let info = info_factory.info(&instruction);
                if info.used_registers().iter().any(|register| {
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
                }) {
                    return Err(TerminalObjectError::UnsupportedScalarStackMutation {
                        machine,
                        offset,
                    });
                }
            }
        }
        Architecture::Aarch64 => {
            if !start.is_multiple_of(4) || !end.is_multiple_of(4) {
                return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                    machine,
                    offset: start,
                });
            }
            for offset in (start..end).step_by(4) {
                let encoded = u32::from_le_bytes(
                    bytes[offset..offset + 4]
                        .try_into()
                        .expect("four-byte AArch64 word"),
                );
                if encoded == 0xd65f_03c0 {
                    if !require_return || offset + 4 != end || saw_return {
                        return Err(TerminalObjectError::NonLinearScalarControlFlow {
                            machine,
                            offset,
                        });
                    }
                    saw_return = true;
                    continue;
                }
                if encoded == 0x9400_0000 {
                    let call = call_sites.remove(&offset).ok_or(
                        TerminalObjectError::UntypedScalarInternalCall { machine, offset },
                    )?;
                    match call.owner {
                        TerminalCallSiteOwner::Operation(operation) if !allow_calls => {
                            return Err(TerminalObjectError::ScalarConditionalCallOutsideArm {
                                machine,
                                operation,
                                offset,
                            });
                        }
                        TerminalCallSiteOwner::Operation(_) if allow_calls => {}
                        TerminalCallSiteOwner::CleanupAction { edge, .. }
                            if allow_calls
                                && scalar_affine_cleanup
                                    .is_some_and(|cleanup| cleanup.psi_edge == edge) => {}
                        _ => {
                            return Err(TerminalObjectError::UntypedScalarInternalCall {
                                machine,
                                offset,
                            });
                        }
                    }
                    let call_evidence = call.scalar_stack.ok_or(
                        TerminalObjectError::MissingScalarCallStackEvidence {
                            caller: machine,
                            owner: call.owner,
                        },
                    )?;
                    let validated = validate_scalar_call_stack(
                        architecture,
                        machine,
                        bytes,
                        call,
                        call_evidence,
                        evidence,
                        depth,
                        scalar_affine_cleanup,
                    )?;
                    peak = peak.max(validated.caller_live_bytes);
                    validated_calls.push(validated);
                    continue;
                }
                if aarch64_control_flow_instruction(encoded) {
                    return Err(TerminalObjectError::NonLinearScalarControlFlow {
                        machine,
                        offset,
                    });
                }
                if aarch64_stack_adjustment_at(bytes, offset) {
                    let mutation = claimed.remove(&offset).ok_or(
                        TerminalObjectError::UnclaimedScalarStackMutation { machine, offset },
                    )?;
                    validate_aarch64_scalar_mutation(machine, encoded, mutation)?;
                    replay_scalar_mutation(machine, offset, mutation.kind, &mut depth, &mut peak)?;
                } else if aarch64_unsupported_sp_write(encoded) {
                    return Err(TerminalObjectError::UnsupportedScalarStackMutation {
                        machine,
                        offset,
                    });
                }
            }
        }
    }
    if require_return != saw_return || depth != 0 {
        return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok(peak)
}
