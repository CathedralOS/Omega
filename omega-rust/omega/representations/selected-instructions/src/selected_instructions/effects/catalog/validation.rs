use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};

#[cfg(test)]
mod tests;

use super::{
    MachineAlternativeApplicability, MachineBarrier, MachineEffectCatalogValidationError,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineSemanticKind, MachineSizeKnowledge,
};
pub(super) fn validate_declaration(
    constraint: &RegisterInstructionConstraint,
    declaration: &MachineEffectDeclaration,
) -> Result<(), MachineEffectCatalogValidationError> {
    let semantic = declaration.semantic;
    let expected_barrier = if matches!(
        semantic,
        MachineSemanticKind::HostedReadByte
            | MachineSemanticKind::HostedWriteByteI32
            | MachineSemanticKind::HostedExitProcessI32
    ) {
        MachineBarrier::ExternalEffect
    } else if matches!(
        semantic,
        MachineSemanticKind::ConditionalBranchNonZero
            | MachineSemanticKind::Jump
            | MachineSemanticKind::ConditionalBranchU64LessThan
            | MachineSemanticKind::ConditionalBranchI64LessThan
            | MachineSemanticKind::ReturnI64
            | MachineSemanticKind::ReturnAggregate
            | MachineSemanticKind::ReturnUnit
    ) {
        MachineBarrier::ControlFlow
    } else if matches!(
        semantic,
        MachineSemanticKind::CallI64
            | MachineSemanticKind::CallUnit
            | MachineSemanticKind::CallAggregate
    ) {
        MachineBarrier::Call
    } else {
        MachineBarrier::None
    };
    if declaration.barrier != expected_barrier {
        return Err(MachineEffectCatalogValidationError::BarrierMismatch(
            semantic,
        ));
    }
    match (semantic, declaration.call) {
        (
            MachineSemanticKind::CallI64
            | MachineSemanticKind::CallUnit
            | MachineSemanticKind::CallAggregate,
            crate::MachineCallEffect::DirectInternalNormalReturnV1 {
                pre_call_stack_alignment,
            },
        ) if pre_call_stack_alignment.is_power_of_two() => {}
        (
            MachineSemanticKind::CallI64
            | MachineSemanticKind::CallUnit
            | MachineSemanticKind::CallAggregate,
            _,
        ) => {
            return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                semantic,
            ));
        }
        (_, crate::MachineCallEffect::NoneV1) => {}
        _ => {
            return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                semantic,
            ));
        }
    }
    if declaration.alternatives.is_empty() {
        return Err(MachineEffectCatalogValidationError::EmptyAlternatives(
            semantic,
        ));
    }
    if declaration
        .alternatives
        .windows(2)
        .any(|pair| pair[0].key >= pair[1].key)
    {
        return Err(MachineEffectCatalogValidationError::NonCanonicalAlternatives(semantic));
    }
    let expected_family = semantic.into();
    for alternative in &declaration.alternatives {
        if alternative.key.family != expected_family {
            return Err(MachineEffectCatalogValidationError::AlternativeFamilyMismatch(semantic));
        }
        validate_applicability(constraint, alternative.applicability).map_err(|()| {
            MachineEffectCatalogValidationError::InvalidAlternativeApplicability(semantic)
        })?;
        validate_encoded_effects(constraint, declaration, &alternative.encoded)
            .map_err(|()| MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic))?;
        match alternative.size {
            MachineSizeKnowledge::ExactBytes(0)
            | MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 0, ..
            } => {
                return Err(MachineEffectCatalogValidationError::InvalidSizeKnowledge(
                    semantic,
                ));
            }
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes,
                maximum_bytes: Some(maximum),
            } if maximum < minimum_bytes => {
                return Err(MachineEffectCatalogValidationError::InvalidSizeKnowledge(
                    semantic,
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_encoded_effects(
    constraint: &RegisterInstructionConstraint,
    declaration: &MachineEffectDeclaration,
    encoded: &MachineEncodedEffects,
) -> Result<(), ()> {
    let canonical = |values: &[u16]| values.windows(2).all(|pair| pair[0] < pair[1]);
    if !canonical(&encoded.external_operand_reads)
        || !canonical(&encoded.external_operand_writes)
        || encoded
            .implicit_unit_uses
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || encoded
            .implicit_unit_defs
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || encoded
            .implicit_unit_clobbers
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(());
    }
    for operand in &encoded.external_operand_reads {
        let row = constraint
            .operands
            .iter()
            .find(|row| row.operand == *operand)
            .ok_or(())?;
        if !matches!(
            row.access,
            RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
        ) {
            return Err(());
        }
    }
    for operand in &encoded.external_operand_writes {
        let row = constraint
            .operands
            .iter()
            .find(|row| row.operand == *operand)
            .ok_or(())?;
        if !matches!(
            row.access,
            RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
        ) {
            return Err(());
        }
    }
    if !encoded
        .implicit_unit_uses
        .iter()
        .all(|unit| constraint.implicit_uses.contains(unit))
        || !encoded
            .implicit_unit_defs
            .iter()
            .all(|unit| constraint.implicit_defs.contains(unit))
        || !encoded
            .implicit_unit_clobbers
            .iter()
            .all(|unit| constraint.clobbers.contains(unit))
    {
        return Err(());
    }
    if declaration.semantic == MachineSemanticKind::HostedExitProcessI32
        && (declaration.memory != crate::MachineMemoryEffect::NoneV1
            || declaration.trap != crate::MachineTrapBehavior::HostedExitReturnedV1
            || encoded.memory != MachineEncodedMemoryEffect::NoneV1
            || encoded.trap != MachineEncodedTrapBehavior::HostedExitReturnedV1
            || encoded.stack != MachineEncodedStackEffect::UnchangedV1
            || encoded.control != MachineEncodedControlEffect::HostedExitOrTrapV1
            || declaration.call != crate::MachineCallEffect::NoneV1)
    {
        return Err(());
    }
    if declaration.semantic == MachineSemanticKind::HostedReadByte
        && (declaration.memory != crate::MachineMemoryEffect::HostedReadByteV1
            || declaration.trap != crate::MachineTrapBehavior::HostedReadFailureV1
            || !matches!(
                encoded.memory,
                MachineEncodedMemoryEffect::HostedReadByteV1 { .. }
            )
            || encoded.trap != MachineEncodedTrapBehavior::HostedReadFailureV1
            || encoded.stack != MachineEncodedStackEffect::UnchangedV1
            || encoded.control != MachineEncodedControlEffect::HostedReadReturnOrTrapV1)
    {
        return Err(());
    }
    if declaration.semantic == MachineSemanticKind::HostedWriteByteI32
        && (declaration.memory != crate::MachineMemoryEffect::HostedWriteByteV1
            || declaration.trap != crate::MachineTrapBehavior::HostedWriteFailureV1
            || !matches!(
                encoded.memory,
                MachineEncodedMemoryEffect::HostedWriteByteV1 { .. }
            )
            || encoded.trap != MachineEncodedTrapBehavior::HostedWriteFailureV1
            || encoded.stack != MachineEncodedStackEffect::UnchangedV1
            || encoded.control != MachineEncodedControlEffect::HostedWriteReturnOrTrapV1)
    {
        return Err(());
    }
    if declaration.semantic == MachineSemanticKind::CallAggregate {
        let arity = constraint
            .operands
            .iter()
            .take_while(|operand| operand.access == RegisterOperandAccess::Use)
            .count();
        let (arguments, results) = constraint.operands.split_at(arity);
        if !(1..=2).contains(&results.len())
            || results.iter().any(|operand| {
                operand.access != RegisterOperandAccess::Def || operand.fixed_view.is_none()
            })
            || !encoded
                .external_operand_reads
                .iter()
                .copied()
                .eq(arguments.iter().map(|operand| operand.operand))
            || !encoded
                .external_operand_writes
                .iter()
                .copied()
                .eq(results.iter().map(|operand| operand.operand))
            || encoded.implicit_unit_uses != constraint.implicit_uses
            || encoded.implicit_unit_defs != constraint.implicit_defs
            || encoded.implicit_unit_clobbers != constraint.clobbers
            || encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            || encoded.control != MachineEncodedControlEffect::DirectRelativeCallV1
        {
            return Err(());
        }
    }
    if declaration.semantic == MachineSemanticKind::CallI64 {
        let (result, arguments) = constraint.operands.split_last().ok_or(())?;
        if result.access != RegisterOperandAccess::Def
            || arguments
                .iter()
                .any(|argument| argument.access != RegisterOperandAccess::Use)
            || !encoded
                .external_operand_reads
                .iter()
                .copied()
                .eq(arguments.iter().map(|argument| argument.operand))
            || encoded.external_operand_writes != [result.operand]
            || encoded.implicit_unit_uses != constraint.implicit_uses
            || encoded.implicit_unit_defs != constraint.implicit_defs
            || encoded.implicit_unit_clobbers != constraint.clobbers
            || encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            || encoded.control != MachineEncodedControlEffect::DirectRelativeCallV1
        {
            return Err(());
        }
    }
    if declaration.semantic == MachineSemanticKind::CallUnit
        && (constraint
            .operands
            .iter()
            .any(|operand| operand.access != RegisterOperandAccess::Use)
            || !encoded
                .external_operand_reads
                .iter()
                .copied()
                .eq(constraint.operands.iter().map(|operand| operand.operand))
            || !encoded.external_operand_writes.is_empty()
            || encoded.implicit_unit_uses != constraint.implicit_uses
            || encoded.implicit_unit_defs != constraint.implicit_defs
            || encoded.implicit_unit_clobbers != constraint.clobbers
            || encoded.control != MachineEncodedControlEffect::DirectRelativeCallV1)
    {
        return Err(());
    }
    let expected_barrier = match encoded.control {
        MachineEncodedControlEffect::HostedReadReturnOrTrapV1
        | MachineEncodedControlEffect::HostedExitOrTrapV1
        | MachineEncodedControlEffect::HostedWriteReturnOrTrapV1 => MachineBarrier::ExternalEffect,
        MachineEncodedControlEffect::FallThroughV1 => MachineBarrier::None,
        MachineEncodedControlEffect::DirectRelativeCallV1 => MachineBarrier::Call,
        MachineEncodedControlEffect::ConditionalRelativeBranchV1
        | MachineEncodedControlEffect::UnconditionalRelativeBranchV1
        | MachineEncodedControlEffect::ReturnFromActivationStackV1
        | MachineEncodedControlEffect::ReturnIndirectRegisterV1 { .. } => {
            MachineBarrier::ControlFlow
        }
    };
    if declaration.barrier != expected_barrier {
        return Err(());
    }
    match (encoded.memory, encoded.stack, encoded.trap) {
        (
            MachineEncodedMemoryEffect::NoneV1,
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::HostedExitReturnedV1,
        ) if declaration.semantic == MachineSemanticKind::HostedExitProcessI32
            && declaration.memory == crate::MachineMemoryEffect::NoneV1
            && declaration.trap == crate::MachineTrapBehavior::HostedExitReturnedV1
            && encoded.control == MachineEncodedControlEffect::HostedExitOrTrapV1
            && encoded.external_operand_reads == [0]
            && encoded.external_operand_writes.is_empty()
            && constraint.operands.len() == 1
            && constraint.operands[0].access == RegisterOperandAccess::Use
            && encoded.implicit_unit_uses == constraint.implicit_uses
            && encoded.implicit_unit_defs == constraint.implicit_defs
            && encoded.implicit_unit_clobbers == constraint.clobbers => {}
        (
            MachineEncodedMemoryEffect::HostedReadByteV1 { .. },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::HostedReadFailureV1,
        ) if declaration.semantic == MachineSemanticKind::HostedReadByte
            && declaration.memory == crate::MachineMemoryEffect::HostedReadByteV1
            && declaration.trap == crate::MachineTrapBehavior::HostedReadFailureV1
            && encoded.control == MachineEncodedControlEffect::HostedReadReturnOrTrapV1
            && encoded.external_operand_reads.is_empty()
            && encoded.external_operand_writes.is_empty()
            && constraint.operands.is_empty()
            && encoded.implicit_unit_uses == constraint.implicit_uses
            && encoded.implicit_unit_defs == constraint.implicit_defs
            && encoded.implicit_unit_clobbers == constraint.clobbers => {}
        (
            MachineEncodedMemoryEffect::HostedWriteByteV1 { .. },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::HostedWriteFailureV1,
        ) if declaration.semantic == MachineSemanticKind::HostedWriteByteI32
            && declaration.memory == crate::MachineMemoryEffect::HostedWriteByteV1
            && declaration.trap == crate::MachineTrapBehavior::HostedWriteFailureV1
            && encoded.control == MachineEncodedControlEffect::HostedWriteReturnOrTrapV1
            && encoded.external_operand_reads == [0]
            && encoded.external_operand_writes.is_empty()
            && constraint.operands.len() == 1
            && constraint.operands[0].access == RegisterOperandAccess::Use
            && encoded.implicit_unit_uses == constraint.implicit_uses
            && encoded.implicit_unit_defs == constraint.implicit_defs
            && encoded.implicit_unit_clobbers == constraint.clobbers => {}
        (
            MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                pointer_operand,
                index_operand,
                byte_count: 1,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.memory == crate::MachineMemoryEffect::ReadPointerV1
            && pointer_operand != index_operand
            && encoded.external_operand_reads.contains(&pointer_operand)
            && encoded.external_operand_reads.contains(&index_operand) => {}
        (
            MachineEncodedMemoryEffect::ReadActivationStackV1 {
                stack_pointer: memory_pointer,
                byte_count: memory_bytes,
            },
            MachineEncodedStackEffect::PopBytesV1 {
                stack_pointer,
                byte_count: stack_bytes,
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if memory_pointer == stack_pointer
            && memory_bytes == stack_bytes
            && memory_bytes != 0 => {}
        (
            MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                stack_pointer: memory_pointer,
                byte_count: memory_bytes,
            },
            MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                stack_pointer,
                return_address_byte_count,
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if memory_pointer == stack_pointer
            && memory_bytes == return_address_byte_count
            && return_address_byte_count != 0 => {}
        (
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand,
                byte_count,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.memory == crate::MachineMemoryEffect::ReadPointerV1
            && matches!(
                (declaration.semantic, byte_count),
                (MachineSemanticKind::Load8, 1)
                    | (MachineSemanticKind::Load16, 2)
                    | (MachineSemanticKind::Load32, 4)
                    | (MachineSemanticKind::Load64, 8)
            )
            && encoded.external_operand_reads.contains(&pointer_operand) => {}
        (
            MachineEncodedMemoryEffect::WriteFrameStorageV1 { byte_count: 8, .. },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.memory == crate::MachineMemoryEffect::WriteFrameStorageV1 => {}
        (
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == crate::MachineSemanticKind::Store
            && declaration.memory == crate::MachineMemoryEffect::WritePointerV1
            && encoded.external_operand_reads == [0, 1]
            && encoded.external_operand_writes.is_empty() => {}
        (MachineEncodedMemoryEffect::NoneV1, MachineEncodedStackEffect::UnchangedV1, _)
            if declaration.memory == crate::MachineMemoryEffect::NoneV1 => {}
        _ => return Err(()),
    }
    Ok(())
}

fn validate_applicability(
    constraint: &RegisterInstructionConstraint,
    applicability: MachineAlternativeApplicability,
) -> Result<(), ()> {
    let operand = |number| {
        constraint
            .operands
            .iter()
            .find(|operand| operand.operand == number)
    };
    let reads = |access| {
        matches!(
            access,
            RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
        )
    };
    let writes = |access| {
        matches!(
            access,
            RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
        )
    };
    match applicability {
        MachineAlternativeApplicability::Always => Ok(()),
        MachineAlternativeApplicability::ResultAliasesOperand {
            result,
            operand: input,
        } => {
            let (Some(result), Some(input)) = (operand(result), operand(input)) else {
                return Err(());
            };
            (result.operand != input.operand
                && writes(result.access)
                && reads(input.access)
                && result.class == input.class)
                .then_some(())
                .ok_or(())
        }
        MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result,
            aliased_operand,
            distinct_operand,
        } => {
            let (Some(result), Some(aliased), Some(distinct)) = (
                operand(result),
                operand(aliased_operand),
                operand(distinct_operand),
            ) else {
                return Err(());
            };
            (result.operand != aliased.operand
                && result.operand != distinct.operand
                && aliased.operand != distinct.operand
                && writes(result.access)
                && reads(aliased.access)
                && reads(distinct.access)
                && result.class == aliased.class
                && result.class == distinct.class)
                .then_some(())
                .ok_or(())
        }
        MachineAlternativeApplicability::ResultAliasesOperands {
            result,
            left,
            right,
        } => {
            let (Some(result), Some(left), Some(right)) =
                (operand(result), operand(left), operand(right))
            else {
                return Err(());
            };
            (result.operand != left.operand
                && result.operand != right.operand
                && left.operand != right.operand
                && writes(result.access)
                && reads(left.access)
                && reads(right.access)
                && result.class == left.class
                && result.class == right.class)
                .then_some(())
                .ok_or(())
        }
        MachineAlternativeApplicability::ResultDistinctFromOperands {
            result,
            left,
            right,
        } => {
            let (Some(result), Some(left), Some(right)) =
                (operand(result), operand(left), operand(right))
            else {
                return Err(());
            };
            (result.operand != left.operand
                && result.operand != right.operand
                && left.operand != right.operand
                && writes(result.access)
                && reads(left.access)
                && reads(right.access)
                && result.class == left.class
                && result.class == right.class)
                .then_some(())
                .ok_or(())
        }
        MachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
            left, right, ..
        } => {
            let (Some(left), Some(right)) = (operand(left), operand(right)) else {
                return Err(());
            };
            (left.operand != right.operand
                && reads(left.access)
                && reads(right.access)
                && left.class == right.class)
                .then_some(())
                .ok_or(())
        }
    }
}
