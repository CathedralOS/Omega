//! Exact x86-64 scalar division-stack replay.
//!
//! This module validates branch geometry and exact instructions while replaying
//! balanced scalar stack mutations and typed internal calls across ordinary and
//! exceptional division regions. It does not infer control-flow evidence,
//! select calling conventions, or emit instructions.

use crate::object_artifact::replay::x86_register_info_writes_stack_pointer;
use crate::order::strictly_increasing;

use machine_code::{ScalarDivisionBranchEvidence, ScalarStackEvidence};
use semantic_vocabulary::MachineId;
use target::Architecture;

use super::invalid_conditional_evidence as rejected;
use super::stack_mutation::{ScalarReplay, replay_scalar_mutation, validate_x86_scalar_mutation};
use crate::{ObjectError, ObjectScalarCallStack, ObjectScalarStack};

pub(crate) fn validate_linear_scalar_division_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    calls: &[machine_code::InternalCallRelocation],
    evidence: &ScalarStackEvidence,
    branches: &[ScalarDivisionBranchEvidence],
) -> Result<(ObjectScalarStack, Vec<ObjectScalarCallStack>), ObjectError> {
    if architecture != Architecture::X86_64 || branches.is_empty() {
        return Err(ObjectError::InvalidScalarConditionalEvidence { machine, offset: 0 });
    }
    if !strictly_increasing(&evidence.mutations, |row| row.offset) {
        return Err(ObjectError::NonCanonicalScalarStackMutationOrder(machine));
    }
    let mut walk = ScalarReplay::new(architecture, machine, bytes, evidence, calls)?;
    let (depth, peak) = replay_x86_scalar_division_branches(
        machine,
        bytes,
        0,
        bytes.len(),
        true,
        branches,
        &mut walk,
    )?;
    let late_error = (depth != 0).then_some(ObjectError::MissingBalancedScalarReturn(machine));
    walk.finish(peak, late_error)
}

pub(crate) fn decode_exact_x86_instruction(
    machine: MachineId,
    bytes: &[u8],
    offset: usize,
    byte_count: usize,
) -> Result<iced_x86::Instruction, ObjectError> {
    let end = offset
        .checked_add(byte_count)
        .filter(|end| *end <= bytes.len())
        .ok_or(ObjectError::InvalidScalarConditionalEvidence { machine, offset })?;
    let mut decoder = iced_x86::Decoder::with_ip(
        64,
        &bytes[offset..end],
        offset as u64,
        iced_x86::DecoderOptions::NONE,
    );
    let instruction = decoder.decode();
    if instruction.is_invalid() || instruction.len() != byte_count || decoder.can_decode() {
        return Err(ObjectError::InvalidScalarConditionalEvidence { machine, offset });
    }
    Ok(instruction)
}

/// Validates one branch's decoded instruction pair: the conditional must be
/// `Jne` targeting the ordinary arm, the join must be `Jmp` targeting the
/// merge.
fn validate_scalar_division_branch_instructions(
    machine: MachineId,
    bytes: &[u8],
    branch: &ScalarDivisionBranchEvidence,
) -> Result<(), ObjectError> {
    let conditional = decode_exact_x86_instruction(
        machine,
        bytes,
        branch.branch_offset,
        branch.branch_byte_count,
    )?;
    let join =
        decode_exact_x86_instruction(machine, bytes, branch.join_offset, branch.join_byte_count)?;
    if conditional.mnemonic() != iced_x86::Mnemonic::Jne
        || conditional.flow_control() != iced_x86::FlowControl::ConditionalBranch
        || usize::try_from(conditional.near_branch_target()).ok()
            != Some(branch.ordinary_arm_offset)
        || join.mnemonic() != iced_x86::Mnemonic::Jmp
        || join.flow_control() != iced_x86::FlowControl::UnconditionalBranch
        || usize::try_from(join.near_branch_target()).ok() != Some(branch.merge_offset)
    {
        return Err(rejected(machine, branch.branch_offset));
    }
    Ok(())
}

