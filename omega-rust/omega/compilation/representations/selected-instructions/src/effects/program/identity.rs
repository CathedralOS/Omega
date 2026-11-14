/*
  Canonical SHA-256 content identity (`pre_allocation_machine_effect_identity`)
  for `PreAllocationMachineEffectPlan` and shared row encoders for instructions,
  ownership events, affine cleanup actions, provenance, and dense ID lists.
*/

mod instruction {
    use crate::{
        MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineMemoryEffect,
        MachineTrapBehavior, SelectedInstructionKind,
    };

    use crate::{
        InstructionMachineEffects, SaturatingOperation, saturating_family_tag, trapping_family_tag,
    };

    use super::provenance::encode_provenance;
    use super::values::encode_units;
    use crate::codec_bytes::encode_len;
    use register_model::encode_constraint_key;

    pub(super) fn encode_cfg_instruction(
        bytes: &mut Vec<u8>,
        instruction: &InstructionMachineEffects,
    ) {
        encode_ordinary_instruction(bytes, instruction);
    }

    pub(super) fn encode_ordinary_instruction(
        bytes: &mut Vec<u8>,
        instruction: &InstructionMachineEffects,
    ) {
        encode_common_fields(bytes, instruction);
        bytes.push(match instruction.memory {
            MachineMemoryEffect::CopyBytesV1 => 6,
            MachineMemoryEffect::AtomicFenceV1 => 15,
            MachineMemoryEffect::CatalogFenceV1 => 16,
            crate::MachineMemoryEffect::PrivateCallbackMaterializationV1 => 17,
            MachineMemoryEffect::AtomicPointerV1 => 8,
            MachineMemoryEffect::TemporaryStackStorageV1 => 9,
            MachineMemoryEffect::TaskRegisterActivationV1 => 18,
            MachineMemoryEffect::InstalledInterruptCpuCheckV1 => 20,
            MachineMemoryEffect::InstalledInterruptTaskCommitV1 => 21,
            MachineMemoryEffect::InstalledInterruptTablePublicationV1 => 22,

            MachineMemoryEffect::SegmentDescriptorReloadV1 => 19,
            MachineMemoryEffect::NoneV1 => 0,
            MachineMemoryEffect::ReadPointerV1 => 1,
            MachineMemoryEffect::HostedReadByteV1 => 5,
            MachineMemoryEffect::HostedWriteByteV1 => 3,
            MachineMemoryEffect::WriteFrameStorageV1 => 2,
            MachineMemoryEffect::ReadFrameStorageV1 => 7,
            MachineMemoryEffect::WritePointerV1 => 4,
            MachineMemoryEffect::ScanByteSequenceV1 => 10,
            MachineMemoryEffect::ReadEncodedScalarV1 => 13,
            MachineMemoryEffect::WriteEncodedScalarV1 => 14,
            MachineMemoryEffect::CompareByteSequencesV1 => 11,
            MachineMemoryEffect::ZeroBytesV1 => 12,
        });
        encode_effect_tail(bytes, instruction);
    }

