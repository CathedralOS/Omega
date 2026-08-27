//! Exact x86-64 scalar division-stack replay.
//!
//! This module validates branch geometry and exact instructions while replaying
//! balanced scalar stack mutations and typed internal calls across ordinary and
//! exceptional division regions. It does not infer control-flow evidence,
//! select calling conventions, or emit instructions.

use omega_target::Architecture;
use omega_terminal_machine_code::{
    TerminalScalarDivisionBranchEvidence, TerminalScalarStackEvidence, TerminalScalarStackMutation,
};
use psi_core::MachineId;

use super::scalar_call_stack::validate_scalar_call_stack;
use super::scalar_stack_mutation::{replay_scalar_mutation, validate_x86_scalar_mutation};
use super::{
    TerminalObjectError, TerminalObjectScalarCallStack, TerminalObjectScalarStack,
    validate_internal_call_site,
};

pub(super) fn validate_linear_scalar_division_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    calls: &[omega_terminal_machine_code::TerminalInternalCallRelocation],
    evidence: &TerminalScalarStackEvidence,
    branches: &[TerminalScalarDivisionBranchEvidence],
) -> Result<
    (
        TerminalObjectScalarStack,
        Vec<TerminalObjectScalarCallStack>,
    ),
    TerminalObjectError,
> {
    if architecture != Architecture::X86_64 || branches.is_empty() {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence { machine, offset: 0 });
    }
    if evidence
        .mutations
        .windows(2)
        .any(|pair| pair[0].offset >= pair[1].offset)
    {
        return Err(TerminalObjectError::NonCanonicalScalarStackMutationOrder(
            machine,
        ));
    }
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
        call_sites.insert(call.offset - 1, *call);
    }
    let mut validated_calls = Vec::with_capacity(calls.len());
    let mut cursor = 0;
    let mut depth = 0_u32;
    let mut peak = 0_u32;
    for branch in branches {
        let branch_end = branch
            .branch_offset
            .checked_add(branch.branch_byte_count)
            .ok_or(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.branch_offset,
            })?;
        let join_end = branch
            .join_offset
            .checked_add(branch.join_byte_count)
            .ok_or(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.join_offset,
            })?;
        if cursor > branch.branch_offset
            || branch.branch_offset >= branch_end
            || branch_end > branch.join_offset
            || join_end != branch.ordinary_arm_offset
            || branch.ordinary_arm_offset >= branch.merge_offset
            || branch.merge_offset > bytes.len()
        {
            return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.branch_offset,
            });
        }
        let conditional = decode_exact_x86_instruction(
            machine,
            bytes,
            branch.branch_offset,
            branch.branch_byte_count,
        )?;
        let join = decode_exact_x86_instruction(
            machine,
            bytes,
            branch.join_offset,
            branch.join_byte_count,
        )?;
        if conditional.mnemonic() != iced_x86::Mnemonic::Jne
            || conditional.flow_control() != iced_x86::FlowControl::ConditionalBranch
            || usize::try_from(conditional.near_branch_target()).ok()
                != Some(branch.ordinary_arm_offset)
            || join.mnemonic() != iced_x86::Mnemonic::Jmp
            || join.flow_control() != iced_x86::FlowControl::UnconditionalBranch
            || usize::try_from(join.near_branch_target()).ok() != Some(branch.merge_offset)
        {
            return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.branch_offset,
            });
        }
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            cursor,
            branch.branch_offset,
            false,
            &mut claimed,
            &mut call_sites,
            evidence,
            &mut validated_calls,
            &mut depth,
            &mut peak,
        )?;
        let branch_depth = depth;
        let mut special_depth = branch_depth;
        let mut special_peak = peak;
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            branch_end,
            branch.join_offset,
            false,
            &mut claimed,
            &mut call_sites,
            evidence,
            &mut validated_calls,
            &mut special_depth,
            &mut special_peak,
        )?;
        let mut ordinary_depth = branch_depth;
        let mut ordinary_peak = peak;
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            branch.ordinary_arm_offset,
            branch.merge_offset,
            false,
            &mut claimed,
            &mut call_sites,
            evidence,
            &mut validated_calls,
            &mut ordinary_depth,
            &mut ordinary_peak,
        )?;
        if special_depth != ordinary_depth {
            return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
        }
        depth = special_depth;
        peak = special_peak.max(ordinary_peak);
        cursor = branch.merge_offset;
    }
    replay_x86_scalar_linear_region(
        machine,
        bytes,
        cursor,
        bytes.len(),
        true,
        &mut claimed,
        &mut call_sites,
        evidence,
        &mut validated_calls,
        &mut depth,
        &mut peak,
    )?;
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
    if depth != 0 {
        return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok((
        TerminalObjectScalarStack {
            local_peak_bytes: peak,
            stack_alignment: evidence.stack_alignment,
        },
        validated_calls,
    ))
}

