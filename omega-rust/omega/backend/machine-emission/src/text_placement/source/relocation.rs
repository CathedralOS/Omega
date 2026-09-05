use machine_code::{
    FunctionFragment, FunctionFragmentConditionalBranchPredicate, FunctionFragmentControlProvenance,
};
use selected_instructions::{MachineAlternativeFamily, MachineEncodedControlEffect};

use super::super::TextPlacementError;

pub(in crate::text_placement) fn prove_none(
    function: &FunctionFragment,
) -> Result<(), TextPlacementError> {
    for block in &function.blocks {
        for row in &block.instructions {
            match row.alternative.family {
                MachineAlternativeFamily::ConditionalBranchNonZero
                | MachineAlternativeFamily::ConditionalBranchU64LessThan
                | MachineAlternativeFamily::ConditionalBranchI64LessThan => {
                    let Some(branch) = row.branch.as_deref() else {
                        return Err(TextPlacementError::UnsupportedRelocationShape);
                    };
                    let FunctionFragmentControlProvenance::ConditionalBranch {
                        predicate,
                        when_taken,
                        when_fallthrough,
                    } = &row.control
                    else {
                        return Err(TextPlacementError::UnsupportedRelocationShape);
                    };
                    let expected_predicate = match row.alternative.family {
                        MachineAlternativeFamily::ConditionalBranchNonZero => {
                            FunctionFragmentConditionalBranchPredicate::NonZeroV1
                        }
                        MachineAlternativeFamily::ConditionalBranchU64LessThan => {
                            FunctionFragmentConditionalBranchPredicate::U64LessThanV1
                        }
                        MachineAlternativeFamily::ConditionalBranchI64LessThan => {
                            FunctionFragmentConditionalBranchPredicate::I64LessThanV1
                        }
                        _ => unreachable!("branch family matched above"),
                    };
                    if branch.predicate != expected_predicate
                        || *predicate != expected_predicate
                        || branch.source_block != block.block
                        || branch.when_taken_edge != when_taken.psi_edge
                        || branch.when_taken_block != when_taken.block
                        || branch.when_fallthrough_edge != when_fallthrough.psi_edge
                        || branch.when_fallthrough_block != when_fallthrough.block
                        || branch.decoded_effects.control
                            != MachineEncodedControlEffect::ConditionalRelativeBranchV1
                        || target_block_offset(function, branch.when_taken_block)
                            != Some(branch.when_taken_offset)
                        || target_block_offset(function, branch.when_fallthrough_block)
                            != Some(branch.when_fallthrough_offset)
                    {
                        return Err(TextPlacementError::UnsupportedRelocationShape);
                    }
                }
                MachineAlternativeFamily::ReturnI64 | MachineAlternativeFamily::ReturnUnit => {
                    if row.branch.is_some()
                        || !matches!(
                            row.control,
                            FunctionFragmentControlProvenance::Return { .. }
                        )
                    {
                        return Err(TextPlacementError::UnsupportedRelocationShape);
                    }
                }
                MachineAlternativeFamily::CallI64 => {
                    return Err(TextPlacementError::UnsupportedRelocationShape);
                }
                MachineAlternativeFamily::CompareI64Zero
                | MachineAlternativeFamily::CompareI64
                | MachineAlternativeFamily::MaterializeI64
                | MachineAlternativeFamily::CopyI64
                | MachineAlternativeFamily::ExactAddI64
                | MachineAlternativeFamily::ExactAddI64Immediate
                | MachineAlternativeFamily::ExactSubtractI64
                | MachineAlternativeFamily::ExactSubtractI64Immediate => {
                    if row.branch.is_some()
                        || row.control != FunctionFragmentControlProvenance::None
                    {
                        return Err(TextPlacementError::UnsupportedRelocationShape);
                    }
                }
            }
        }
    }
    Ok(())
}

fn target_block_offset(
    function: &FunctionFragment,
    target: selected_instructions::SelectedBlockId,
) -> Option<u64> {
    function
        .blocks
        .iter()
        .find(|block| block.block == target)
        .map(|block| block.offset)
}