    fn encode_effect_tail(bytes: &mut Vec<u8>, instruction: &InstructionMachineEffects) {
        bytes.push(match instruction.trap {
            MachineTrapBehavior::ExplicitCrashV1 => 5,
            MachineTrapBehavior::TrappingIntegerV1 => 6,
            MachineTrapBehavior::NeverV1 => 0,
            MachineTrapBehavior::HostedExitReturnedV1 => 3,
            MachineTrapBehavior::HostedReadFailureV1 => 4,
            MachineTrapBehavior::HostedWriteFailureV1 => 2,
            MachineTrapBehavior::MayArchitecturalFaultV1 => 1,
        });
        encode_barrier(bytes, instruction.barrier);
        match instruction.call {
            MachineCallEffect::NoneV1 => bytes.push(0),
            MachineCallEffect::SupervisorNormalReturnV1 => bytes.push(3),
            MachineCallEffect::InternalNoNormalV1 {
                pre_call_stack_alignment,
            } => {
                bytes.push(4);
                crate::codec_bytes::push_encoded(bytes, pre_call_stack_alignment.to_le_bytes());
            }
            MachineCallEffect::InternalNormalReturnV1 {
                pre_call_stack_alignment,
            } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, pre_call_stack_alignment.to_le_bytes());
            }
            MachineCallEffect::ExternalNormalReturnV1 {
                pre_call_stack_alignment,
            } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, pre_call_stack_alignment.to_le_bytes());
            }
        }
        bytes.push(match instruction.cleanup {
            MachineCleanupEffect::NoneV1 => 0,
        });
        encode_provenance_and_alternatives(bytes, instruction);
    }

    fn encode_common_fields(bytes: &mut Vec<u8>, instruction: &InstructionMachineEffects) {
        crate::codec_bytes::push_encoded(bytes, instruction.instruction.0.to_le_bytes());
        encode_kind(bytes, instruction.kind);
        encode_constraint_key(bytes, instruction.constraint);
        encode_units(bytes, &instruction.unit_uses);
        encode_units(bytes, &instruction.unit_defs);
        encode_units(bytes, &instruction.unit_clobbers);
    }

    fn encode_provenance_and_alternatives(
        bytes: &mut Vec<u8>,
        instruction: &InstructionMachineEffects,
    ) {
        encode_provenance(bytes, &instruction.provenance);
        encode_len(bytes, instruction.alternatives.len());
        for alternative in &instruction.alternatives {
            crate::encode_machine_alternative_identity(bytes, alternative);
        }
    }

    fn encode_barrier(bytes: &mut Vec<u8>, barrier: MachineBarrier) {
        bytes.push(match barrier {
            MachineBarrier::None => 0,
            MachineBarrier::ControlFlow => 1,
            MachineBarrier::ExternalEffect => 3,
            MachineBarrier::Call => 2,
        });
    }

    pub(crate) fn encode_kind(bytes: &mut Vec<u8>, kind: SelectedInstructionKind) {
        let tag = match kind {
            SelectedInstructionKind::Store { .. } => 24,
            SelectedInstructionKind::AddressOffset { .. } => 25,
            SelectedInstructionKind::CompareI64Zero => 0,
            SelectedInstructionKind::MaterializeI64 { .. } => 1,
            SelectedInstructionKind::CopyBytes => 59,
            SelectedInstructionKind::ScanByteSequence { .. } => 233,
            SelectedInstructionKind::LoadEncodedScalar => 236,
            SelectedInstructionKind::StoreEncodedScalar => 237,
            SelectedInstructionKind::CompareByteSequences => 234,
            SelectedInstructionKind::ZeroBytes => 235,
            SelectedInstructionKind::CopyI64 => 2,
            SelectedInstructionKind::BitwiseAndI64 => 51,
            SelectedInstructionKind::BitwiseXorI64 => 52,
            SelectedInstructionKind::ExactDivideU64 { .. } => 56,
            SelectedInstructionKind::WrappingRemainderI64 { .. } => 57,
            SelectedInstructionKind::WrappingAddI64 => 58,
            SelectedInstructionKind::SaturatingAdd { carrier } => {
                saturating_family_tag(SaturatingOperation::Add, carrier)
            }
            SelectedInstructionKind::SaturatingSubtract { carrier } => {
                saturating_family_tag(SaturatingOperation::Subtract, carrier)
            }
            SelectedInstructionKind::SaturatingDivide { carrier, .. } => {
                saturating_family_tag(SaturatingOperation::Divide, carrier)
            }
            SelectedInstructionKind::SaturatingRemainder { carrier, .. } => {
                saturating_family_tag(SaturatingOperation::Remainder, carrier)
            }
            SelectedInstructionKind::SaturatingMultiply { carrier } => {
                saturating_family_tag(SaturatingOperation::Multiply, carrier)
            }
            SelectedInstructionKind::TrappingInteger { form } => trapping_family_tag(form),
            SelectedInstructionKind::PortWrite { .. } => 239,
            SelectedInstructionKind::RegisterAccess { .. } => 247,
            SelectedInstructionKind::InstalledInterruptCpuCheck { .. } => 249,
            SelectedInstructionKind::InstalledInterruptTaskCommit { .. } => 250,
            SelectedInstructionKind::InstalledInterruptTablePublication { .. } => 251,

            SelectedInstructionKind::AtomicFence => 238,
            SelectedInstructionKind::AtomicAccess { form } => form.machine_tag(),
            SelectedInstructionKind::AccessorScalarAccess { form, .. } => form.machine_tag(),
            SelectedInstructionKind::NearestIeeeFloatArithmetic { form } => form.machine_tag(),
            SelectedInstructionKind::FloatResultPolicy { form } => form.machine_tag(),
            SelectedInstructionKind::Float32ToBits => 26,
            SelectedInstructionKind::Float64ToBits => 27,
            SelectedInstructionKind::BitsToFloat32 => 28,
            SelectedInstructionKind::BitsToFloat64 => 29,
            SelectedInstructionKind::ZeroExtendU8 => 15,
            SelectedInstructionKind::ZeroExtendU32 => 20,
            SelectedInstructionKind::ZeroExtendU16 => 37,
            SelectedInstructionKind::SignExtendI8 => 38,
            SelectedInstructionKind::SignExtendI16 => 39,
            SelectedInstructionKind::SignExtendI32 => 40,
            SelectedInstructionKind::MaterializeBooleanEqual => 41,
            SelectedInstructionKind::MaterializeBooleanU64LessThan => 42,
            SelectedInstructionKind::MaterializeBooleanI64LessThan => 43,
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => 44,
            SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => 45,
            SelectedInstructionKind::ExactAddI64 { .. } => 3,
            SelectedInstructionKind::ExactAddI64Immediate { .. } => 4,
            SelectedInstructionKind::ExactSubtractI64 { .. } => 5,
            SelectedInstructionKind::ConditionalBranchNonZero => 6,
            SelectedInstructionKind::ReturnScalar => 7,
            SelectedInstructionKind::CallAggregate { .. } => 35,
            SelectedInstructionKind::ReturnAggregate { .. } => 36,
            SelectedInstructionKind::ExactSubtractI64Immediate { .. } => 8,
            SelectedInstructionKind::ReturnUnit => 9,
            SelectedInstructionKind::CompareI64 => 10,
            SelectedInstructionKind::CompareI64Immediate { .. } => 53,
            SelectedInstructionKind::ConditionalBranchU64LessThan => 11,
            SelectedInstructionKind::ConditionalBranchI64LessThan => 12,
            SelectedInstructionKind::CallScalar { .. } => 13,
            SelectedInstructionKind::NormalizedForeignCall { .. } => 87,
            SelectedInstructionKind::ExactMultiplyI64 { .. } => 88,
            SelectedInstructionKind::ExactRemainderU64 { .. } => 89,
            SelectedInstructionKind::WrappingSubtractI64 => 90,
            SelectedInstructionKind::WrappingMultiplyI64 => 91,
            SelectedInstructionKind::WrappingDivideI64 { .. } => 92,
            SelectedInstructionKind::ExactDivideI64 { .. } => 112,
            SelectedInstructionKind::ExactRemainderI64 { .. } => 113,
            SelectedInstructionKind::BitwiseOrI64 => 93,
            SelectedInstructionKind::BitwiseNotI64 => 94,
            SelectedInstructionKind::SaveFloatingControl { .. } => 103,
            SelectedInstructionKind::RestoreFloatingControl { .. } => 104,
            SelectedInstructionKind::WrappingShiftLeftI64 => 105,
            SelectedInstructionKind::WrappingShiftRightI64 => 106,
            SelectedInstructionKind::WrappingShiftRightU64 => 107,
            SelectedInstructionKind::ExactShiftLeftI64 { .. } => 108,
            SelectedInstructionKind::ExactShiftRightI64 { .. } => 109,
            SelectedInstructionKind::ExactShiftRightU64 { .. } => 110,
            SelectedInstructionKind::Jump => 14,
            SelectedInstructionKind::Load64 { .. } => 16,
            SelectedInstructionKind::LoadPacked { .. } => 46,
            SelectedInstructionKind::StorePacked { .. } => 47,
            SelectedInstructionKind::Load8 { .. } => 33,
            SelectedInstructionKind::Load16 { .. } => 34,
            SelectedInstructionKind::Load32 { .. } => 30,
            SelectedInstructionKind::Crash => 111,
            SelectedInstructionKind::PrivilegedCrash => 252,
            SelectedInstructionKind::HostedExitProcessI32 => 31,
            SelectedInstructionKind::HostedReadByte { .. } => 32,
            SelectedInstructionKind::HostedWriteByteI32 { .. } => 23,
            SelectedInstructionKind::ByteViewAddress => 22,
            SelectedInstructionKind::Load8Indexed => 21,
            SelectedInstructionKind::Store64 { .. } => 17,
            SelectedInstructionKind::FrameAddress { .. } => 18,
            SelectedInstructionKind::InvokeNoNormal { .. } => 248,
            SelectedInstructionKind::CallUnit { .. } => 19,
            SelectedInstructionKind::ImageAddress { .. } => 232,
        };
        crate::kind_encoding::encode_kind(bytes, tag, kind);
    }

    #[cfg(test)]
    mod outgoing_slot_tests {
        use super::{SelectedInstructionKind, encode_kind};
        use crate::{FrameStorageSlotId, OutgoingArgumentSlotId, OutgoingArgumentSlotRole};
        use semantic_vocabulary::OperationId;

        #[test]
        fn outgoing_frame_roles_preserve_argument_bytes_and_use_a_distinct_copy_tag() {
            let argument = OutgoingArgumentSlotId {
                invocation_site: (OperationId::new(43).unwrap()).into(),
                argument_index: 7,
                role: OutgoingArgumentSlotRole::Argument,
            };
            let copy = OutgoingArgumentSlotId {
                role: OutgoingArgumentSlotRole::ValueCopy,
                ..argument
            };
            for (tag, argument_kind, copy_kind) in [
                (
                    17,
                    SelectedInstructionKind::Store64 {
                        slot: FrameStorageSlotId::Outgoing(argument),
                        byte_offset: 8,
                    },
                    SelectedInstructionKind::Store64 {
                        slot: FrameStorageSlotId::Outgoing(copy),
                        byte_offset: 8,
                    },
                ),
                (
                    18,
                    SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Outgoing(argument),
                        byte_offset: 8,
                    },
                    SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Outgoing(copy),
                        byte_offset: 8,
                    },
                ),
            ] {
                let mut legacy = vec![tag, 0, 0];
                crate::codec_bytes::push_encoded(&mut legacy, 43_u64.to_le_bytes());
                crate::codec_bytes::push_encoded(&mut legacy, 7_u32.to_le_bytes());
                crate::codec_bytes::push_encoded(&mut legacy, 8_u32.to_le_bytes());
                let mut encoded = Vec::new();
                encode_kind(&mut encoded, argument_kind);
                assert_eq!(encoded, legacy);
                encoded.clear();
                encode_kind(&mut encoded, copy_kind);
                legacy[1] = 3;
                assert_eq!(encoded, legacy);
            }
        }
    }

    #[cfg(test)]
    mod float_result_policy_tests {
        use super::encode_kind;
        use crate::{FloatResultPolicyForm, SelectedInstructionKind};

        #[test]
        fn float_result_policy_instruction_kind_pins_both_identity_bytes() {
            for form in FloatResultPolicyForm::ALL {
                let mut bytes = Vec::new();
                encode_kind(
                    &mut bytes,
                    SelectedInstructionKind::FloatResultPolicy { form },
                );
                assert_eq!(bytes, [231, form.ordinal()]);
            }
        }
    }

    #[cfg(test)]
    mod port_write_tests {
        use super::*;
        #[test]
        fn port_write_identity_retains_both_operands_and_an_append_only_tag() {
            for (port, value) in [(0, 0), (0x3f8, 0x41), (u16::MAX, u8::MAX)] {
                let mut bytes = Vec::new();
                encode_kind(
                    &mut bytes,
                    SelectedInstructionKind::PortWrite { port, value },
                );
                let [low, high] = port.to_le_bytes();
                assert_eq!(bytes, [239, low, high, value]);
            }
        }
    }
}
mod ownership {
    use optimization_unit::OwnershipEvent;
    use terminal_psi::{StructuralPathSegment, TerminalAffineCleanupAction};

