//! Dense offset reconstruction with separate production encoding and replay validation.

use std::collections::BTreeMap;

use omega_isa_x86_64::{
    encode_x86_64_selected_nonzero_branch_form, encode_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
};
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId};

use crate::ResolvedSelectedFunctionLayout;

use super::super::error::OptimizedX86BranchRelaxationError;
use super::work::checked_delta;

pub(super) fn reflow_production_functions(
    functions: &mut [ResolvedSelectedFunctionLayout],
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    for function in functions {
        let offsets = assign_dense_offsets(function)?;
        for block in &mut function.blocks {
            for row in &mut block.instructions {
                let Some(branch) = row.branch.as_mut() else {
                    continue;
                };
                rewrite_branch_offsets(
                    branch,
                    row.offset,
                    row.bytes.len(),
                    &offsets,
                    row.instruction,
                )?;
                let encoded = if row.bytes.len() == 2 {
                    encode_x86_64_selected_short_nonzero_branch_form(
                        physical,
                        row.alternative,
                        branch.byte_displacement,
                    )
                } else {
                    encode_x86_64_selected_nonzero_branch_form(
                        physical,
                        row.alternative,
                        branch.byte_displacement,
                    )
                }
                .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
                if encoded.footprint().encoded != branch.decoded_effects {
                    return Err(OptimizedX86BranchRelaxationError::BranchEffectsMismatch(
                        row.instruction,
                    ));
                }
                row.bytes = encoded.bytes().to_vec();
            }
        }
    }
    Ok(())
}

pub(super) fn reflow_replay_functions(
    functions: &mut [ResolvedSelectedFunctionLayout],
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    for function in functions {
        let mut next = 0_u64;
        let mut offsets = BTreeMap::new();
        for block in &mut function.blocks {
            block.offset = next;
            offsets.insert(block.block, next);
            let mut local = next;
            for row in &mut block.instructions {
                row.offset = local;
                local = local
                    .checked_add(
                        u64::try_from(row.bytes.len())
                            .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                    )
                    .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
            }
            block.byte_count = local - next;
            next = local;
        }
        function.byte_count = next;
        for block in &mut function.blocks {
            for row in &mut block.instructions {
                let Some(branch) = row.branch.as_mut() else {
                    continue;
                };
                let nonzero = offsets.get(&branch.when_nonzero_block).copied().ok_or(
                    OptimizedX86BranchRelaxationError::MissingTargetBlock(
                        branch.when_nonzero_block,
                    ),
                )?;
                let zero = offsets.get(&branch.when_zero_block).copied().ok_or(
                    OptimizedX86BranchRelaxationError::MissingTargetBlock(branch.when_zero_block),
                )?;
                let end = row
                    .offset
                    .checked_add(
                        u64::try_from(row.bytes.len())
                            .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                    )
                    .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
                if zero != end {
                    return Err(
                        OptimizedX86BranchRelaxationError::BranchFallthroughMismatch(
                            row.instruction,
                        ),
                    );
                }
                branch.when_nonzero_offset = nonzero;
                branch.when_zero_offset = zero;
                branch.byte_displacement = checked_delta(nonzero, end)?;
                if row.bytes.len() == 2 {
                    let bytes = [0x75, branch.byte_displacement as i8 as u8];
                    let decoded = validate_x86_64_selected_short_nonzero_branch_form(
                        physical,
                        row.alternative,
                        branch.byte_displacement,
                        &bytes,
                    )
                    .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
                    if decoded.footprint().encoded != branch.decoded_effects {
                        return Err(OptimizedX86BranchRelaxationError::BranchEffectsMismatch(
                            row.instruction,
                        ));
                    }
                    row.bytes = bytes.to_vec();
                } else {
                    let mut bytes = vec![0x0f, 0x85];
                    let displacement = i32::try_from(branch.byte_displacement).map_err(|_| {
                        OptimizedX86BranchRelaxationError::MalformedBranch(row.instruction)
                    })?;
                    bytes.extend_from_slice(&displacement.to_le_bytes());
                    let decoded = validate_x86_64_selected_nonzero_branch_form(
                        physical,
                        row.alternative,
                        branch.byte_displacement,
                        &bytes,
                    )
                    .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
                    if decoded.footprint().encoded != branch.decoded_effects {
                        return Err(OptimizedX86BranchRelaxationError::BranchEffectsMismatch(
                            row.instruction,
                        ));
                    }
                    row.bytes = bytes;
                }
            }
        }
    }
    Ok(())
}

fn assign_dense_offsets(
    function: &mut ResolvedSelectedFunctionLayout,
) -> Result<BTreeMap<SelectedBlockId, u64>, OptimizedX86BranchRelaxationError> {
    let mut offsets = BTreeMap::new();
    let mut offset = 0_u64;
    for block in &mut function.blocks {
        block.offset = offset;
        offsets.insert(block.block, offset);
        let start = offset;
        for row in &mut block.instructions {
            row.offset = offset;
            offset = offset
                .checked_add(
                    u64::try_from(row.bytes.len())
                        .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                )
                .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
        }
        block.byte_count = offset - start;
    }
    function.byte_count = offset;
    Ok(offsets)
}

fn rewrite_branch_offsets(
    branch: &mut crate::ResolvedConditionalBranchEvidence,
    instruction_offset: u64,
    instruction_size: usize,
    offsets: &BTreeMap<SelectedBlockId, u64>,
    instruction: SelectedInstructionId,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    let nonzero = *offsets.get(&branch.when_nonzero_block).ok_or(
        OptimizedX86BranchRelaxationError::MissingTargetBlock(branch.when_nonzero_block),
    )?;
    let zero = *offsets.get(&branch.when_zero_block).ok_or(
        OptimizedX86BranchRelaxationError::MissingTargetBlock(branch.when_zero_block),
    )?;
    let end = instruction_offset
        .checked_add(
            u64::try_from(instruction_size)
                .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
        )
        .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
    if zero != end {
        return Err(OptimizedX86BranchRelaxationError::BranchFallthroughMismatch(instruction));
    }
    branch.when_nonzero_offset = nonzero;
    branch.when_zero_offset = zero;
    branch.byte_displacement = checked_delta(nonzero, end)?;
    Ok(())
}
