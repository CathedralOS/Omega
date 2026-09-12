use machine_code::{
    FunctionFragment, FunctionFragmentBranchEvidence as Branch,
    FunctionFragmentConditionalBranchPredicate, FunctionFragmentControlProvenance,
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
                    let Some(Branch::Conditional(branch)) = row.branch.as_deref() else {
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
                MachineAlternativeFamily::Jump => {
                    let (
                        Some(Branch::Jump(branch)),
                        FunctionFragmentControlProvenance::Jump { successor },
                    ) = (row.branch.as_deref(), &row.control)
                    else {
                        return Err(TextPlacementError::UnsupportedRelocationShape);
                    };
                    if branch.source_block != block.block
                        || branch.target_edge != successor.psi_edge
                        || branch.target_block != successor.block
                        || branch.decoded_effects.control
                            != MachineEncodedControlEffect::UnconditionalRelativeBranchV1
                        || target_block_offset(function, branch.target_block)
                            != Some(branch.target_offset)
                    {
                        return Err(TextPlacementError::UnsupportedRelocationShape);
                    }
                }
                MachineAlternativeFamily::HostedExitProcessI32 => {
                    if row.branch.is_some()
                        || !matches!(
                            row.control,
                            FunctionFragmentControlProvenance::HostedExitProcess { .. }
                        )
                    {
                        return Err(TextPlacementError::UnsupportedRelocationShape);
                    }
                }
                MachineAlternativeFamily::ReturnScalar
                | MachineAlternativeFamily::ReturnUnit
                | MachineAlternativeFamily::ReturnAggregate => {
                    if row.branch.is_some()
                        || !matches!(
                            row.control,
                            FunctionFragmentControlProvenance::Return { .. }
                        )
                    {
                        return Err(TextPlacementError::UnsupportedRelocationShape);
                    }
                }
                MachineAlternativeFamily::CallScalar
                | MachineAlternativeFamily::CallUnit
                | MachineAlternativeFamily::CallAggregate => {
                    return Err(TextPlacementError::UnsupportedRelocationShape);
                }
                MachineAlternativeFamily::CompareI64Zero
                | MachineAlternativeFamily::HostedWriteByteI32
                | MachineAlternativeFamily::HostedReadByte
                | MachineAlternativeFamily::Load8Indexed
                | MachineAlternativeFamily::Load64
                | MachineAlternativeFamily::Load8
                | MachineAlternativeFamily::Load16
                | MachineAlternativeFamily::Load32
                | MachineAlternativeFamily::LoadPacked3
                | MachineAlternativeFamily::LoadPacked5
                | MachineAlternativeFamily::LoadPacked6
                | MachineAlternativeFamily::LoadPacked7
                | MachineAlternativeFamily::StorePacked
                | MachineAlternativeFamily::Store
                | MachineAlternativeFamily::AddressOffset
                | MachineAlternativeFamily::Store64
                | MachineAlternativeFamily::FrameAddress
                | MachineAlternativeFamily::CompareI64
                | MachineAlternativeFamily::MaterializeI64
                | MachineAlternativeFamily::CopyI64
                | MachineAlternativeFamily::BitwiseAndI64
                | MachineAlternativeFamily::BitwiseXorI64
                | MachineAlternativeFamily::Float32ToBits
                | MachineAlternativeFamily::Float64ToBits
                | MachineAlternativeFamily::BitsToFloat32
                | MachineAlternativeFamily::BitsToFloat64
                | MachineAlternativeFamily::ZeroExtendU8
                | MachineAlternativeFamily::ZeroExtendU32
                | MachineAlternativeFamily::ZeroExtendU16
                | MachineAlternativeFamily::SignExtendI8
                | MachineAlternativeFamily::SignExtendI16
                | MachineAlternativeFamily::SignExtendI32
                | MachineAlternativeFamily::MaterializeBooleanEqual
                | MachineAlternativeFamily::MaterializeBooleanU64LessThan
                | MachineAlternativeFamily::MaterializeBooleanI64LessThan
                | MachineAlternativeFamily::MaterializeBooleanU64LessOrEqual
                | MachineAlternativeFamily::MaterializeBooleanI64LessOrEqual
                | MachineAlternativeFamily::ByteViewAddress
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
