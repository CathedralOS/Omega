//! Exact scalar conditional-region topology and branch-edge replay.
//!
//! This module partitions retained conditional trees and division branches
//! into non-overlapping code regions, while independently decoding each x86-64
//! or AArch64 conditional edge. It does not replay region bodies or stack state.

use omega_target::Architecture;
use omega_terminal_machine_code::{
    TerminalScalarConditionalBranchEvidence, TerminalScalarConditionalCondition,
    TerminalScalarDivisionBranchEvidence,
};
use psi_core::MachineId;

use super::TerminalObjectError;

pub(super) fn division_branches_in_region(
    branches: &[TerminalScalarDivisionBranchEvidence],
    start: usize,
    end: usize,
) -> &[TerminalScalarDivisionBranchEvidence] {
    let first = branches.partition_point(|branch| branch.branch_offset < start);
    let last = branches.partition_point(|branch| branch.branch_offset < end);
    &branches[first..last]
}

pub(super) fn validate_division_branch_regions(
    machine: MachineId,
    branches: &[TerminalScalarDivisionBranchEvidence],
    regions: &[(usize, usize)],
) -> Result<(), TerminalObjectError> {
    for pair in branches.windows(2) {
        if pair[0].branch_offset >= pair[1].branch_offset {
            return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: pair[1].branch_offset,
            });
        }
    }
    if let Some(branch) = branches.iter().find(|branch| {
        !regions
            .iter()
            .any(|(start, end)| *start <= branch.branch_offset && branch.branch_offset < *end)
    }) {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: branch.branch_offset,
        });
    }
    Ok(())
}

pub(super) fn collect_conditional_tree_regions(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    decisions: &[TerminalScalarConditionalBranchEvidence],
    prefixes: &mut Vec<(usize, usize, TerminalScalarConditionalCondition)>,
    leaves: &mut Vec<(usize, usize)>,
) -> Result<(), TerminalObjectError> {
    let Some((root, descendants)) = decisions.split_first() else {
        if start >= end {
            return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: start,
            });
        }
        leaves.push((start, end));
        return Ok(());
    };
    let branch_end = root
        .branch_offset
        .checked_add(root.branch_byte_count)
        .ok_or(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: root.branch_offset,
        })?;
    if root.branch_offset < start
        || branch_end >= root.false_arm_offset
        || root.false_arm_offset >= end
    {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: root.branch_offset,
        });
    }
    validate_scalar_conditional_branch(
        architecture,
        root.condition,
        machine,
        bytes,
        root.branch_offset,
        root.branch_byte_count,
        root.false_arm_offset,
    )?;
    prefixes.push((start, root.branch_offset, root.condition));
    let true_decision_count =
        descendants.partition_point(|branch| branch.branch_offset < root.false_arm_offset);
    let (true_decisions, false_decisions) = descendants.split_at(true_decision_count);
    collect_conditional_tree_regions(
        architecture,
        machine,
        bytes,
        branch_end,
        root.false_arm_offset,
        true_decisions,
        prefixes,
        leaves,
    )?;
    collect_conditional_tree_regions(
        architecture,
        machine,
        bytes,
        root.false_arm_offset,
        end,
        false_decisions,
        prefixes,
        leaves,
    )
}

fn validate_scalar_conditional_branch(
    architecture: Architecture,
    condition: TerminalScalarConditionalCondition,
    machine: MachineId,
    bytes: &[u8],
    branch_offset: usize,
    branch_byte_count: usize,
    false_arm_offset: usize,
) -> Result<(), TerminalObjectError> {
    let invalid = || TerminalObjectError::InvalidScalarConditionalEvidence {
        machine,
        offset: branch_offset,
    };
    let target = match architecture {
        Architecture::X86_64 => {
            if branch_byte_count != 6
                || bytes.get(branch_offset..branch_offset.saturating_add(2)) != Some(&[0x0f, 0x84])
            {
                return Err(invalid());
            }
            let displacement = bytes
                .get(branch_offset + 2..branch_offset + 6)
                .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
                .map(i32::from_le_bytes)
                .ok_or_else(invalid)?;
            i64::try_from(branch_offset + branch_byte_count)
                .ok()
                .and_then(|base| base.checked_add(i64::from(displacement)))
                .and_then(|target| usize::try_from(target).ok())
                .ok_or_else(invalid)?
        }
        Architecture::Aarch64 => {
            if branch_byte_count != 4 || !branch_offset.is_multiple_of(4) {
                return Err(invalid());
            }
            let encoded = bytes
                .get(branch_offset..branch_offset + 4)
                .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
                .map(u32::from_le_bytes)
                .ok_or_else(invalid)?;
            match condition {
                TerminalScalarConditionalCondition::Parameter
                    if encoded & 0xff00_0000 != 0x3400_0000 =>
                {
                    return Err(invalid());
                }
                TerminalScalarConditionalCondition::Expression
                    if encoded & 0xff00_001f != 0x5400_0000 =>
                {
                    return Err(invalid());
                }
                _ => {}
            }
            let immediate = ((encoded >> 5) & 0x7ffff) as i32;
            let displacement = (immediate << 13 >> 13) * 4;
            i64::try_from(branch_offset)
                .ok()
                .and_then(|base| base.checked_add(i64::from(displacement)))
                .and_then(|target| usize::try_from(target).ok())
                .ok_or_else(invalid)?
        }
    };
    if target != false_arm_offset {
        return Err(invalid());
    }
    Ok(())
}