    use super::values::encode_ids;
    use crate::codec_bytes::encode_len;
    use terminal_psi::ReferenceOriginProjection::Exact;

    pub fn encode_ownership(bytes: &mut Vec<u8>, ownership: &[OwnershipEvent]) {
        encode_len(bytes, ownership.len());
        for event in ownership {
            match event {
                OwnershipEvent::ClaimTransfer(claims) => {
                    bytes.push(1);
                    encode_ids(bytes, claims.iter().map(|id| id.get()));
                }
                OwnershipEvent::ClaimCompletion(receipts) => {
                    bytes.push(2);
                    encode_len(bytes, receipts.len());
                    for receipt in receipts {
                        bytes.extend_from_slice(&receipt.claim.get().to_le_bytes());
                        bytes.extend_from_slice(&receipt.argument_index.to_le_bytes());
                        encode_path(bytes, &receipt.path);
                    }
                }
                OwnershipEvent::Cleanup(actions) => {
                    bytes.push(3);
                    encode_len(bytes, actions.len());
                    for action in actions {
                        encode_cleanup(bytes, action);
                    }
                }
                OwnershipEvent::StructuralReturn(claims) => {
                    bytes.push(4);
                    encode_ids(bytes, claims.iter().map(|id| id.get()));
                }
                OwnershipEvent::CrashFrontier(claims) => {
                    bytes.push(5);
                    encode_ids(bytes, claims.iter().map(|id| id.get()));
                }
                OwnershipEvent::ExternalCompletionFrontier(claims) => {
                    bytes.push(6);
                    encode_ids(bytes, claims.iter().map(|id| id.get()));
                }
            }
        }
    }