pub(super) fn decode_exact_x86_instruction(
    machine: MachineId,
    bytes: &[u8],
    offset: usize,
    byte_count: usize,
) -> Result<iced_x86::Instruction, TerminalObjectError> {
    let end = offset
        .checked_add(byte_count)
        .filter(|end| *end <= bytes.len())
        .ok_or(TerminalObjectError::InvalidScalarConditionalEvidence { machine, offset })?;
    let mut decoder = iced_x86::Decoder::with_ip(
        64,
        &bytes[offset..end],
        offset as u64,
        iced_x86::DecoderOptions::NONE,
    );
    let instruction = decoder.decode();
    if instruction.is_invalid() || instruction.len() != byte_count || decoder.can_decode() {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence { machine, offset });
    }
    Ok(instruction)
}
fn replay_x86_scalar_linear_region(
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
    evidence: &TerminalScalarStackEvidence,
    validated_calls: &mut Vec<TerminalObjectScalarCallStack>,
    depth: &mut u32,
    peak: &mut u32,
) -> Result<(), TerminalObjectError> {
    if start > end || end > bytes.len() {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: start,
        });
    }
    let mut decoder = iced_x86::Decoder::with_ip(
        64,
        &bytes[start..end],
        start as u64,
        iced_x86::DecoderOptions::NONE,
    );
    let mut info_factory = iced_x86::InstructionInfoFactory::new();
    let mut saw_return = false;
    while decoder.can_decode() {
        let instruction = decoder.decode();
        let offset = usize::try_from(instruction.ip()).expect("function-relative x86 IP");
        if instruction.is_invalid() {
            return Err(TerminalObjectError::InvalidScalarInstructionEncoding { machine, offset });
        }
        if instruction.mnemonic() == iced_x86::Mnemonic::Ret {
            if !require_return || offset.checked_add(instruction.len()) != Some(end) || saw_return {
                return Err(TerminalObjectError::NonLinearScalarControlFlow { machine, offset });
            }
            saw_return = true;
            continue;
        }
        if instruction.mnemonic() == iced_x86::Mnemonic::Call {
            let call = call_sites
                .remove(&offset)
                .ok_or(TerminalObjectError::UntypedScalarInternalCall { machine, offset })?;
            let call_evidence =
                call.scalar_stack
                    .ok_or(TerminalObjectError::MissingScalarCallStackEvidence {
                        caller: machine,
                        owner: call.owner,
                    })?;
            let validated = validate_scalar_call_stack(
                Architecture::X86_64,
                machine,
                bytes,
                call,
                call_evidence,
                evidence,
                *depth,
                None,
            )?;
            *peak = (*peak).max(validated.caller_live_bytes);
            validated_calls.push(validated);
            continue;
        }
        if instruction.flow_control() != iced_x86::FlowControl::Next {
            return Err(TerminalObjectError::NonLinearScalarControlFlow { machine, offset });
        }
        let stack_mutation = matches!(
            instruction.mnemonic(),
            iced_x86::Mnemonic::Push | iced_x86::Mnemonic::Pop
        ) || matches!(
            instruction.mnemonic(),
            iced_x86::Mnemonic::Add | iced_x86::Mnemonic::Sub
        ) && instruction.op0_register() == iced_x86::Register::RSP
            || instruction.mnemonic() == iced_x86::Mnemonic::Lea
                && instruction.op0_register() == iced_x86::Register::RSP;
        if stack_mutation {
            let mutation = claimed
                .remove(&offset)
                .ok_or(TerminalObjectError::UnclaimedScalarStackMutation { machine, offset })?;
            validate_x86_scalar_mutation(machine, bytes, &instruction, mutation)?;
            replay_scalar_mutation(machine, offset, mutation.kind, depth, peak)?;
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
            return Err(TerminalObjectError::UnsupportedScalarStackMutation { machine, offset });
        }
    }
    if require_return != saw_return {
        return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_x86_scalar_division_region(
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    require_return: bool,
    branches: &[TerminalScalarDivisionBranchEvidence],
    claimed: &mut std::collections::BTreeMap<usize, TerminalScalarStackMutation>,
    call_sites: &mut std::collections::BTreeMap<
        usize,
        omega_terminal_machine_code::TerminalInternalCallRelocation,
    >,
    evidence: &TerminalScalarStackEvidence,
    validated_calls: &mut Vec<TerminalObjectScalarCallStack>,
) -> Result<u32, TerminalObjectError> {
    if branches.is_empty() || start > end || end > bytes.len() {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: start,
        });
    }
    let mut cursor = start;
    let mut depth = 0_u32;
    let mut peak = 0_u32;
    for branch in branches {
        let branch_end = branch
            .branch_offset
            .checked_add(branch.branch_byte_count)
            .ok_or(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.branch_offset,
            })?;
        let join_end = branch
            .join_offset
            .checked_add(branch.join_byte_count)
            .ok_or(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.join_offset,
            })?;
        if cursor > branch.branch_offset
            || branch.branch_offset < start
            || branch.branch_offset >= branch_end
            || branch_end > branch.join_offset
            || join_end != branch.ordinary_arm_offset
            || branch.ordinary_arm_offset >= branch.merge_offset
            || branch.merge_offset > end
        {
            return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.branch_offset,
            });
        }
        let conditional = decode_exact_x86_instruction(
            machine,
            bytes,
            branch.branch_offset,
            branch.branch_byte_count,
        )?;
        let join = decode_exact_x86_instruction(
            machine,
            bytes,
            branch.join_offset,
            branch.join_byte_count,
        )?;
        if conditional.mnemonic() != iced_x86::Mnemonic::Jne
            || conditional.flow_control() != iced_x86::FlowControl::ConditionalBranch
            || usize::try_from(conditional.near_branch_target()).ok()
                != Some(branch.ordinary_arm_offset)
            || join.mnemonic() != iced_x86::Mnemonic::Jmp
            || join.flow_control() != iced_x86::FlowControl::UnconditionalBranch
            || usize::try_from(join.near_branch_target()).ok() != Some(branch.merge_offset)
        {
            return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.branch_offset,
            });
        }
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            cursor,
            branch.branch_offset,
            false,
            claimed,
            call_sites,
            evidence,
            validated_calls,
            &mut depth,
            &mut peak,
        )?;
        let branch_depth = depth;
        let mut special_depth = branch_depth;
        let mut special_peak = peak;
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            branch_end,
            branch.join_offset,
            false,
            claimed,
            call_sites,
            evidence,
            validated_calls,
            &mut special_depth,
            &mut special_peak,
        )?;
        let mut ordinary_depth = branch_depth;
        let mut ordinary_peak = peak;
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            branch.ordinary_arm_offset,
            branch.merge_offset,
            false,
            claimed,
            call_sites,
            evidence,
            validated_calls,
            &mut ordinary_depth,
            &mut ordinary_peak,
        )?;
        if special_depth != ordinary_depth {
            return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
        }
        depth = special_depth;
        peak = special_peak.max(ordinary_peak);
        cursor = branch.merge_offset;
    }
    replay_x86_scalar_linear_region(
        machine,
        bytes,
        cursor,
        end,
        require_return,
        claimed,
        call_sites,
        evidence,
        validated_calls,
        &mut depth,
        &mut peak,
    )?;
    if depth != 0 {
        return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok(peak)
}