fn replay_x86_scalar_linear_region(
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    require_return: bool,
    walk: &mut ScalarReplay<'_>,
    depth: &mut u32,
    peak: &mut u32,
) -> Result<(), ObjectError> {
    if start > end || end > bytes.len() {
        return Err(rejected(machine, start));
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
            return Err(ObjectError::InvalidScalarInstructionEncoding { machine, offset });
        }
        if instruction.mnemonic() == iced_x86::Mnemonic::Ret {
            if !require_return || offset.checked_add(instruction.len()) != Some(end) || saw_return {
                return Err(ObjectError::NonLinearScalarControlFlow { machine, offset });
            }
            saw_return = true;
            continue;
        }
        if instruction.mnemonic() == iced_x86::Mnemonic::Call {
            walk.call(offset, true, None, *depth, peak)?;
            continue;
        }
        if instruction.flow_control() != iced_x86::FlowControl::Next {
            return Err(ObjectError::NonLinearScalarControlFlow { machine, offset });
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
            let mutation = walk.claim(offset)?;
            validate_x86_scalar_mutation(machine, bytes, &instruction, mutation)?;
            replay_scalar_mutation(machine, offset, mutation.kind, depth, peak)?;
            continue;
        }
        let info = info_factory.info(&instruction);
        if x86_register_info_writes_stack_pointer(info) {
            return Err(ObjectError::UnsupportedScalarStackMutation { machine, offset });
        }
    }
    if require_return != saw_return {
        return Err(ObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok(())
}

/// Replays one `start..end` division region: every branch's pre-branch gap, its
/// exceptional arm, and its ordinary arm, followed by the region tail. Returns
/// the depth and peak left behind so callers can apply their own depth policy.
#[allow(clippy::too_many_arguments)]
fn replay_x86_scalar_division_branches(
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    require_return: bool,
    branches: &[ScalarDivisionBranchEvidence],
    walk: &mut ScalarReplay<'_>,
) -> Result<(u32, u32), ObjectError> {
    let mut cursor = start;
    let mut depth = 0_u32;
    let mut peak = 0_u32;
    for branch in branches {
        let branch_end = branch
            .branch_offset
            .checked_add(branch.branch_byte_count)
            .ok_or(rejected(machine, branch.branch_offset))?;
        let join_end = branch
            .join_offset
            .checked_add(branch.join_byte_count)
            .ok_or(rejected(machine, branch.join_offset))?;
        if cursor > branch.branch_offset
            || branch.branch_offset < start
            || branch.branch_offset >= branch_end
            || branch_end > branch.join_offset
            || join_end != branch.ordinary_arm_offset
            || branch.ordinary_arm_offset >= branch.merge_offset
            || branch.merge_offset > end
        {
            return Err(rejected(machine, branch.branch_offset));
        }
        validate_scalar_division_branch_instructions(machine, bytes, branch)?;
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            cursor,
            branch.branch_offset,
            false,
            walk,
            &mut depth,
            &mut peak,
        )?;
        let mut special_depth = depth;
        let mut special_peak = peak;
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            branch_end,
            branch.join_offset,
            false,
            walk,
            &mut special_depth,
            &mut special_peak,
        )?;
        let mut ordinary_depth = depth;
        let mut ordinary_peak = peak;
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            branch.ordinary_arm_offset,
            branch.merge_offset,
            false,
            walk,
            &mut ordinary_depth,
            &mut ordinary_peak,
        )?;
        if special_depth != ordinary_depth {
            return Err(ObjectError::MissingBalancedScalarReturn(machine));
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
        walk,
        &mut depth,
        &mut peak,
    )?;
    Ok((depth, peak))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn replay_x86_scalar_division_region(
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    require_return: bool,
    branches: &[ScalarDivisionBranchEvidence],
    walk: &mut ScalarReplay<'_>,
) -> Result<u32, ObjectError> {
    if branches.is_empty() || start > end || end > bytes.len() {
        return Err(rejected(machine, start));
    }
    let (depth, peak) = replay_x86_scalar_division_branches(
        machine,
        bytes,
        start,
        end,
        require_return,
        branches,
        walk,
    )?;
    if depth != 0 {
        return Err(ObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok(peak)
}