    pub fn encode_cleanup(bytes: &mut Vec<u8>, action: &TerminalAffineCleanupAction) {
        match action {
            TerminalAffineCleanupAction::RetireHome(place) => {
                bytes.push(4);
                crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            }
            TerminalAffineCleanupAction::DiscardRoot(place) => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            }
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, discard.place.get().to_le_bytes());
                encode_path(bytes, &discard.path);
                crate::codec_bytes::push_encoded(
                    bytes,
                    discard.structural_type.get().to_le_bytes(),
                );
            }
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                bytes.push(if cleanup.retains_descendants { 5 } else { 3 });
                crate::codec_bytes::push_encoded(bytes, cleanup.place.get().to_le_bytes());
                encode_path(bytes, &cleanup.path);
                crate::codec_bytes::push_encoded(
                    bytes,
                    cleanup.structural_type.get().to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(
                    bytes,
                    cleanup.cleanup_machine.get().to_le_bytes(),
                );
                encode_ids(
                    bytes,
                    cleanup.consumed_claims.iter().map(|claim| claim.get()),
                );
                match cleanup.cleanup_receiver {
                    None => bytes.push(0),
                    Some(place) => {
                        bytes.push(1);
                        crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
                    }
                }
                encode_ids(
                    bytes,
                    cleanup.requirement_obligations.iter().map(|id| id.get()),
                );
            }
        }
    }

    pub fn encode_path(bytes: &mut Vec<u8>, path: &[StructuralPathSegment]) {
        encode_len(bytes, path.len());
        for segment in path {
            encode_projection(bytes, segment);
        }
    }

    pub fn encode_reference_origin_path(
        bytes: &mut Vec<u8>,
        path: &[terminal_psi::ReferenceOriginProjection],
    ) {
        encode_len(bytes, path.len());
        for projection in path {
            match projection {
                Exact(segment) => encode_projection(bytes, segment),
                terminal_psi::ReferenceOriginProjection::Element => bytes.push(9),
            }
        }
    }

    fn encode_projection(bytes: &mut Vec<u8>, segment: &StructuralPathSegment) {
        terminal_psi::structural_path_encoding::encode_structural_path_segment(
            bytes,
            segment,
            Vec::extend_from_slice,
            crate::selected_instructions::plan_identity::encode_scalar_type,
        );
    }
}
mod provenance {
    use crate::SelectedInstructionProvenance;
    use optimization_unit::PsiProvenance;

