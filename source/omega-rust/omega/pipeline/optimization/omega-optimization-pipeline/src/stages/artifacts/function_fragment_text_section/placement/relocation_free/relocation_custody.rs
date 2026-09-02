use omega_machine_code::{FunctionFragment, FunctionFragmentControlProvenance};
use omega_selected_instructions::{MachineAlternativeFamily, MachineEncodedControlEffect};

use super::super::super::RelocationFreeTextSectionPlacementError;

pub(super) fn prove_none(
    function: &FunctionFragment,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    for block in &function.blocks {
        for row in &block.instructions {
            match row.alternative.family {
                MachineAlternativeFamily::ConditionalBranchNonZero => {
                    let Some(branch) = row.branch.as_deref() else {
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    };
                    let FunctionFragmentControlProvenance::ConditionalBranch {
                        when_nonzero,
                        when_zero,
                    } = &row.control
                    else {
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    };
                    if branch.source_block != block.block
                        || branch.when_nonzero_edge != when_nonzero.psi_edge
                        || branch.when_nonzero_block != when_nonzero.block
                        || branch.when_zero_edge != when_zero.psi_edge
                        || branch.when_zero_block != when_zero.block
                        || branch.decoded_effects.control
                            != MachineEncodedControlEffect::ConditionalRelativeBranchV1
                        || target_block_offset(function, branch.when_nonzero_block)
                            != Some(branch.when_nonzero_offset)
                        || target_block_offset(function, branch.when_zero_block)
                            != Some(branch.when_zero_offset)
                    {
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    }
                }
                MachineAlternativeFamily::ReturnI64 | MachineAlternativeFamily::ReturnUnit => {
                    if row.branch.is_some()
                        || !matches!(
                            row.control,
                            FunctionFragmentControlProvenance::Return { .. }
                        )
                    {
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    }
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
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn target_block_offset(
    function: &FunctionFragment,
    target: omega_selected_instructions::SelectedBlockId,
) -> Option<u64> {
    function
        .blocks
        .iter()
        .find(|block| block.block == target)
        .map(|block| block.offset)
}
