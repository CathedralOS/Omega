//! Exact conditional scalar-stack replay.
//!
//! This module validates conditional-tree regions, terminal and crash leaves,
//! optional division subregions, and balanced stack/call evidence on both native
//! targets. It does not infer control flow, choose ABI placements, or emit
//! instructions.

use crate::object_artifact::replay::x86_register_info_writes_stack_pointer;
use crate::order::strictly_increasing;

use machine_code::{
    ScalarConditionalBranchEvidence, ScalarConditionalCondition, ScalarControlAffineCleanupRecord,
    ScalarDivisionBranchEvidence, ScalarStackEvidence,
};
use semantic_vocabulary::MachineId;
use target::Architecture;

use super::super::unit::stack::aarch64_stack_adjustment_at;
use super::conditional_regions::{
    collect_conditional_tree_regions, division_branches_in_region, validate_division_branch_regions,
};
use super::division_stack::replay_x86_scalar_division_region;
use super::invalid_conditional_evidence as rejected;
use super::stack_mutation::{
    ScalarReplay, aarch64_control_flow_instruction, aarch64_unsupported_sp_write,
    replay_scalar_mutation, validate_aarch64_scalar_mutation, validate_x86_scalar_mutation,
};
use crate::{ObjectError, ObjectScalarCallStack, ObjectScalarStack};

#[allow(clippy::too_many_arguments)]
fn replay_scalar_conditional_region_with_divisions(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    require_return: bool,
    division_branches: &[ScalarDivisionBranchEvidence],
    allow_calls: bool,
    walk: &mut ScalarReplay<'_>,
) -> Result<u32, ObjectError> {
    if division_branches.is_empty() {
        return replay_scalar_conditional_region(
            architecture,
            machine,
            bytes,
            start,
            end,
            require_return,
            walk,
            allow_calls,
            None,
        );
    }
    if architecture != Architecture::X86_64 || !allow_calls {
        return Err(rejected(machine, start));
    }
    replay_x86_scalar_division_region(
        machine,
        bytes,
        start,
        end,
        require_return,
        division_branches,
        walk,
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
    division_branches: &[ScalarDivisionBranchEvidence],
    walk: &mut ScalarReplay<'_>,
) -> Result<u32, ObjectError> {
    if !crash {
        return replay_scalar_conditional_region_with_divisions(
            architecture,
            machine,
            bytes,
            start,
            end,
            true,
            division_branches,
            true,
            walk,
        );
    }
    if !division_branches.is_empty() {
        return Err(rejected(machine, start));
    }
    let crash_bytes: &[u8] = match architecture {
        Architecture::X86_64 => &[0x0f, 0x0b],
        Architecture::Aarch64 => &[0x00, 0x00, 0x20, 0xd4],
    };
    let crash_offset = end
        .checked_sub(crash_bytes.len())
        .ok_or(rejected(machine, start))?;
    if crash_offset < start || bytes.get(crash_offset..end) != Some(crash_bytes) {
        return Err(rejected(machine, crash_offset));
    }
    replay_scalar_conditional_region(
        architecture,
        machine,
        bytes,
        start,
        crash_offset,
        false,
        walk,
        true,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_conditional_tree_scalar_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    calls: &[machine_code::InternalCallRelocation],
    evidence: &ScalarStackEvidence,
    decisions: &[ScalarConditionalBranchEvidence],
    crash_leaves: &[bool],
    division_branches: &[ScalarDivisionBranchEvidence],
    cleanups: &[ScalarControlAffineCleanupRecord],
) -> Result<(ObjectScalarStack, Vec<ObjectScalarCallStack>), ObjectError> {
    if decisions.is_empty()
        || crash_leaves.len() != decisions.len() + 1
        || !cleanups.is_empty() && cleanups.len() != crash_leaves.len()
        || !cleanups.is_empty() && crash_leaves.iter().any(|crash| *crash)
        || !cleanups.is_empty() && !division_branches.is_empty()
        || evidence.cleanup_preservation.is_some()
        || !strictly_increasing(&decisions, |row| row.branch_offset)
        || !strictly_increasing(&evidence.mutations, |row| row.offset)
    {
        return Err(rejected(
            machine,
            decisions.first().map_or(0, |branch| branch.branch_offset),
        ));
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
        return Err(rejected(machine, decisions[0].branch_offset));
    }
    let mut division_regions = prefixes
        .iter()
        .map(|(start, end, _)| (*start, *end))
        .collect::<Vec<_>>();
    division_regions.extend(leaves.iter().copied());
    validate_division_branch_regions(machine, division_branches, &division_regions)?;

    let mut walk = ScalarReplay::new(architecture, machine, bytes, evidence, calls)?;
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
            condition == ScalarConditionalCondition::Expression,
            &mut walk,
        )?;
        if condition == ScalarConditionalCondition::Parameter && prefix_peak != 0 {
            return Err(rejected(machine, end));
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
                &mut walk,
                true,
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
                &mut walk,
            )?
        };
        peak = peak.max(leaf_peak);
    }
    walk.finish(peak, None)
}