    use super::values::encode_ids;
    use crate::codec_bytes::encode_len;

    pub fn encode_provenance(bytes: &mut Vec<u8>, provenance: &SelectedInstructionProvenance) {
        encode_ids(bytes, provenance.operations.iter().map(|id| id.get()));
        encode_ids(bytes, provenance.values.iter().map(|id| id.get()));
        encode_ids(bytes, provenance.edges.iter().map(|id| id.get()));
        encode_ids(bytes, provenance.obligations.iter().map(|id| id.get()));
        encode_len(bytes, provenance.work.len());
        for settlement in &provenance.work {
            match settlement.site {
                PsiProvenance::Operation(operation) => {
                    bytes.push(0);
                    crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
                }
                PsiProvenance::Edge(edge) => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
                }
            }
            crate::codec_bytes::push_encoded(bytes, settlement.units.to_le_bytes());
        }
    }
}
mod values {
    use register_model::RegisterUnitId;

    pub(super) fn encode_units(bytes: &mut Vec<u8>, units: &[RegisterUnitId]) {
        crate::codec_bytes::encode_le_list(bytes, units, |unit| unit.0.to_le_bytes());
    }

    pub fn encode_ids(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u64>) {
        crate::codec_bytes::encode_len(bytes, values.len());
        for value in values {
            crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
        }
    }
}

