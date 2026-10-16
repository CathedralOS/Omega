use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};

#[cfg(test)]
mod atomic_tests;
#[cfg(test)]
mod float_result_policy_tests;
#[cfg(test)]
mod tests;

use super::{
    MachineAlternativeApplicability, MachineBarrier, MachineEffectCatalogValidationError,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineSemanticKind, MachineSizeKnowledge,
};

/// Float-to-integer Trapping is the same observable policy check as integer
/// Trapping: a failed representability predicate executes an in-place Crash.
/// Exact and Saturating conversions must not borrow that control/effect surface.
fn has_policy_trap(semantic: MachineSemanticKind) -> bool {
    matches!(semantic, MachineSemanticKind::TrappingInteger(_))
        || matches!(semantic, MachineSemanticKind::FloatResultPolicy(form) if form.can_trap())
        || matches!(
            semantic,
            MachineSemanticKind::NearestIeeeFloatArithmetic(crate::IeeeFloatArithmeticForm {
                operation: crate::IeeeFloatArithmeticOperation::TruncateToInteger {
                    domain: crate::IeeeFloatToIntegerDomain::Trapping,
                    ..
                },
                ..
            })
        )
}

fn encoded_scalar_constraint(
    constraint: &register_model::RegisterInstructionConstraint,
    inputs: usize,
) -> bool {
    constraint.operands.len() == 6
        && constraint.implicit_uses.is_empty()
        && constraint.implicit_defs.is_empty()
        && !constraint.clobbers.is_empty()
        && constraint
            .operands
            .iter()
            .enumerate()
            .all(|(position, operand)| {
                operand.operand as usize == position
                    && operand.access
                        == if position < inputs {
                            register_model::RegisterOperandAccess::Use
                        } else {
                            register_model::RegisterOperandAccess::Def
                        }
                    && operand.early_clobber == (position >= inputs)
                    && operand.fixed_view.is_none()
                    && operand.tied_to.is_none()
            })
}