pub(crate) fn replay_scalar_conditional_region(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    require_return: bool,
    walk: &mut ScalarReplay<'_>,
    allow_calls: bool,
    scalar_affine_cleanup: Option<&machine_code::UnitAffineCleanupRecord>,
) -> Result<u32, ObjectError> {
    if start > end || end > bytes.len() {
        return Err(rejected(machine, start));
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
                    return Err(ObjectError::InvalidScalarInstructionEncoding { machine, offset });
                }
                if instruction.mnemonic() == iced_x86::Mnemonic::Ret {
                    if !require_return
                        || offset.checked_add(instruction.len()) != Some(end)
                        || saw_return
                    {
                        return Err(ObjectError::NonLinearScalarControlFlow { machine, offset });
                    }
                    saw_return = true;
                    continue;
                }
                if instruction.mnemonic() == iced_x86::Mnemonic::Call {
                    walk.call(offset, allow_calls, scalar_affine_cleanup, depth, &mut peak)?;
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
                ) && instruction.op0_register()
                    == iced_x86::Register::RSP
                    || instruction.mnemonic() == iced_x86::Mnemonic::Lea
                        && instruction.op0_register() == iced_x86::Register::RSP;
                if stack_mutation {
                    let mutation = walk.claim(offset)?;
                    validate_x86_scalar_mutation(machine, bytes, &instruction, mutation)?;
                    replay_scalar_mutation(machine, offset, mutation.kind, &mut depth, &mut peak)?;
                    continue;
                }
                let info = info_factory.info(&instruction);
                if x86_register_info_writes_stack_pointer(info) {
                    return Err(ObjectError::UnsupportedScalarStackMutation { machine, offset });
                }
            }
        }
        Architecture::Aarch64 => {
            if !start.is_multiple_of(4) || !end.is_multiple_of(4) {
                return Err(rejected(machine, start));
            }
            for offset in (start..end).step_by(4) {
                let encoded = u32::from_le_bytes(
                    bytes[offset..offset + 4]
                        .try_into()
                        .expect("four-byte AArch64 word"),
                );
                if encoded == 0xd65f_03c0 {
                    if !require_return || offset + 4 != end || saw_return {
                        return Err(ObjectError::NonLinearScalarControlFlow { machine, offset });
                    }
                    saw_return = true;
                    continue;
                }
                if encoded == 0x9400_0000 {
                    walk.call(offset, allow_calls, scalar_affine_cleanup, depth, &mut peak)?;
                    continue;
                }
                if aarch64_control_flow_instruction(encoded) {
                    return Err(ObjectError::NonLinearScalarControlFlow { machine, offset });
                }
                if aarch64_stack_adjustment_at(bytes, offset) {
                    let mutation = walk.claim(offset)?;
                    validate_aarch64_scalar_mutation(machine, encoded, mutation)?;
                    replay_scalar_mutation(machine, offset, mutation.kind, &mut depth, &mut peak)?;
                } else if aarch64_unsupported_sp_write(encoded) {
                    return Err(ObjectError::UnsupportedScalarStackMutation { machine, offset });
                }
            }
        }
    }
    if require_return != saw_return || depth != 0 {
        return Err(ObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok(peak)
}