use crate::{PreAllocationMachineEffectIdentity, PreAllocationMachineEffectPlan};

use crate::codec_bytes::encode_len;
use crate::digest_encoder::content_identity;
use instruction::encode_cfg_instruction;
pub(crate) use instruction::encode_kind;
pub use ownership::{encode_cleanup, encode_ownership, encode_path, encode_reference_origin_path};
pub use provenance::encode_provenance;
pub use values::encode_ids;

pub fn pre_allocation_machine_effect_identity(
    plan: &PreAllocationMachineEffectPlan,
) -> PreAllocationMachineEffectIdentity {
    identity_with_domain(plan, b"omega.terminal-preallocation-machine-effects.v36\0")
}

fn identity_with_domain(
    plan: &PreAllocationMachineEffectPlan,
    domain: &[u8],
) -> PreAllocationMachineEffectIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encode_terminal_pre_allocation_machine_effect_content(plan));
    PreAllocationMachineEffectIdentity::from_bytes(content_identity(&bytes))
}

pub(crate) fn encode_terminal_pre_allocation_machine_effect_content(
    plan: &PreAllocationMachineEffectPlan,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.work_schedule.marker().to_le_bytes());
    bytes.extend_from_slice(&plan.target.identity_bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.register_constraints.bytes());
    bytes.extend_from_slice(&plan.machine_effect_catalog.bytes());

    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        crate::codec_bytes::push_encoded(&mut bytes, function.machine.get().to_le_bytes());
        encode_len(&mut bytes, function.blocks.len());
        for block in &function.blocks {
            crate::codec_bytes::push_encoded(&mut bytes, block.block.0.to_le_bytes());
            encode_len(&mut bytes, block.instructions.len());
            for instruction in &block.instructions {
                encode_cfg_instruction(&mut bytes, instruction);
            }
        }
    }

    bytes
}

