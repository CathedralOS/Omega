use omega_register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, ValidatedRegisterConstraintCatalog,
};

use super::{
    MachineAlternativeApplicability, MachineBarrier, MachineCleanupEffect, MachineEffectCatalog,
    MachineEffectCatalogValidationError, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
    StructuralUnitCallBarrier, StructuralUnitCallEffect, StructuralUnitCallFrameEffect,
    StructuralUnitCallMemoryEffect,
};
pub(super) fn validate_structural_unit_call(
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &MachineEffectCatalog,
) -> Result<(), MachineEffectCatalogValidationError> {
    let (Some(key), Some(declaration)) = (
        catalog.selected_keys.structural_unit_call,
        catalog.structural_unit_call,
    ) else {
        return if catalog.selected_keys.structural_unit_call.is_none()
            && catalog.structural_unit_call.is_none()
        {
            Ok(())
        } else {
            Err(MachineEffectCatalogValidationError::StructuralCallDeclarationMismatch)
        };
    };
    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == key)
        .ok_or(MachineEffectCatalogValidationError::StructuralCallDeclarationMismatch)?;
    if declaration.constraint != key
        || !row.operands.is_empty()
        || row.implicit_uses.is_empty()
        || row.implicit_defs.is_empty()
        || row.clobbers.is_empty()
        || declaration.memory
            != (StructuralUnitCallMemoryEffect::ReadOwnedIndirectPairWriteCallerCopiesV1 {
                root_byte_count: 16,
                copy_stack_byte_offsets: [32, 48],
            })
        || declaration.frame
            != (StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
                frame_byte_count: 72,
                shadow_byte_count: 32,
                pre_call_stack_alignment: 16,
            })
        || declaration.trap != MachineTrapBehavior::MayArchitecturalFaultV1
        || declaration.barrier != StructuralUnitCallBarrier::CallV1
        || declaration.call != StructuralUnitCallEffect::DirectInternalUnitV1
        || declaration.cleanup != MachineCleanupEffect::NoneV1
    {
        return Err(MachineEffectCatalogValidationError::StructuralCallDeclarationMismatch);
    }
    Ok(())
}

pub(super) fn validate_declaration(
    constraint: &RegisterInstructionConstraint,
    declaration: &MachineEffectDeclaration,
) -> Result<(), MachineEffectCatalogValidationError> {
    let semantic = declaration.semantic;
    let expected_barrier = if matches!(
        semantic,
        MachineSemanticKind::ConditionalBranchNonZero
            | MachineSemanticKind::ConditionalBranchU64LessThan
            | MachineSemanticKind::ConditionalBranchI64LessThan
            | MachineSemanticKind::ReturnI64
            | MachineSemanticKind::ReturnUnit
    ) {
        MachineBarrier::ControlFlow
    } else {
        MachineBarrier::None
    };
    if declaration.barrier != expected_barrier {
        return Err(MachineEffectCatalogValidationError::BarrierMismatch(
            semantic,
        ));
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
    let control = !matches!(encoded.control, MachineEncodedControlEffect::FallThroughV1);
    if control != matches!(declaration.barrier, MachineBarrier::ControlFlow) {
        return Err(());
    }
    match (encoded.memory, encoded.stack, encoded.trap) {
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
        (MachineEncodedMemoryEffect::NoneV1, MachineEncodedStackEffect::UnchangedV1, _) => {}
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