pub(super) fn validate_declaration(
    architecture: target::Architecture,
    constraint: &RegisterInstructionConstraint,
    declaration: &MachineEffectDeclaration,
) -> Result<(), MachineEffectCatalogValidationError> {
    let semantic = declaration.semantic;
    // A Trapping form may stop the process in place, which is as observable
    // as a hosted effect: it must not move across one, and it is never dead.
    let expected_barrier = if has_policy_trap(semantic)
        || matches!(
            semantic,
            MachineSemanticKind::HostedReadByte
                | MachineSemanticKind::SaveFloatingControl
                | MachineSemanticKind::RestoreFloatingControl
                | MachineSemanticKind::HostedWriteByteI32
                | MachineSemanticKind::HostedExitProcessI32
                | MachineSemanticKind::PortWrite
                | MachineSemanticKind::SystemRegisterRead
                | MachineSemanticKind::SystemRegisterWrite
                | MachineSemanticKind::PortRegisterRead
                | MachineSemanticKind::PortRegisterWrite
                | MachineSemanticKind::ModelSpecificRead
                | MachineSemanticKind::ModelSpecificWrite
                | MachineSemanticKind::ProcessorHalt
                | MachineSemanticKind::AtomicFence
                | MachineSemanticKind::AtomicAccess(_)
                | MachineSemanticKind::PlacedScalarAccess(_)
        ) {
        MachineBarrier::ExternalEffect
    } else if matches!(
        semantic,
        MachineSemanticKind::Crash
            | MachineSemanticKind::ConditionalBranchNonZero
            | MachineSemanticKind::Jump
            | MachineSemanticKind::ConditionalBranchU64LessThan
            | MachineSemanticKind::ConditionalBranchI64LessThan
            | MachineSemanticKind::ReturnScalar
            | MachineSemanticKind::ReturnAggregate
            | MachineSemanticKind::ReturnUnit
    ) {
        MachineBarrier::ControlFlow
    } else if matches!(
        semantic,
        MachineSemanticKind::CallScalar
            | MachineSemanticKind::CallUnit
            | MachineSemanticKind::CallAggregate
            | MachineSemanticKind::NormalizedForeignCall
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
    // The declared memory and trap surfaces are functions of the selected
    // semantic — the same contract the barrier and call surfaces already
    // follow. A footprint class belongs to the semantics whose encoding
    // performs it; every other declaration — including returns and calls,
    // whose activation-stack lifecycle is described by the encoded stack
    // and call surfaces instead — declares no memory access. A hosted trap
    // result names its owning hosted operation, a Trapping form names its
    // policy trap, only a semantic whose encoded work dereferences memory
    // declares the bare architectural fault, and every other rule declares
    // that it never faults.
    // Understating a surface the rule needs, or overstating one it cannot
    // have, fails admission instead of reaching canonical replay.
    let temporary_stack = architecture == target::Architecture::X86_64
        && matches!(semantic, MachineSemanticKind::NearestIeeeFloatArithmetic(form) if form.operation.rounding_direction().is_some());
    if semantic == MachineSemanticKind::PortWrite
        && (architecture != target::Architecture::X86_64
            || !constraint.operands.is_empty()
            || !constraint.implicit_uses.is_empty()
            || !constraint.implicit_defs.is_empty())
    {
        return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
            semantic,
        ));
    }
    if semantic == MachineSemanticKind::AtomicFence
        && (!constraint.operands.is_empty()
            || !constraint.implicit_uses.is_empty()
            || !constraint.implicit_defs.is_empty()
            || !constraint.clobbers.is_empty()
            || declaration.alternatives.iter().any(|alternative| {
                alternative.encoded != crate::MachineEncodedEffects::atomic_fence_v1()
            }))
    {
        return Err(MachineEffectCatalogValidationError::BarrierMismatch(
            semantic,
        ));
    }
    let table_call = semantic == MachineSemanticKind::NormalizedForeignCall
        && !declaration.alternatives.is_empty()
        && declaration.alternatives.iter().all(|alternative| {
            alternative.encoded.control == MachineEncodedControlEffect::IndirectExternalCallV1
                && matches!(
                    alternative.encoded.memory,
                    MachineEncodedMemoryEffect::ReadTableFieldV1 { .. }
                )
        });
    let expected_memory = match semantic {
        _ if table_call => crate::MachineMemoryEffect::ReadPointerV1,
        _ if temporary_stack => crate::MachineMemoryEffect::TemporaryStackStorageV1,
        MachineSemanticKind::AtomicFence => crate::MachineMemoryEffect::AtomicFenceV1,
        MachineSemanticKind::AtomicAccess(_) => crate::MachineMemoryEffect::AtomicPointerV1,
        MachineSemanticKind::PlacedScalarAccess(form) => {
            if form.write {
                crate::MachineMemoryEffect::WritePointerV1
            } else {
                crate::MachineMemoryEffect::ReadPointerV1
            }
        }
        MachineSemanticKind::CopyBytes => crate::MachineMemoryEffect::CopyBytesV1,
        MachineSemanticKind::ZeroBytes => crate::MachineMemoryEffect::ZeroBytesV1,
        MachineSemanticKind::ScanByteSequence => crate::MachineMemoryEffect::ScanByteSequenceV1,
        MachineSemanticKind::LoadEncodedScalar => crate::MachineMemoryEffect::ReadEncodedScalarV1,
        MachineSemanticKind::StoreEncodedScalar => crate::MachineMemoryEffect::WriteEncodedScalarV1,
        MachineSemanticKind::CompareByteSequences => {
            crate::MachineMemoryEffect::CompareByteSequencesV1
        }
        MachineSemanticKind::HostedReadByte => crate::MachineMemoryEffect::HostedReadByteV1,
        MachineSemanticKind::HostedWriteByteI32 => crate::MachineMemoryEffect::HostedWriteByteV1,
        MachineSemanticKind::Load8
        | MachineSemanticKind::Load16
        | MachineSemanticKind::Load32
        | MachineSemanticKind::Load64
        | MachineSemanticKind::Load8Indexed
        | MachineSemanticKind::LoadPacked3
        | MachineSemanticKind::LoadPacked5
        | MachineSemanticKind::LoadPacked6
        | MachineSemanticKind::LoadPacked7 => crate::MachineMemoryEffect::ReadPointerV1,
        MachineSemanticKind::Store64 | MachineSemanticKind::SaveFloatingControl => {
            crate::MachineMemoryEffect::WriteFrameStorageV1
        }
        MachineSemanticKind::RestoreFloatingControl => {
            crate::MachineMemoryEffect::ReadFrameStorageV1
        }
        MachineSemanticKind::Store | MachineSemanticKind::StorePacked => {
            crate::MachineMemoryEffect::WritePointerV1
        }
        _ => crate::MachineMemoryEffect::NoneV1,
    };
    if declaration.memory != expected_memory {
        return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
            semantic,
        ));
    }
    let expected_trap = match semantic {
        _ if table_call => crate::MachineTrapBehavior::MayArchitecturalFaultV1,
        _ if temporary_stack => crate::MachineTrapBehavior::MayArchitecturalFaultV1,
        MachineSemanticKind::PortWrite
        | MachineSemanticKind::SystemRegisterRead
        | MachineSemanticKind::SystemRegisterWrite
        | MachineSemanticKind::PortRegisterRead
        | MachineSemanticKind::PortRegisterWrite
        | MachineSemanticKind::ModelSpecificRead
        | MachineSemanticKind::ModelSpecificWrite
        | MachineSemanticKind::ProcessorHalt
        | MachineSemanticKind::AtomicAccess(_)
        | MachineSemanticKind::PlacedScalarAccess(_) => {
            crate::MachineTrapBehavior::MayArchitecturalFaultV1
        }
        MachineSemanticKind::Crash => crate::MachineTrapBehavior::ExplicitCrashV1,
        semantic if has_policy_trap(semantic) => crate::MachineTrapBehavior::TrappingIntegerV1,
        MachineSemanticKind::HostedExitProcessI32 => {
            crate::MachineTrapBehavior::HostedExitReturnedV1
        }
        MachineSemanticKind::HostedReadByte => crate::MachineTrapBehavior::HostedReadFailureV1,
        MachineSemanticKind::HostedWriteByteI32 => crate::MachineTrapBehavior::HostedWriteFailureV1,
        MachineSemanticKind::CopyBytes
        | MachineSemanticKind::ZeroBytes
        | MachineSemanticKind::ScanByteSequence
        | MachineSemanticKind::LoadEncodedScalar
        | MachineSemanticKind::StoreEncodedScalar
        | MachineSemanticKind::CompareByteSequences
        | MachineSemanticKind::Load8
        | MachineSemanticKind::Load16
        | MachineSemanticKind::Load32
        | MachineSemanticKind::Load64
        | MachineSemanticKind::Load8Indexed
        | MachineSemanticKind::LoadPacked3
        | MachineSemanticKind::LoadPacked5
        | MachineSemanticKind::LoadPacked6
        | MachineSemanticKind::LoadPacked7
        | MachineSemanticKind::Store
        | MachineSemanticKind::StorePacked
        | MachineSemanticKind::Store64
        | MachineSemanticKind::SaveFloatingControl
        | MachineSemanticKind::RestoreFloatingControl => {
            crate::MachineTrapBehavior::MayArchitecturalFaultV1
        }
        _ => crate::MachineTrapBehavior::NeverV1,
    };
    if declaration.trap != expected_trap {
        return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
            semantic,
        ));
    }
    // The cleanup surface binds fail-closed even though its vocabulary has
    // one value today: a second variant must name its owning semantics here
    // before any declaration may carry it.
    if declaration.cleanup != crate::MachineCleanupEffect::NoneV1 {
        return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
            semantic,
        ));
    }
    match (semantic, declaration.call) {
        (
            MachineSemanticKind::CallScalar
            | MachineSemanticKind::CallUnit
            | MachineSemanticKind::CallAggregate,
            crate::MachineCallEffect::InternalNormalReturnV1 {
                pre_call_stack_alignment,
            },
        ) if pre_call_stack_alignment.is_power_of_two() => {}
        (
            MachineSemanticKind::CallScalar
            | MachineSemanticKind::CallUnit
            | MachineSemanticKind::CallAggregate,
            _,
        ) => {
            return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                semantic,
            ));
        }
        (
            MachineSemanticKind::NormalizedForeignCall,
            crate::MachineCallEffect::ExternalNormalReturnV1 {
                pre_call_stack_alignment,
            },
        ) if pre_call_stack_alignment.is_power_of_two()
            && constraint.key.family == register_model::RegisterConstraintFamily::Call => {}
        (
            MachineSemanticKind::NormalizedForeignCall,
            crate::MachineCallEffect::SupervisorNormalReturnV1,
        ) if constraint.key.family == register_model::RegisterConstraintFamily::SystemCall => {}
        (MachineSemanticKind::NormalizedForeignCall, _) => {
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
        validate_encoded_effects(
            architecture,
            constraint,
            declaration,
            alternative.applicability,
            &alternative.encoded,
        )
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
    architecture: target::Architecture,
    constraint: &RegisterInstructionConstraint,
    declaration: &MachineEffectDeclaration,
    applicability: MachineAlternativeApplicability,
    encoded: &MachineEncodedEffects,
) -> Result<(), ()> {
    if let MachineSemanticKind::FloatResultPolicy(form) = declaration.semantic {
        // Policy forms consume completed payloads in semantic order, then
        // define one result. A self-consistent but wrong-arity constraint is
        // not evidence for the adapter's actual dependencies.
        let input_count = form.operand_count();
        if constraint.operands.len() != input_count + 1
            || constraint
                .operands
                .iter()
                .enumerate()
                .any(|(position, operand)| {
                    usize::from(operand.operand) != position
                        || operand.access
                            != if position < input_count {
                                RegisterOperandAccess::Use
                            } else {
                                RegisterOperandAccess::Def
                            }
                })
        {
            return Err(());
        }
    }
    if declaration.semantic == MachineSemanticKind::Crash {
        return if constraint.operands.is_empty()
            && !constraint.implicit_uses.is_empty()
            && constraint.implicit_defs == constraint.implicit_uses
            && constraint.clobbers.is_empty()
            && encoded.external_operand_reads.is_empty()
            && encoded.external_operand_writes.is_empty()
            && encoded.implicit_unit_uses == constraint.implicit_uses
            && encoded.implicit_unit_defs == constraint.implicit_defs
            && encoded.implicit_unit_clobbers.is_empty()
            && encoded.memory == MachineEncodedMemoryEffect::NoneV1
            && encoded.stack == MachineEncodedStackEffect::UnchangedV1
            && encoded.trap == MachineEncodedTrapBehavior::ExplicitCrashV1
            && encoded.control == MachineEncodedControlEffect::CrashV1
            && applicability == MachineAlternativeApplicability::Always
        {
            Ok(())
        } else {
            Err(())
        };
    }
    // Packed forms use a real early definition for their instruction-local
    // scratch. Neither the memory footprint nor that interference may be
    // weakened by a catalog row advertising only the eventual source result.
    if matches!(
        declaration.semantic,
        MachineSemanticKind::LoadPacked3
            | MachineSemanticKind::LoadPacked5
            | MachineSemanticKind::LoadPacked6
            | MachineSemanticKind::LoadPacked7
            | MachineSemanticKind::StorePacked
    ) {
        let load = declaration.semantic != MachineSemanticKind::StorePacked;
        if constraint.operands.len() != 3
            || constraint
                .operands
                .iter()
                .enumerate()
                .any(|(index, operand)| {
                    let writes = index == 2 || (load && index == 1);
                    operand.operand != index as u16
                        || operand.access
                            != if writes {
                                RegisterOperandAccess::Def
                            } else {
                                RegisterOperandAccess::Use
                            }
                        || operand.early_clobber != writes
                        || operand.tied_to.is_some()
                })
            || encoded.external_operand_reads.as_slice()
                != if load { &[0][..] } else { &[0, 1][..] }
            || encoded.external_operand_writes.as_slice()
                != if load { &[1, 2][..] } else { &[2][..] }
        {
            return Err(());
        }
    }
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
        let row = constraint.operand(*operand).ok_or(())?;
        if !matches!(
            row.access,
            RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
        ) {
            return Err(());
        }
    }
    for operand in &encoded.external_operand_writes {
        let row = constraint.operand(*operand).ok_or(())?;
        if !matches!(
            row.access,
            RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
        ) {
            return Err(());
        }
    }
    // The encoded row must restate its constraint row's operand contract
    // exactly: every contracted definition is a declared write, and every
    // contracted input is a declared read. Membership alone is not enough —
    // a row that drops a contracted dependency understates the surface its
    // instruction's operand homes are allocated against, and a row that
    // invents one misstates the alternative's encoding.
    if !encoded
        .external_operand_writes
        .iter()
        .copied()
        .eq(constraint.operands.iter().filter_map(|operand| {
            matches!(
                operand.access,
                RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
            )
            .then_some(operand.operand)
        }))
    {
        return Err(());
    }
    let mut contracted_reads = constraint
        .operands
        .iter()
        .filter_map(|operand| {
            matches!(
                operand.access,
                RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
            )
            .then_some(operand.operand)
        })
        .collect::<Vec<u16>>();
    match declaration.semantic {
        // A return's operand homes are placed for the caller; the return
        // encoding itself consumes no selected operand's incoming value.
        MachineSemanticKind::ReturnScalar
        | MachineSemanticKind::ReturnAggregate
        | MachineSemanticKind::ReturnUnit => contracted_reads.clear(),
        // An all-aliased subtract realizes x - x as a form whose result
        // depends on neither input home; only that alternative may drop the
        // aliased operands from its read surface.
        MachineSemanticKind::ExactSubtractI64 | MachineSemanticKind::WrappingSubtractI64 => {
            if let MachineAlternativeApplicability::ResultAliasesOperands { left, right, .. } =
                applicability
            {
                contracted_reads.retain(|operand| *operand != left && *operand != right);
            }
        }
        _ => {}
    }
    if encoded.external_operand_reads != contracted_reads {
        return Err(());
    }
    // Implicit custody restates the row with the same exactness as operand
    // custody: every contracted implicit definition and declared clobber is
    // one the encoding performs. A row that drops one understates the
    // interference its register homes are allocated against. An
    // optimizer-selected form may clobber units beyond its row — the units
    // the post-allocation phase proves dead before selecting it — but never
    // fewer.
    let clobbers_match = match applicability {
        MachineAlternativeApplicability::OptimizationSelected { .. } => constraint
            .clobbers
            .iter()
            .all(|unit| encoded.implicit_unit_clobbers.contains(unit)),
        _ => encoded.implicit_unit_clobbers == constraint.clobbers,
    };
    if encoded.implicit_unit_defs != constraint.implicit_defs || !clobbers_match {
        return Err(());
    }
    let implicit_uses_match = match encoded.control {
        // An indirect-register return honestly reads only the register its
        // control effect names — AArch64's link register — while the row's
        // implicit uses also carry ABI state, such as the stack pointer,
        // that the encoding does not consume. That row may narrow to a
        // non-empty subset of the contracted uses: a use list that reads
        // nothing has dropped the return-address read the control effect
        // still performs.
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { .. } => {
            encoded.implicit_unit_uses == constraint.implicit_uses
                || (!encoded.implicit_unit_uses.is_empty()
                    && encoded
                        .implicit_unit_uses
                        .iter()
                        .all(|unit| constraint.implicit_uses.contains(unit)))
        }
        _ => encoded.implicit_unit_uses == constraint.implicit_uses,
    };
    if !implicit_uses_match {
        return Err(());
    }
    if declaration.semantic == MachineSemanticKind::CopyBytes
        && (declaration.memory != crate::MachineMemoryEffect::CopyBytesV1
            || !matches!(
                encoded.memory,
                MachineEncodedMemoryEffect::CopyBytesV1 { .. }
            )
            || declaration.call != crate::MachineCallEffect::NoneV1
            || !constraint.implicit_uses.is_empty()
            || !constraint.implicit_defs.is_empty()
            || constraint.clobbers.is_empty())
    {
        return Err(());
    }
    if declaration.semantic == MachineSemanticKind::ZeroBytes
        && (declaration.memory != crate::MachineMemoryEffect::ZeroBytesV1
            || !matches!(
                encoded.memory,
                MachineEncodedMemoryEffect::ZeroBytesV1 { .. }
            )
            || declaration.call != crate::MachineCallEffect::NoneV1
            || !constraint.implicit_uses.is_empty()
            || !constraint.implicit_defs.is_empty()
            || constraint.clobbers.is_empty())
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
        // An indirect aggregate returns through its hidden input pointer; its
        // exact ABI and destination are checked by ordinary call replay.
        if results.len() > 2
            || (results.is_empty() && arguments.is_empty())
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
    if declaration.semantic == MachineSemanticKind::CallScalar {
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
    if declaration.semantic == MachineSemanticKind::NormalizedForeignCall {
        let supervisor =
            constraint.key.family == register_model::RegisterConstraintFamily::SystemCall;
        // Kernel entry is not an imported CALL/BL. Even a declaration with
        // the right operands must reject a fabricated return-address write,
        // stack lifecycle, or ordinary call control.
        let control = if supervisor {
            if encoded.memory != MachineEncodedMemoryEffect::NoneV1
                || encoded.stack != MachineEncodedStackEffect::UnchangedV1
            {
                return Err(());
            }
            MachineEncodedControlEffect::SupervisorCallV1
        } else if matches!(
            encoded.memory,
            MachineEncodedMemoryEffect::ReadTableFieldV1 { .. }
        ) {
            MachineEncodedControlEffect::IndirectExternalCallV1
        } else {
            MachineEncodedControlEffect::DirectRelativeCallV1
        };
        let arity = constraint
            .operands
            .iter()
            .take_while(|operand| operand.access == RegisterOperandAccess::Use)
            .count();
        let (arguments, results) = constraint.operands.split_at(arity);
        // A normalized foreign row is a per-plan fixed-view roster: leading
        // Use arguments and trailing Def result fragments, every operand
        // pinned to the exact ABI view the evaluated boundary plan selected.
        if constraint
            .operands
            .iter()
            .any(|operand| operand.fixed_view.is_none())
            || results
                .iter()
                .any(|operand| operand.access != RegisterOperandAccess::Def)
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
            || encoded.control != control
        {
            return Err(());
        }
    }
    // The fall-through-or-trap surface belongs to the Trapping forms alone,
    // with exactly their trap surface and no memory or stack effect.
    if has_policy_trap(declaration.semantic)
        != (encoded.control == MachineEncodedControlEffect::FallThroughOrTrapV1)
        || has_policy_trap(declaration.semantic)
            != (encoded.trap == MachineEncodedTrapBehavior::TrappingIntegerV1)
    {
        return Err(());
    }
    let expected_barrier = match encoded.control {
        MachineEncodedControlEffect::AtomicRetryUntilCommitV1
            if architecture == target::Architecture::X86_64
                && matches!(declaration.semantic, MachineSemanticKind::AtomicAccess(form)
                    if matches!(form.operation, crate::AtomicOperation::FetchAnd | crate::AtomicOperation::FetchOr | crate::AtomicOperation::FetchXor)) =>
        {
            MachineBarrier::ExternalEffect
        }
        MachineEncodedControlEffect::AtomicRetryUntilCommitV1 => return Err(()),
        MachineEncodedControlEffect::CrashV1 => return Err(()),
        MachineEncodedControlEffect::FallThroughOrTrapV1 => MachineBarrier::ExternalEffect,
        MachineEncodedControlEffect::HostedReadReturnOrTrapV1
        | MachineEncodedControlEffect::HostedExitOrTrapV1
        | MachineEncodedControlEffect::HostedWriteReturnOrTrapV1 => MachineBarrier::ExternalEffect,
        MachineEncodedControlEffect::FallThroughV1
            if matches!(
                declaration.semantic,
                MachineSemanticKind::SaveFloatingControl
                    | MachineSemanticKind::RestoreFloatingControl
                    | MachineSemanticKind::PortWrite
                    | MachineSemanticKind::SystemRegisterRead
                    | MachineSemanticKind::SystemRegisterWrite
                    | MachineSemanticKind::PortRegisterRead
                    | MachineSemanticKind::PortRegisterWrite
                    | MachineSemanticKind::ModelSpecificRead
                    | MachineSemanticKind::ModelSpecificWrite
                    | MachineSemanticKind::ProcessorHalt
                    | MachineSemanticKind::AtomicFence
                    | MachineSemanticKind::AtomicAccess(_)
                    | MachineSemanticKind::PlacedScalarAccess(_)
            ) =>
        {
            MachineBarrier::ExternalEffect
        }
        MachineEncodedControlEffect::FallThroughV1 => MachineBarrier::None,
        MachineEncodedControlEffect::DirectRelativeCallV1
        | MachineEncodedControlEffect::IndirectTableCallOrTrapV1
        | MachineEncodedControlEffect::IndirectExternalCallV1 => MachineBarrier::Call,
        MachineEncodedControlEffect::SupervisorCallV1
            if declaration.semantic == MachineSemanticKind::NormalizedForeignCall
                && declaration.call == crate::MachineCallEffect::SupervisorNormalReturnV1 =>
        {
            MachineBarrier::Call
        }
        MachineEncodedControlEffect::SupervisorCallV1 => return Err(()),
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
    // A control-flow rule also pins its shape inside the barrier class: a
    // Jump cannot borrow a conditional edge, and a return cannot borrow a
    // branch or call encoding.
    let control_shape_matches = match declaration.semantic {
        MachineSemanticKind::ConditionalBranchNonZero
        | MachineSemanticKind::ConditionalBranchU64LessThan
        | MachineSemanticKind::ConditionalBranchI64LessThan => {
            encoded.control == MachineEncodedControlEffect::ConditionalRelativeBranchV1
        }
        MachineSemanticKind::Jump => {
            encoded.control == MachineEncodedControlEffect::UnconditionalRelativeBranchV1
        }
        MachineSemanticKind::ReturnScalar
        | MachineSemanticKind::ReturnAggregate
        | MachineSemanticKind::ReturnUnit => matches!(
            encoded.control,
            MachineEncodedControlEffect::ReturnFromActivationStackV1
                | MachineEncodedControlEffect::ReturnIndirectRegisterV1 { .. }
        ),
        _ => true,
    };
    if !control_shape_matches {
        return Err(());
    }
    if matches!(
        declaration.semantic,
        MachineSemanticKind::SaveFloatingControl | MachineSemanticKind::RestoreFloatingControl
    ) && (!constraint.operands.is_empty()
        || !encoded.external_operand_reads.is_empty()
        || !encoded.external_operand_writes.is_empty()
        || encoded.implicit_unit_uses != constraint.implicit_uses
        || encoded.implicit_unit_defs != constraint.implicit_defs
        || encoded.implicit_unit_clobbers != constraint.clobbers
        || encoded.control != MachineEncodedControlEffect::FallThroughV1)
    {
        return Err(());
    }
    match (encoded.memory, encoded.stack, encoded.trap) {
        (
            MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
                stack_pointer: memory_pointer,
                byte_count: 16,
            },
            MachineEncodedStackEffect::TemporaryBytesV1 {
                stack_pointer,
                byte_count: 16,
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if memory_pointer == stack_pointer
            && declaration.memory == crate::MachineMemoryEffect::TemporaryStackStorageV1
            && matches!(declaration.semantic, MachineSemanticKind::NearestIeeeFloatArithmetic(form) if form.operation.rounding_direction().is_some()) =>
            {}
        (
            MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                byte_count: 4 | 8, ..
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::SaveFloatingControl
            && declaration.memory == crate::MachineMemoryEffect::WriteFrameStorageV1 => {}
        (
            MachineEncodedMemoryEffect::ReadFrameStorageV1 {
                byte_count: 4 | 8, ..
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::RestoreFloatingControl
            && declaration.memory == crate::MachineMemoryEffect::ReadFrameStorageV1 => {}
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
            MachineEncodedMemoryEffect::CopyBytesV1 {
                source_pointer_operand: 0,
                destination_pointer_operand: 1,
                count_operand: 2,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::CopyBytes
            && declaration.memory == crate::MachineMemoryEffect::CopyBytesV1
            && declaration.trap == crate::MachineTrapBehavior::MayArchitecturalFaultV1
            && encoded.control == MachineEncodedControlEffect::FallThroughV1
            && encoded.external_operand_reads == [0, 1, 2]
            && encoded.external_operand_writes == [3, 4]
            && constraint.operands.len() == 5
            && constraint
                .operands
                .iter()
                .enumerate()
                .all(|(position, operand)| {
                    operand.operand as usize == position
                        && operand.access
                            == if position < 3 {
                                RegisterOperandAccess::Use
                            } else {
                                RegisterOperandAccess::Def
                            }
                        && operand.early_clobber == (position >= 3)
                        && operand.fixed_view.is_none()
                        && operand.tied_to.is_none()
                })
            && encoded.implicit_unit_uses == constraint.implicit_uses
            && encoded.implicit_unit_defs == constraint.implicit_defs
            && encoded.implicit_unit_clobbers == constraint.clobbers => {}
        (
            MachineEncodedMemoryEffect::ZeroBytesV1 {
                destination_pointer_operand: 0,
                count_operand: 1,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::ZeroBytes
            && declaration.memory == crate::MachineMemoryEffect::ZeroBytesV1
            && declaration.trap == crate::MachineTrapBehavior::MayArchitecturalFaultV1
            && encoded.control == MachineEncodedControlEffect::FallThroughV1
            && encoded.external_operand_reads == [0, 1]
            && encoded.external_operand_writes == [2]
            && constraint.operands.len() == 3
            && constraint
                .operands
                .iter()
                .enumerate()
                .all(|(position, operand)| {
                    operand.operand as usize == position
                        && operand.access
                            == if position < 2 {
                                RegisterOperandAccess::Use
                            } else {
                                RegisterOperandAccess::Def
                            }
                        && operand.early_clobber == (position >= 2)
                        && operand.fixed_view.is_none()
                        && operand.tied_to.is_none()
                })
            && encoded.implicit_unit_uses == constraint.implicit_uses
            && encoded.implicit_unit_defs == constraint.implicit_defs
            && encoded.implicit_unit_clobbers == constraint.clobbers => {}
        (
            MachineEncodedMemoryEffect::ReadEncodedScalarV1 {
                pointer_operand: 0,
                codec_operand: 1,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::LoadEncodedScalar
            && declaration.memory == crate::MachineMemoryEffect::ReadEncodedScalarV1
            && encoded.control == MachineEncodedControlEffect::FallThroughV1
            && encoded.external_operand_reads == [0, 1]
            && encoded.external_operand_writes == [2, 3, 4, 5]
            && encoded_scalar_constraint(constraint, 2) => {}
        (
            MachineEncodedMemoryEffect::WriteEncodedScalarV1 {
                pointer_operand: 0,
                codec_operand: 1,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::StoreEncodedScalar
            && declaration.memory == crate::MachineMemoryEffect::WriteEncodedScalarV1
            && encoded.control == MachineEncodedControlEffect::FallThroughV1
            && encoded.external_operand_reads == [0, 1, 2]
            && encoded.external_operand_writes == [3, 4, 5]
            && encoded_scalar_constraint(constraint, 3) => {}
        (
            MachineEncodedMemoryEffect::ScanByteSequenceV1 {
                base_pointer_operand: 0,
                count_operand: 1,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::ScanByteSequence
            && declaration.memory == crate::MachineMemoryEffect::ScanByteSequenceV1
            && declaration.trap == crate::MachineTrapBehavior::MayArchitecturalFaultV1
            && encoded.control == MachineEncodedControlEffect::FallThroughV1
            && encoded.external_operand_reads == [0, 1]
            && encoded.external_operand_writes == [2, 3, 4]
            && constraint.operands.len() == 5
            && constraint
                .operands
                .iter()
                .enumerate()
                .all(|(position, operand)| {
                    operand.operand as usize == position
                        && operand.access
                            == if position < 2 {
                                RegisterOperandAccess::Use
                            } else {
                                RegisterOperandAccess::Def
                            }
                        && operand.early_clobber == (position >= 2)
                        && operand.fixed_view.is_none()
                        && operand.tied_to.is_none()
                })
            && encoded.implicit_unit_uses == constraint.implicit_uses
            && encoded.implicit_unit_defs == constraint.implicit_defs
            && encoded.implicit_unit_clobbers == constraint.clobbers => {}
        (
            MachineEncodedMemoryEffect::CompareByteSequencesV1 {
                left_base_operand: 0,
                left_count_operand: 1,
                right_base_operand: 2,
                right_count_operand: 3,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::CompareByteSequences
            && declaration.memory == crate::MachineMemoryEffect::CompareByteSequencesV1
            && declaration.trap == crate::MachineTrapBehavior::MayArchitecturalFaultV1
            && encoded.control == MachineEncodedControlEffect::FallThroughV1
            && encoded.external_operand_reads == [0, 1, 2, 3]
            && encoded.external_operand_writes == [4, 5, 6, 7]
            && constraint.operands.len() == 8
            && constraint
                .operands
                .iter()
                .enumerate()
                .all(|(position, operand)| {
                    operand.operand as usize == position
                        && operand.access
                            == if position < 4 {
                                RegisterOperandAccess::Use
                            } else {
                                RegisterOperandAccess::Def
                            }
                        && operand.early_clobber == (position >= 4)
                        && operand.fixed_view.is_none()
                        && operand.tied_to.is_none()
                })
            && encoded.implicit_unit_uses == constraint.implicit_uses
            && encoded.implicit_unit_defs == constraint.implicit_defs
            && encoded.implicit_unit_clobbers == constraint.clobbers => {}
        (
            MachineEncodedMemoryEffect::ReadTableFieldV1 {
                pointer_operand, ..
            },
            MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                return_address_byte_count: 8,
                ..
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::NormalizedForeignCall
            && declaration.memory == crate::MachineMemoryEffect::ReadPointerV1
            && encoded.control == MachineEncodedControlEffect::IndirectExternalCallV1
            && encoded.external_operand_reads.contains(&pointer_operand) => {}
        (
            MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                pointer_operand,
                index_operand,
                byte_count: 1,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::Load8Indexed
            && declaration.memory == crate::MachineMemoryEffect::ReadPointerV1
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
        ) if matches!(
            declaration.semantic,
            MachineSemanticKind::ReturnScalar
                | MachineSemanticKind::ReturnAggregate
                | MachineSemanticKind::ReturnUnit
        ) && memory_pointer == stack_pointer
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
        ) if matches!(
            declaration.semantic,
            MachineSemanticKind::CallScalar
                | MachineSemanticKind::CallUnit
                | MachineSemanticKind::CallAggregate
                | MachineSemanticKind::NormalizedForeignCall
        ) && memory_pointer == stack_pointer
            && memory_bytes == return_address_byte_count
            && return_address_byte_count != 0 => {}
        (
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if matches!(declaration.semantic, MachineSemanticKind::PlacedScalarAccess(form) if !form.write && form.ordinary_semantic().is_some() && u16::from(form.bytes) == byte_count)
            && declaration.memory == crate::MachineMemoryEffect::ReadPointerV1
            && encoded.external_operand_reads == [0]
            && encoded.external_operand_writes == [1]
            && encoded.control == MachineEncodedControlEffect::FallThroughV1 => {}
        (
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if matches!(declaration.semantic, MachineSemanticKind::PlacedScalarAccess(form) if form.write && form.ordinary_semantic().is_some())
            && declaration.memory == crate::MachineMemoryEffect::WritePointerV1
            && encoded.external_operand_reads == [0, 1]
            && encoded.external_operand_writes.is_empty()
            && encoded.control == MachineEncodedControlEffect::FallThroughV1 => {}
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
                (MachineSemanticKind::LoadPacked3, 3)
                    | (MachineSemanticKind::LoadPacked5, 5)
                    | (MachineSemanticKind::LoadPacked6, 6)
                    | (MachineSemanticKind::LoadPacked7, 7)
                    | (MachineSemanticKind::Load8, 1)
                    | (MachineSemanticKind::Load16, 2)
                    | (MachineSemanticKind::Load32, 4)
                    | (MachineSemanticKind::Load64, 8)
            )
            && encoded.external_operand_reads.contains(&pointer_operand) => {}
        (
            MachineEncodedMemoryEffect::WriteFrameStorageV1 { byte_count: 8, .. },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::Store64
            && declaration.memory == crate::MachineMemoryEffect::WriteFrameStorageV1 => {}
        (
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if matches!(
            declaration.semantic,
            crate::MachineSemanticKind::Store | crate::MachineSemanticKind::StorePacked
        ) && declaration.memory == crate::MachineMemoryEffect::WritePointerV1
            && encoded.external_operand_reads == [0, 1]
            && (if declaration.semantic == crate::MachineSemanticKind::StorePacked {
                encoded.external_operand_writes == [2]
            } else {
                encoded.external_operand_writes.is_empty()
            }) => {}
        (
            MachineEncodedMemoryEffect::NoneV1,
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::TrappingIntegerV1,
        ) if has_policy_trap(declaration.semantic)
            && declaration.memory == crate::MachineMemoryEffect::NoneV1
            && declaration.trap == crate::MachineTrapBehavior::TrappingIntegerV1
            && encoded.control == MachineEncodedControlEffect::FallThroughOrTrapV1 => {}
        (
            MachineEncodedMemoryEffect::AtomicFenceV1,
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::NeverV1,
        ) if declaration.semantic == MachineSemanticKind::AtomicFence
            && declaration.memory == crate::MachineMemoryEffect::AtomicFenceV1
            && encoded == &crate::MachineEncodedEffects::atomic_fence_v1()
            && constraint.operands.is_empty() => {}
        (
            MachineEncodedMemoryEffect::AtomicPointerV1 {
                pointer_operand: 0,
                form,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::AtomicAccess(form)
            && declaration.memory == crate::MachineMemoryEffect::AtomicPointerV1
            && encoded.control
                == if architecture == target::Architecture::X86_64
                    && matches!(
                        form.operation,
                        crate::AtomicOperation::FetchAnd
                            | crate::AtomicOperation::FetchOr
                            | crate::AtomicOperation::FetchXor
                    )
                {
                    MachineEncodedControlEffect::AtomicRetryUntilCommitV1
                } else {
                    MachineEncodedControlEffect::FallThroughV1
                }
            && constraint.operands.len() == usize::from(form.operand_count())
            && encoded
                .external_operand_reads
                .iter()
                .copied()
                .eq(0..form.operation.input_count())
            && encoded
                .external_operand_writes
                .iter()
                .copied()
                .eq(form.observed_operand())
            && constraint
                .operands
                .iter()
                .enumerate()
                .all(|(index, operand)| {
                    if usize::from(operand.operand) != index || operand.tied_to.is_some() {
                        return false;
                    }
                    if Some(operand.operand) == form.observed_operand() {
                        // Allocatable outputs require early clobber. A retry
                        // may instead use fixed homes: all inputs must also
                        // be fixed and distinct, and target catalog replay
                        // checks their actual physical-unit separation.
                        operand.access == RegisterOperandAccess::Def
                            && (operand.early_clobber
                                || (encoded.control
                                    == MachineEncodedControlEffect::AtomicRetryUntilCommitV1
                                    && operand.fixed_view.is_some()
                                    && constraint.operands
                                        [..usize::from(form.operation.input_count())]
                                        .iter()
                                        .all(|input| {
                                            input.fixed_view.is_some()
                                                && input.fixed_view != operand.fixed_view
                                        })))
                    } else {
                        operand.access == RegisterOperandAccess::Use && !operand.early_clobber
                    }
                }) => {}
        // The fallthrough surface admits no hosted trap shape; hosted rules
        // must pass their own rows above instead of borrowing this one.
        (
            MachineEncodedMemoryEffect::NoneV1,
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::NeverV1
            | MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.memory == crate::MachineMemoryEffect::NoneV1 => {}
        _ => return Err(()),
    }
    Ok(())
}

fn validate_applicability(
    constraint: &RegisterInstructionConstraint,
    applicability: MachineAlternativeApplicability,
) -> Result<(), ()> {
    let operand = |number| constraint.operand(number);
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
        // Only a post-allocation rule can rewrite an already-homed
        // instruction onto a declared form; an earlier phase's rule has no
        // physical homes to guard against.
        MachineAlternativeApplicability::OptimizationSelected { optimization } => (optimization
            .execution_phase()
            == optimization_core::OptimizationExecutionPhase::PostAllocationMachine)
            .then_some(())
            .ok_or(()),
    }
}