#[cfg(test)]
mod tests {
    use crate::{
        MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
        MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
        MachineEffectCatalogIdentity, MachineEncodedEffects, MachineLatencyKnowledge,
        MachineMemoryEffect, MachineSizeKnowledge, MachineTrapBehavior, SelectedBlockId,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlanIdentity,
        SelectedInstructionProvenance,
    };
    use optimization_core::OptimizationUnitIdentity;
    use register_model::{
        RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
        RegisterUnitId, TargetRegisterEnvironmentIdentity,
    };
    use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
    use target::NativeTarget;

    use crate::{
        BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects,
        PreAllocationMachineEffectIdentity, PreAllocationMachineEffectPlan,
    };

    use super::pre_allocation_machine_effect_identity;

    fn plan() -> PreAllocationMachineEffectPlan {
        let constraint = RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 4,
        };
        let mut plan = PreAllocationMachineEffectPlan {
            identity: PreAllocationMachineEffectIdentity::from_bytes([0; 32]),
            selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            target: NativeTarget::linux_x64(),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([3; 32]),
            register_constraints: RegisterConstraintCatalogIdentity::from_bytes([4; 32]),
            machine_effect_catalog: MachineEffectCatalogIdentity::from_bytes([5; 32]),
            functions: vec![FunctionMachineEffects {
                machine: MachineId::new(1).unwrap(),
                blocks: vec![BlockMachineEffects {
                    block: SelectedBlockId(0),
                    instructions: vec![InstructionMachineEffects {
                        instruction: SelectedInstructionId(0),
                        kind: SelectedInstructionKind::CompareI64Zero,
                        constraint,
                        unit_uses: vec![RegisterUnitId(0)],
                        unit_defs: vec![RegisterUnitId(1)],
                        unit_clobbers: Vec::new(),
                        memory: MachineMemoryEffect::NoneV1,
                        trap: MachineTrapBehavior::NeverV1,
                        barrier: MachineBarrier::None,
                        call: MachineCallEffect::NoneV1,
                        cleanup: MachineCleanupEffect::NoneV1,
                        provenance: SelectedInstructionProvenance::default(),
                        alternatives: vec![MachineAlternative {
                            key: MachineAlternativeKey {
                                family: MachineAlternativeFamily::CompareI64Zero,
                                variant: 0,
                            },
                            applicability: MachineAlternativeApplicability::Always,
                            size: MachineSizeKnowledge::ExactBytes(3),
                            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                            encoded: MachineEncodedEffects::fallthrough_v1(vec![0], vec![]),
                        }],
                    }],
                }],
            }],
        };
        plan.identity = pre_allocation_machine_effect_identity(&plan);
        plan
    }

    #[test]
    fn identity_binds_roots_effect_rows_provenance_and_alternatives() {
        let source = plan();
        let baseline = source.identity;
        assert_eq!(baseline, pre_allocation_machine_effect_identity(&source));

        let mut changed = source.clone();
        changed.selected = SelectedInstructionPlanIdentity::from_bytes([9; 32]);
        assert_ne!(baseline, pre_allocation_machine_effect_identity(&changed));
        let mut changed = source.clone();
        changed.functions[0].blocks[0].instructions[0]
            .unit_clobbers
            .push(RegisterUnitId(2));
        assert_ne!(baseline, pre_allocation_machine_effect_identity(&changed));
        let mut changed = source.clone();
        changed.functions[0].blocks[0].instructions[0].barrier = MachineBarrier::ControlFlow;
        assert_ne!(baseline, pre_allocation_machine_effect_identity(&changed));
        let mut changed = source.clone();
        changed.functions[0].blocks[0].instructions[0].alternatives[0].size =
            MachineSizeKnowledge::ExactBytes(4);
        assert_ne!(baseline, pre_allocation_machine_effect_identity(&changed));
    }
}
