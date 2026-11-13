/*
  Versioned wire persistence (`OMGMFX\0\0`) for `PreAllocationMachineEffectPlan`,
  including framing, instruction and alternative decoding, ownership and cleanup
  decoding, and shared value cursor primitives.
*/

mod cursor {
    use super::PreAllocationMachineEffectDecodeError;
    pub struct Cursor<'a> {
        bytes: &'a [u8],
        position: usize,
    }

    impl<'a> Cursor<'a> {
        pub const fn new(bytes: &'a [u8]) -> Self {
            Self { bytes, position: 0 }
        }

        pub fn take(
            &mut self,
            count: usize,
        ) -> Result<&'a [u8], PreAllocationMachineEffectDecodeError> {
            let end = self
                .position
                .checked_add(count)
                .ok_or(PreAllocationMachineEffectDecodeError::Truncated)?;
            let bytes = self
                .bytes
                .get(self.position..end)
                .ok_or(PreAllocationMachineEffectDecodeError::Truncated)?;
            self.position = end;
            Ok(bytes)
        }

        pub fn array<const N: usize>(
            &mut self,
        ) -> Result<[u8; N], PreAllocationMachineEffectDecodeError> {
            self.take(N)?
                .try_into()
                .map_err(|_| PreAllocationMachineEffectDecodeError::Truncated)
        }

        pub fn byte(&mut self) -> Result<u8, PreAllocationMachineEffectDecodeError> {
            Ok(self.take(1)?[0])
        }

        pub fn u16(&mut self) -> Result<u16, PreAllocationMachineEffectDecodeError> {
            Ok(u16::from_le_bytes(self.array()?))
        }

        pub fn u32(&mut self) -> Result<u32, PreAllocationMachineEffectDecodeError> {
            Ok(u32::from_le_bytes(self.array()?))
        }

        pub fn u64(&mut self) -> Result<u64, PreAllocationMachineEffectDecodeError> {
            Ok(u64::from_le_bytes(self.array()?))
        }

        pub fn u128(&mut self) -> Result<u128, PreAllocationMachineEffectDecodeError> {
            Ok(u128::from_le_bytes(self.array()?))
        }

        pub fn i128(&mut self) -> Result<i128, PreAllocationMachineEffectDecodeError> {
            Ok(i128::from_le_bytes(self.array()?))
        }

        pub fn length(&mut self) -> Result<usize, PreAllocationMachineEffectDecodeError> {
            usize::try_from(self.u64()?)
                .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)
        }

        pub fn remaining(&self) -> usize {
            self.bytes.len().saturating_sub(self.position)
        }
    }
}
mod error {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PreAllocationMachineEffectDecodeError {
        Truncated,
        WrongMagic,
        UnsupportedVersion(u32),
        InvalidField,
        InvalidIdentity,
        TrailingBytes,
    }

    impl std::fmt::Display for PreAllocationMachineEffectDecodeError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "invalid pre-allocation machine-effect artifact: {self:?}"
            )
        }
    }

    impl std::error::Error for PreAllocationMachineEffectDecodeError {}
}
mod v6 {

    use optimization_core::{AcceptedObligationFactIdentity, OptimizationUnitIdentity};

    use super::super::identity;
    use super::{Cursor, PreAllocationMachineEffectDecodeError};
    use crate::{
        MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
        MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
        MachineEffectCatalogIdentity, MachineEncodedControlEffect, MachineEncodedEffects,
        MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
        MachineLatencyKnowledge, MachineMemoryEffect, MachineSizeKnowledge, MachineTrapBehavior,
        SelectedBlockId, SelectedInstructionId, SelectedInstructionKind,
        SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
    };
    use optimization_unit::{OwnershipEvent, PsiProvenance, WorkSettlement};
    use register_model::{
        RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
        RegisterUnitId, TargetRegisterEnvironmentIdentity,
    };
    use semantic_vocabulary::{
        ClaimId, EdgeId, IntegerValue, MachineId, ObligationId, OperationId, PlaceId,
        StructuralTypeId, ValueId, WorkScheduleIdentity,
    };
    use target::{Architecture, NativeTarget, ObjectFormat};

    use crate::{
        BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects,
        PreAllocationMachineEffectIdentity, PreAllocationMachineEffectPlan,
        pre_allocation_machine_effect_identity,
    };

    mod framing {
        use super::{
            BlockMachineEffects, Cursor, FunctionMachineEffects, MAGIC,
            MachineEffectCatalogIdentity, MachineId, OptimizationUnitIdentity,
            PreAllocationMachineEffectDecodeError, PreAllocationMachineEffectIdentity,
            PreAllocationMachineEffectPlan, RegisterConstraintCatalogIdentity, SelectedBlockId,
            SelectedInstructionPlanIdentity, TargetRegisterEnvironmentIdentity, VERSION,
            WorkScheduleIdentity, decode_instruction, decode_target, identity,
            pre_allocation_machine_effect_identity,
        };
        pub(crate) fn encode_terminal_pre_allocation_machine_effect_plan(
            plan: &PreAllocationMachineEffectPlan,
        ) -> Vec<u8> {
            let content = identity::encode_terminal_pre_allocation_machine_effect_content(plan);
            let mut encoded = Vec::with_capacity(44 + content.len());
            encoded.extend_from_slice(MAGIC);
            crate::codec_bytes::push_encoded(&mut encoded, VERSION.to_le_bytes());
            encoded.extend_from_slice(&plan.identity.bytes());
            encoded.extend_from_slice(&content);
            encoded
        }

        pub(crate) fn decode_terminal_pre_allocation_machine_effect_plan(
            encoded: &[u8],
        ) -> Result<PreAllocationMachineEffectPlan, PreAllocationMachineEffectDecodeError> {
            let mut cursor = Cursor::new(encoded);
            if cursor.take(8)? != MAGIC {
                return Err(PreAllocationMachineEffectDecodeError::WrongMagic);
            }
            let version = cursor.u32()?;
            if version != VERSION {
                return Err(PreAllocationMachineEffectDecodeError::UnsupportedVersion(
                    version,
                ));
            }
            let identity = PreAllocationMachineEffectIdentity::from_bytes(cursor.array()?);
            let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
            let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
            let work_schedule = WorkScheduleIdentity::new(cursor.u32()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
            let target = decode_target(&mut cursor)?;
            let register_environment =
                TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
            let register_constraints =
                RegisterConstraintCatalogIdentity::from_bytes(cursor.array()?);
            let machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes(cursor.array()?);
            let function_count = cursor.length()?;
            let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
            for _ in 0..function_count {
                let machine = MachineId::new(cursor.u64()?)
                    .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
                let block_count = cursor.length()?;
                let mut blocks = Vec::with_capacity(block_count.min(cursor.remaining()));
                for _ in 0..block_count {
                    let block = SelectedBlockId(cursor.u32()?);
                    let instruction_count = cursor.length()?;
                    let mut instructions =
                        Vec::with_capacity(instruction_count.min(cursor.remaining()));
                    for _ in 0..instruction_count {
                        instructions.push(decode_instruction(&mut cursor, true, true, true)?);
                    }
                    blocks.push(BlockMachineEffects {
                        block,
                        instructions,
                    });
                }
                functions.push(FunctionMachineEffects { machine, blocks });
            }
            if cursor.remaining() != 0 {
                return Err(PreAllocationMachineEffectDecodeError::TrailingBytes);
            }
            let plan = PreAllocationMachineEffectPlan {
                identity,
                selected,
                optimization_unit,
                work_schedule,
                target,
                register_environment,
                register_constraints,
                machine_effect_catalog,
                functions,
            };
            let expected_identity = pre_allocation_machine_effect_identity(&plan);
            if plan.identity != expected_identity {
                return Err(PreAllocationMachineEffectDecodeError::InvalidIdentity);
            }
            Ok(plan)
        }
    }
    mod instruction {
        use super::{
            AcceptedObligationFactIdentity, Cursor, EdgeId, InstructionMachineEffects,
            IntegerValue, MachineAlternative, MachineAlternativeApplicability,
            MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier, MachineCallEffect,
            MachineCleanupEffect, MachineEncodedControlEffect, MachineEncodedEffects,
            MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
            MachineId, MachineLatencyKnowledge, MachineMemoryEffect, MachineSizeKnowledge,
            MachineTrapBehavior, ObligationId, OperationId, PreAllocationMachineEffectDecodeError,
            PsiProvenance, SelectedInstructionId, SelectedInstructionKind,
            SelectedInstructionProvenance, ValueId, WorkSettlement, decode_accepted_fact,
            decode_constraint_key, decode_ids, decode_obligation, decode_units,
        };
        use crate::FrameStorageSlotId::Outgoing;
        use crate::{
            SaturatingCarrier, SaturatingOperation, TrappingForm, saturating_family_tag,
            trapping_family_tag,
        };
        pub(super) fn decode_instruction(
            cursor: &mut Cursor<'_>,
            allow_i64_less_than: bool,
            allow_scalar_call: bool,
            allow_jump: bool,
        ) -> Result<InstructionMachineEffects, PreAllocationMachineEffectDecodeError> {
            let instruction = SelectedInstructionId(cursor.u32()?);
            let kind = decode_kind(cursor, allow_i64_less_than, allow_scalar_call, allow_jump)?;
            let constraint = decode_constraint_key(cursor)?;
            let unit_uses = decode_units(cursor)?;
            let unit_defs = decode_units(cursor)?;
            let unit_clobbers = decode_units(cursor)?;
            let memory = match cursor.byte()? {
                9 => MachineMemoryEffect::TemporaryStackStorageV1,
                18 => MachineMemoryEffect::TaskRegisterActivationV1,
                20 => MachineMemoryEffect::InstalledInterruptCpuCheckV1,
                21 => MachineMemoryEffect::InstalledInterruptTaskCommitV1,
                22 => MachineMemoryEffect::InstalledInterruptTablePublicationV1,

                19 => MachineMemoryEffect::SegmentDescriptorReloadV1,
                0 => MachineMemoryEffect::NoneV1,
                1 => MachineMemoryEffect::ReadPointerV1,
                5 => MachineMemoryEffect::HostedReadByteV1,
                3 => MachineMemoryEffect::HostedWriteByteV1,
                2 => MachineMemoryEffect::WriteFrameStorageV1,
                7 => MachineMemoryEffect::ReadFrameStorageV1,
                6 => MachineMemoryEffect::CopyBytesV1,
                10 => MachineMemoryEffect::ScanByteSequenceV1,
                13 => MachineMemoryEffect::ReadEncodedScalarV1,
                14 => MachineMemoryEffect::WriteEncodedScalarV1,
                11 => MachineMemoryEffect::CompareByteSequencesV1,
                12 => MachineMemoryEffect::ZeroBytesV1,
                4 => MachineMemoryEffect::WritePointerV1,
                15 => MachineMemoryEffect::AtomicFenceV1,
                16 => MachineMemoryEffect::CatalogFenceV1,
                17 => MachineMemoryEffect::PrivateCallbackMaterializationV1,
                8 => MachineMemoryEffect::AtomicPointerV1,
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            let trap = match cursor.byte()? {
                5 => MachineTrapBehavior::ExplicitCrashV1,
                6 => MachineTrapBehavior::TrappingIntegerV1,
                3 => MachineTrapBehavior::HostedExitReturnedV1,
                4 => MachineTrapBehavior::HostedReadFailureV1,
                2 => MachineTrapBehavior::HostedWriteFailureV1,
                0 => MachineTrapBehavior::NeverV1,
                1 if allow_scalar_call => MachineTrapBehavior::MayArchitecturalFaultV1,
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            let barrier = match cursor.byte()? {
                3 => MachineBarrier::ExternalEffect,
                0 => MachineBarrier::None,
                1 => MachineBarrier::ControlFlow,
                2 if allow_scalar_call => MachineBarrier::Call,
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            let call = match cursor.byte()? {
                0 => MachineCallEffect::NoneV1,
                3 if allow_scalar_call => MachineCallEffect::SupervisorNormalReturnV1,
                4 if allow_scalar_call => MachineCallEffect::InternalNoNormalV1 {
                    pre_call_stack_alignment: cursor.u16()?,
                },
                1 if allow_scalar_call => MachineCallEffect::InternalNormalReturnV1 {
                    pre_call_stack_alignment: cursor.u16()?,
                },
                2 if allow_scalar_call => MachineCallEffect::ExternalNormalReturnV1 {
                    pre_call_stack_alignment: cursor.u16()?,
                },
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            if cursor.byte()? != 0 {
                return Err(PreAllocationMachineEffectDecodeError::InvalidField);
            }
            let provenance = decode_provenance(cursor)?;
            let alternative_count = cursor.length()?;
            let mut alternatives = Vec::with_capacity(alternative_count.min(cursor.remaining()));
            for _ in 0..alternative_count {
                alternatives.push(decode_alternative_for_version(
                    cursor,
                    allow_i64_less_than,
                    allow_scalar_call,
                    allow_jump,
                )?);
            }
            Ok(InstructionMachineEffects {
                instruction,
                kind,
                constraint,
                unit_uses,
                unit_defs,
                unit_clobbers,
                memory,
                trap,
                barrier,
                call,
                cleanup: MachineCleanupEffect::NoneV1,
                provenance,
                alternatives,
            })
        }

        fn decode_kind(
            cursor: &mut Cursor<'_>,
            allow_i64_less_than: bool,
            allow_scalar_call: bool,
            allow_jump: bool,
        ) -> Result<SelectedInstructionKind, PreAllocationMachineEffectDecodeError> {
            macro_rules! obligated {
                ($name:ident) => {
                    SelectedInstructionKind::$name {
                        obligation: decode_obligation(cursor)?,
                        accepted_fact: decode_accepted_fact(cursor)?,
                    }
                };
            }

            Ok(match cursor.byte()? {
                crate::AccessorScalarForm::FAMILY_TAG => {
                    SelectedInstructionKind::AccessorScalarAccess {
                        form: crate::AccessorScalarForm::from_machine_payload(cursor.byte()?)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                        contract: cursor.array()?,
                        byte_offset: cursor.u32()?,
                    }
                }
                crate::FloatResultPolicyForm::FAMILY_TAG => {
                    SelectedInstructionKind::FloatResultPolicy {
                        form: crate::FloatResultPolicyForm::from_machine_payload(cursor.byte()?)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    }
                }
                crate::IeeeFloatArithmeticForm::FAMILY_TAG => {
                    SelectedInstructionKind::NearestIeeeFloatArithmetic {
                        form: crate::IeeeFloatArithmeticForm::from_machine_payload(cursor.byte()?)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    }
                }
                crate::AtomicForm::ONCE_FAMILY_TAG => SelectedInstructionKind::AtomicAccess {
                    form: crate::AtomicForm::from_once_payload(cursor.byte()?)
                        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                },
                tag if crate::AtomicForm::from_machine_tag(tag).is_some() => {
                    SelectedInstructionKind::AtomicAccess {
                        form: crate::AtomicForm::from_machine_tag(tag)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    }
                }
                24 => {
                    let byte_offset = cursor.u32()?;
                    let byte_size = cursor.byte()?;
                    if !matches!(byte_size, 1 | 2 | 4 | 8) {
                        return Err(PreAllocationMachineEffectDecodeError::InvalidField);
                    }
                    SelectedInstructionKind::Store {
                        byte_offset,
                        byte_size,
                    }
                }
                25 => SelectedInstructionKind::AddressOffset {
                    byte_offset: cursor.u32()?,
                },
                0 => SelectedInstructionKind::CompareI64Zero,
                1 => SelectedInstructionKind::MaterializeI64 {
                    value: decode_integer(cursor)?,
                },
                59 => SelectedInstructionKind::CopyBytes,
                233 => {
                    let mask = cursor.byte()?;
                    SelectedInstructionKind::ScanByteSequence {
                        predicate: terminal_psi::ByteSequencePredicate::ALL
                            .into_iter()
                            .find(|predicate| predicate.mask_bit() == mask)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    }
                }
                234 => SelectedInstructionKind::CompareByteSequences,
                236 => SelectedInstructionKind::LoadEncodedScalar,
                237 => SelectedInstructionKind::StoreEncodedScalar,
                239 => SelectedInstructionKind::PortWrite {
                    port: cursor.u16()?,
                    value: cursor.byte()?,
                },
                247 => SelectedInstructionKind::RegisterAccess {
                    operation: crate::RegisterOperation::from_catalog_tag(cursor.u16()?)
                        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                },
                249 => SelectedInstructionKind::InstalledInterruptCpuCheck {
                    gdt_bytes: cursor.u32()?,
                },
                250 => SelectedInstructionKind::InstalledInterruptTaskCommit {
                    gdt_bytes: cursor.u32()?,
                },
                251 => SelectedInstructionKind::InstalledInterruptTablePublication {
                    table_bytes: cursor.u32()?,
                },
                238 => SelectedInstructionKind::AtomicFence,
                235 => SelectedInstructionKind::ZeroBytes,
                2 => SelectedInstructionKind::CopyI64,
                26 => SelectedInstructionKind::Float32ToBits,
                27 => SelectedInstructionKind::Float64ToBits,
                28 => SelectedInstructionKind::BitsToFloat32,
                29 => SelectedInstructionKind::BitsToFloat64,
                15 => SelectedInstructionKind::ZeroExtendU8,
                20 => SelectedInstructionKind::ZeroExtendU32,
                37 => SelectedInstructionKind::ZeroExtendU16,
                38 => SelectedInstructionKind::SignExtendI8,
                39 => SelectedInstructionKind::SignExtendI16,
                40 => SelectedInstructionKind::SignExtendI32,
                41 => SelectedInstructionKind::MaterializeBooleanEqual,
                42 => SelectedInstructionKind::MaterializeBooleanU64LessThan,
                43 => SelectedInstructionKind::MaterializeBooleanI64LessThan,
                44 => SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                45 => SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
                32 => SelectedInstructionKind::HostedReadByte {
                    slot: decode_local_storage_slot(cursor)?,
                },
                23 => SelectedInstructionKind::HostedWriteByteI32 {
                    slot: decode_local_storage_slot(cursor)?,
                },
                22 => SelectedInstructionKind::ByteViewAddress,
                21 => SelectedInstructionKind::Load8Indexed,
                16 => SelectedInstructionKind::Load64 {
                    byte_offset: cursor.u32()?,
                },
                tag @ (46 | 47) => {
                    let byte_offset = cursor.u32()?;
                    let width = crate::PackedByteWidth::from_byte_size(cursor.byte()?)
                        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
                    if tag == 46 {
                        SelectedInstructionKind::LoadPacked { byte_offset, width }
                    } else {
                        SelectedInstructionKind::StorePacked { byte_offset, width }
                    }
                }
                33 => SelectedInstructionKind::Load8 {
                    byte_offset: cursor.u32()?,
                },
                34 => SelectedInstructionKind::Load16 {
                    byte_offset: cursor.u32()?,
                },
                30 => SelectedInstructionKind::Load32 {
                    byte_offset: cursor.u32()?,
                },
                tag @ (17 | 18) => {
                    let slot = match cursor.byte()? {
                        4 => crate::FrameStorageSlotId::ActivationBase,
                        2 => crate::FrameStorageSlotId::Incoming {
                            parameter_index: cursor.u32()?,
                            abi_stack_byte_offset: cursor.u32()?,
                        },
                        slot_tag @ (0 | 3) => Outgoing(crate::OutgoingArgumentSlotId {
                            role: if slot_tag == 0 {
                                crate::OutgoingArgumentSlotRole::Argument
                            } else {
                                crate::OutgoingArgumentSlotRole::ValueCopy
                            },
                            invocation_site:
                                semantic_vocabulary::InvocationSite::from_tagged_identity(
                                    cursor.byte()?,
                                    cursor.u64()?,
                                )
                                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                            argument_index: cursor.u32()?,
                        }),
                        1 => crate::FrameStorageSlotId::Local(decode_local_storage_slot(cursor)?),
                        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
                    };
                    let byte_offset = cursor.u32()?;
                    if slot == crate::FrameStorageSlotId::ActivationBase
                        && (tag != 18 || byte_offset != 0)
                    {
                        return Err(PreAllocationMachineEffectDecodeError::InvalidField);
                    }
                    if tag == 17 {
                        SelectedInstructionKind::Store64 { slot, byte_offset }
                    } else {
                        SelectedInstructionKind::FrameAddress { slot, byte_offset }
                    }
                }
                248 => SelectedInstructionKind::InvokeNoNormal {
                    target: decode_call_target(cursor)?,
                },
                19 => SelectedInstructionKind::CallUnit {
                    target: decode_call_target(cursor)?,
                },
                51 => SelectedInstructionKind::BitwiseAndI64,
                52 => SelectedInstructionKind::BitwiseXorI64,
                58 => SelectedInstructionKind::WrappingAddI64,
                56 => obligated!(ExactDivideU64),
                57 => SelectedInstructionKind::WrappingRemainderI64 {
                    obligation: decode_obligation(cursor)?,
                    accepted_fact: decode_accepted_fact(cursor)?,
                },
                88 => obligated!(ExactMultiplyI64),
                89 => obligated!(ExactRemainderU64),
                90 => SelectedInstructionKind::WrappingSubtractI64,
                91 => SelectedInstructionKind::WrappingMultiplyI64,
                92 => SelectedInstructionKind::WrappingDivideI64 {
                    obligation: decode_obligation(cursor)?,
                    accepted_fact: decode_accepted_fact(cursor)?,
                },
                112 => obligated!(ExactDivideI64),
                113 => obligated!(ExactRemainderI64),
                93 => SelectedInstructionKind::BitwiseOrI64,
                94 => SelectedInstructionKind::BitwiseNotI64,
                103 => SelectedInstructionKind::SaveFloatingControl {
                    slot: decode_local_storage_slot(cursor)?,
                },
                104 => SelectedInstructionKind::RestoreFloatingControl {
                    slot: decode_local_storage_slot(cursor)?,
                },
                105 => SelectedInstructionKind::WrappingShiftLeftI64,
                106 => SelectedInstructionKind::WrappingShiftRightI64,
                107 => SelectedInstructionKind::WrappingShiftRightU64,
                108 => obligated!(ExactShiftLeftI64),
                109 => obligated!(ExactShiftRightI64),
                110 => obligated!(ExactShiftRightU64),
                tag if trapping_form(tag).is_some() => SelectedInstructionKind::TrappingInteger {
                    form: trapping_form(tag).unwrap(),
                },
                tag if saturating_kind(tag).is_some() => {
                    let (operation, carrier) = saturating_kind(tag).unwrap();
                    match operation {
                        SaturatingOperation::Add => {
                            SelectedInstructionKind::SaturatingAdd { carrier }
                        }
                        SaturatingOperation::Subtract => {
                            SelectedInstructionKind::SaturatingSubtract { carrier }
                        }
                        SaturatingOperation::Multiply => {
                            SelectedInstructionKind::SaturatingMultiply { carrier }
                        }
                        SaturatingOperation::Divide | SaturatingOperation::Remainder => {
                            let obligation = decode_obligation(cursor)?;
                            let accepted_fact =
                                AcceptedObligationFactIdentity::from_bytes(cursor.array()?);
                            match operation {
                                SaturatingOperation::Divide => {
                                    SelectedInstructionKind::SaturatingDivide {
                                        carrier,
                                        obligation,
                                        accepted_fact,
                                    }
                                }
                                _ => SelectedInstructionKind::SaturatingRemainder {
                                    carrier,
                                    obligation,
                                    accepted_fact,
                                },
                            }
                        }
                    }
                }
                3 => obligated!(ExactAddI64),
                4 => SelectedInstructionKind::ExactAddI64Immediate {
                    immediate: decode_integer(cursor)?,
                    obligation: decode_obligation(cursor)?,
                    accepted_fact: decode_accepted_fact(cursor)?,
                },
                5 => obligated!(ExactSubtractI64),
                6 => SelectedInstructionKind::ConditionalBranchNonZero,
                111 => SelectedInstructionKind::Crash,
                252 => SelectedInstructionKind::PrivilegedCrash,
                31 => SelectedInstructionKind::HostedExitProcessI32,
                7 => SelectedInstructionKind::ReturnScalar,
                36 => SelectedInstructionKind::ReturnAggregate {
                    fragment_count: cursor.byte()?,
                },
                35 if allow_scalar_call => SelectedInstructionKind::CallAggregate {
                    target: decode_call_target(cursor)?,
                },
                8 => SelectedInstructionKind::ExactSubtractI64Immediate {
                    immediate: decode_integer(cursor)?,
                    obligation: decode_obligation(cursor)?,
                    accepted_fact: decode_accepted_fact(cursor)?,
                },
                9 => SelectedInstructionKind::ReturnUnit,
                10 => SelectedInstructionKind::CompareI64,
                53 => SelectedInstructionKind::CompareI64Immediate {
                    immediate: decode_integer(cursor)?,
                },
                11 => SelectedInstructionKind::ConditionalBranchU64LessThan,
                12 if allow_i64_less_than => SelectedInstructionKind::ConditionalBranchI64LessThan,
                14 if allow_jump => SelectedInstructionKind::Jump,
                13 if allow_scalar_call => SelectedInstructionKind::CallScalar {
                    target: decode_call_target(cursor)?,
                },
                232 => SelectedInstructionKind::ImageAddress {
                    reference: match cursor.byte()? {
                        5 => crate::SelectedImageAddressReference::MachineDescriptor {
                            descriptor: semantic_vocabulary::MachineDescriptorId::new(cursor.u64()?)
                                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                        },
                        4 => crate::SelectedImageAddressReference::PrivateFunction {
                            identity: function_identity::MachineFunctionIdentity::from_bytes(
                                cursor.array()?,
                            )
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                        },
                        3 => crate::SelectedImageAddressReference::RetainedEntryStorageStart,
                        0 => crate::SelectedImageAddressReference::ByteLiteral {
                            operation: OperationId::new(cursor.u64()?)
                                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                            place: semantic_vocabulary::PlaceId::new(cursor.u64()?)
                                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                        },
                        2 => crate::SelectedImageAddressReference::NativeCodeLayout {
                            machine: MachineId::new(cursor.u64()?)
                                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                        },
                        1 => crate::SelectedImageAddressReference::ImplementationTable {
                            table: cursor.u32()?,
                        },
                        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
                    },
                },
                87 => SelectedInstructionKind::NormalizedForeignCall {
                    boundary: semantic_vocabulary::BoundaryMachineId::new(cursor.u64()?)
                        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    ordinal: cursor.u32()?,
                },
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            })
        }

        fn decode_integer(
            cursor: &mut Cursor<'_>,
        ) -> Result<IntegerValue, PreAllocationMachineEffectDecodeError> {
            match cursor.byte()? {
                0 => Ok(IntegerValue::Signed(cursor.i128()?)),
                1 => Ok(IntegerValue::Unsigned(cursor.u128()?)),
                _ => Err(PreAllocationMachineEffectDecodeError::InvalidField),
            }
        }

        pub fn decode_local_storage_slot(
            cursor: &mut Cursor<'_>,
        ) -> Result<crate::LocalStorageSlotId, PreAllocationMachineEffectDecodeError> {
            let tag = cursor.byte()?;
            if tag == 8 || tag == 9 {
                let edge = EdgeId::new(cursor.u64()?)
                    .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
                return Ok(if tag == 8 {
                    crate::LocalStorageSlotId::InvocationResult { edge }
                } else {
                    crate::LocalStorageSlotId::InvocationArgument {
                        edge,
                        argument_index: cursor.u32()?,
                    }
                });
            }
            if tag == 6 {
                return Ok(crate::LocalStorageSlotId::EdgeSnapshot {
                    edge: semantic_vocabulary::EdgeId::new(cursor.u64()?)
                        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    binding_index: cursor.u32()?,
                });
            }
            if tag == 4 {
                return Ok(crate::LocalStorageSlotId::StructuralParameter {
                    place: semantic_vocabulary::PlaceId::new(cursor.u64()?)
                        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                });
            }
            if tag == 3 {
                return Ok(crate::LocalStorageSlotId::StructuralBlockParameter {
                    block: semantic_vocabulary::BlockId::new(cursor.u64()?)
                        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    place: semantic_vocabulary::PlaceId::new(cursor.u64()?)
                        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                });
            }
            if tag == 2 {
                return Ok(crate::LocalStorageSlotId::Spill {
                    register: crate::VirtualRegisterId(cursor.u32()?),
                });
            }
            let operation = OperationId::new(cursor.u64()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
            match tag {
                0 => Ok(crate::LocalStorageSlotId::Structural {
                    operation,
                    place: semantic_vocabulary::PlaceId::new(cursor.u64()?)
                        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                }),
                1 => Ok(crate::LocalStorageSlotId::Boundary { operation }),
                7 => Ok(crate::LocalStorageSlotId::BoundaryResult { operation }),
                5 => Ok(crate::LocalStorageSlotId::StructuralCallArgument {
                    operation,
                    argument_index: cursor.u32()?,
                }),
                _ => Err(PreAllocationMachineEffectDecodeError::InvalidField),
            }
        }

        pub fn decode_provenance(
            cursor: &mut Cursor<'_>,
        ) -> Result<SelectedInstructionProvenance, PreAllocationMachineEffectDecodeError> {
            let operations = decode_ids(cursor, OperationId::new)?;
            let values = decode_ids(cursor, ValueId::new)?;
            let edges = decode_ids(cursor, EdgeId::new)?;
            let obligations = decode_ids(cursor, ObligationId::new)?;
            let work_count = cursor.length()?;
            let mut work = Vec::with_capacity(work_count.min(cursor.remaining()));
            for _ in 0..work_count {
                let site_tag = cursor.byte()?;
                let raw = cursor.u64()?;
                let site = match site_tag {
                    0 => PsiProvenance::Operation(
                        OperationId::new(raw)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    ),
                    1 => PsiProvenance::Edge(
                        EdgeId::new(raw)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    ),
                    _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
                };
                work.push(WorkSettlement {
                    site,
                    units: cursor.u64()?,
                });
            }
            let provenance = SelectedInstructionProvenance {
                operations,
                values,
                edges,
                obligations,
                work,
            };
            if !provenance.work_settles_only_claimed_sites() {
                return Err(PreAllocationMachineEffectDecodeError::InvalidField);
            }
            Ok(provenance)
        }

        pub fn decode_alternative(
            cursor: &mut Cursor<'_>,
        ) -> Result<MachineAlternative, PreAllocationMachineEffectDecodeError> {
            decode_alternative_for_version(cursor, true, true, true)
        }

        fn decode_alternative_for_version(
            cursor: &mut Cursor<'_>,
            allow_i64_less_than: bool,
            allow_scalar_call: bool,
            allow_jump: bool,
        ) -> Result<MachineAlternative, PreAllocationMachineEffectDecodeError> {
            let tag = cursor.u16()?;
            let atomic = u8::try_from(tag)
                .ok()
                .and_then(crate::AtomicForm::from_machine_tag);
            let trapping = u8::try_from(tag).ok().and_then(trapping_form);
            let saturating = u8::try_from(tag).ok().and_then(saturating_kind);
            let family = match tag {
                tag if tag == u16::from(crate::AccessorScalarForm::FAMILY_TAG) => {
                    MachineAlternativeFamily::AccessorScalarAccess(
                        crate::AccessorScalarForm::from_machine_payload(cursor.byte()?)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    )
                }
                tag if tag == u16::from(crate::FloatResultPolicyForm::FAMILY_TAG) => {
                    MachineAlternativeFamily::FloatResultPolicy(
                        crate::FloatResultPolicyForm::from_machine_payload(cursor.byte()?)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    )
                }
                tag if tag == u16::from(crate::IeeeFloatArithmeticForm::FAMILY_TAG) => {
                    MachineAlternativeFamily::NearestIeeeFloatArithmetic(
                        crate::IeeeFloatArithmeticForm::from_machine_payload(cursor.byte()?)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    )
                }
                tag if tag == u16::from(crate::AtomicForm::ONCE_FAMILY_TAG) => {
                    MachineAlternativeFamily::AtomicAccess(
                        crate::AtomicForm::from_once_payload(cursor.byte()?)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    )
                }
                _ if atomic.is_some() => MachineAlternativeFamily::AtomicAccess(atomic.unwrap()),
                0 => MachineAlternativeFamily::CompareI64Zero,
                1 => MachineAlternativeFamily::MaterializeI64,
                59 => MachineAlternativeFamily::CopyBytes,
                233 => MachineAlternativeFamily::ScanByteSequence,
                234 => MachineAlternativeFamily::CompareByteSequences,
                236 => MachineAlternativeFamily::LoadEncodedScalar,
                237 => MachineAlternativeFamily::StoreEncodedScalar,
                239 => MachineAlternativeFamily::PortWrite,
                247 => MachineAlternativeFamily::SystemRegisterRead,
                248 => MachineAlternativeFamily::SystemRegisterWrite,
                249 => MachineAlternativeFamily::PortRegisterRead,
                250 => MachineAlternativeFamily::PortRegisterWrite,
                251 => MachineAlternativeFamily::ModelSpecificRead,
                252 => MachineAlternativeFamily::ModelSpecificWrite,
                253 => MachineAlternativeFamily::ProcessorHalt,
                259 => MachineAlternativeFamily::GlobalDescriptorTableLoad,
                260 => MachineAlternativeFamily::GlobalDescriptorTableStore,
                261 => MachineAlternativeFamily::TaskRegisterActivate,
                262 => MachineAlternativeFamily::TaskRegisterRead,
                266 => MachineAlternativeFamily::InstalledInterruptCpuCheck,
                267 => MachineAlternativeFamily::InstalledInterruptTaskCommit,
                268 => MachineAlternativeFamily::InstalledInterruptTablePublication,

                263 => MachineAlternativeFamily::CodeSegmentReload,
                264 => MachineAlternativeFamily::DataSegmentReload,
                265 => MachineAlternativeFamily::SegmentSelectorRead,
                254 => MachineAlternativeFamily::ControlRegisterRead,
                255 => MachineAlternativeFamily::ControlRegisterWrite,
                256 => MachineAlternativeFamily::InterruptControl,
                257 => MachineAlternativeFamily::MemoryFence,

                238 => MachineAlternativeFamily::AtomicFence,
                235 => MachineAlternativeFamily::ZeroBytes,
                2 => MachineAlternativeFamily::CopyI64,
                26 => MachineAlternativeFamily::Float32ToBits,
                27 => MachineAlternativeFamily::Float64ToBits,
                28 => MachineAlternativeFamily::BitsToFloat32,
                29 => MachineAlternativeFamily::BitsToFloat64,
                15 => MachineAlternativeFamily::ZeroExtendU8,
                20 => MachineAlternativeFamily::ZeroExtendU32,
                37 => MachineAlternativeFamily::ZeroExtendU16,
                38 => MachineAlternativeFamily::SignExtendI8,
                39 => MachineAlternativeFamily::SignExtendI16,
                40 => MachineAlternativeFamily::SignExtendI32,
                41 => MachineAlternativeFamily::MaterializeBooleanEqual,
                42 => MachineAlternativeFamily::MaterializeBooleanU64LessThan,
                43 => MachineAlternativeFamily::MaterializeBooleanI64LessThan,
                44 => MachineAlternativeFamily::MaterializeBooleanU64LessOrEqual,
                45 => MachineAlternativeFamily::MaterializeBooleanI64LessOrEqual,

                111 => MachineAlternativeFamily::Crash,
                269 => MachineAlternativeFamily::PrivilegedCrash,
                31 => MachineAlternativeFamily::HostedExitProcessI32,
                32 => MachineAlternativeFamily::HostedReadByte,
                23 => MachineAlternativeFamily::HostedWriteByteI32,
                22 => MachineAlternativeFamily::ByteViewAddress,
                21 => MachineAlternativeFamily::Load8Indexed,
                16 => MachineAlternativeFamily::Load64,
                46 => MachineAlternativeFamily::LoadPacked3,
                47 => MachineAlternativeFamily::LoadPacked5,
                48 => MachineAlternativeFamily::LoadPacked6,
                49 => MachineAlternativeFamily::LoadPacked7,
                50 => MachineAlternativeFamily::StorePacked,
                33 => MachineAlternativeFamily::Load8,
                34 => MachineAlternativeFamily::Load16,
                30 => MachineAlternativeFamily::Load32,
                24 => MachineAlternativeFamily::Store,
                25 => MachineAlternativeFamily::AddressOffset,
                17 => MachineAlternativeFamily::Store64,
                18 => MachineAlternativeFamily::FrameAddress,
                232 => MachineAlternativeFamily::ImageAddress,
                258 => MachineAlternativeFamily::InvokeNoNormal,
                19 => MachineAlternativeFamily::CallUnit,
                3 => MachineAlternativeFamily::ExactAddI64,
                51 => MachineAlternativeFamily::BitwiseAndI64,
                52 => MachineAlternativeFamily::BitwiseXorI64,
                56 => MachineAlternativeFamily::ExactDivideU64,
                57 => MachineAlternativeFamily::WrappingRemainderI64,
                58 => MachineAlternativeFamily::WrappingAddI64,
                88 => MachineAlternativeFamily::ExactMultiplyI64,
                89 => MachineAlternativeFamily::ExactRemainderU64,
                90 => MachineAlternativeFamily::WrappingSubtractI64,
                91 => MachineAlternativeFamily::WrappingMultiplyI64,
                92 => MachineAlternativeFamily::WrappingDivideI64,
                112 => MachineAlternativeFamily::ExactDivideI64,
                113 => MachineAlternativeFamily::ExactRemainderI64,
                93 => MachineAlternativeFamily::BitwiseOrI64,
                94 => MachineAlternativeFamily::BitwiseNotI64,
                103 => MachineAlternativeFamily::SaveFloatingControl,
                104 => MachineAlternativeFamily::RestoreFloatingControl,
                105 => MachineAlternativeFamily::WrappingShiftLeftI64,
                106 => MachineAlternativeFamily::WrappingShiftRightI64,
                107 => MachineAlternativeFamily::WrappingShiftRightU64,
                108 => MachineAlternativeFamily::ExactShiftLeftI64,
                109 => MachineAlternativeFamily::ExactShiftRightI64,
                110 => MachineAlternativeFamily::ExactShiftRightU64,
                _ if trapping.is_some() => {
                    MachineAlternativeFamily::TrappingInteger(trapping.unwrap())
                }
                _ if saturating.is_some() => match saturating.unwrap() {
                    (SaturatingOperation::Add, carrier) => {
                        MachineAlternativeFamily::SaturatingAdd(carrier)
                    }
                    (SaturatingOperation::Subtract, carrier) => {
                        MachineAlternativeFamily::SaturatingSubtract(carrier)
                    }
                    (SaturatingOperation::Divide, carrier) => {
                        MachineAlternativeFamily::SaturatingDivide(carrier)
                    }
                    (SaturatingOperation::Remainder, carrier) => {
                        MachineAlternativeFamily::SaturatingRemainder(carrier)
                    }
                    (SaturatingOperation::Multiply, carrier) => {
                        MachineAlternativeFamily::SaturatingMultiply(carrier)
                    }
                },
                4 => MachineAlternativeFamily::ExactAddI64Immediate,
                5 => MachineAlternativeFamily::ExactSubtractI64,
                6 => MachineAlternativeFamily::ConditionalBranchNonZero,
                7 => MachineAlternativeFamily::ReturnScalar,
                35 if allow_scalar_call => MachineAlternativeFamily::CallAggregate,
                36 => MachineAlternativeFamily::ReturnAggregate,
                8 => MachineAlternativeFamily::ExactSubtractI64Immediate,
                9 => MachineAlternativeFamily::ReturnUnit,
                10 => MachineAlternativeFamily::CompareI64,
                53 => MachineAlternativeFamily::CompareI64Immediate,
                11 => MachineAlternativeFamily::ConditionalBranchU64LessThan,
                12 if allow_i64_less_than => MachineAlternativeFamily::ConditionalBranchI64LessThan,
                13 if allow_scalar_call => MachineAlternativeFamily::CallScalar,
                87 if allow_scalar_call => MachineAlternativeFamily::NormalizedForeignCall,
                14 if allow_jump => MachineAlternativeFamily::Jump,
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            let key = MachineAlternativeKey {
                family,
                variant: cursor.u32()?,
            };
            let applicability = match cursor.byte()? {
                0 => MachineAlternativeApplicability::Always,
                1 => MachineAlternativeApplicability::ResultAliasesOperand {
                    result: cursor.u16()?,
                    operand: cursor.u16()?,
                },
                2 => MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result: cursor.u16()?,
                    aliased_operand: cursor.u16()?,
                    distinct_operand: cursor.u16()?,
                },
                3 => MachineAlternativeApplicability::ResultAliasesOperands {
                    result: cursor.u16()?,
                    left: cursor.u16()?,
                    right: cursor.u16()?,
                },
                4 => MachineAlternativeApplicability::ResultDistinctFromOperands {
                    result: cursor.u16()?,
                    left: cursor.u16()?,
                    right: cursor.u16()?,
                },
                5 => MachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
                    left: cursor.u16()?,
                    right: cursor.u16()?,
                    excluded_view: register_model::RegisterViewId(cursor.u16()?),
                },
                6 => {
                    let tag = cursor.byte()?;
                    let optimization = optimization_core::Optimization::ALL
                        .into_iter()
                        .find(|optimization| *optimization as u8 == tag)
                        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
                    MachineAlternativeApplicability::OptimizationSelected { optimization }
                }
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            let size = match cursor.byte()? {
                0 => MachineSizeKnowledge::ExactBytes(cursor.u16()?),
                1 => {
                    let minimum_bytes = cursor.u16()?;
                    let maximum_bytes = match cursor.byte()? {
                        0 => None,
                        1 => Some(cursor.u16()?),
                        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
                    };
                    MachineSizeKnowledge::EncoderResolved {
                        minimum_bytes,
                        maximum_bytes,
                    }
                }
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            if cursor.byte()? != 0 {
                return Err(PreAllocationMachineEffectDecodeError::InvalidField);
            }
            let encoded = decode_encoded_effects(cursor, allow_scalar_call, allow_jump)?;
            Ok(MachineAlternative {
                key,
                applicability,
                size,
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded,
            })
        }

        fn decode_encoded_effects(
            cursor: &mut Cursor<'_>,
            allow_scalar_call: bool,
            allow_jump: bool,
        ) -> Result<MachineEncodedEffects, PreAllocationMachineEffectDecodeError> {
            let external_operand_reads = decode_u16s(cursor)?;
            let external_operand_writes = decode_u16s(cursor)?;
            let implicit_unit_uses = decode_units(cursor)?;
            let implicit_unit_defs = decode_units(cursor)?;
            let implicit_unit_clobbers = decode_units(cursor)?;
            let memory = match cursor.byte()? {
                19 if allow_scalar_call => MachineEncodedMemoryEffect::ReadTableFieldV1 {
                    pointer_operand: cursor.u16()?,
                    byte_offset: cursor.u32()?,
                },
                12 => MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
                    stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                    byte_count: cursor.u16()?,
                },

                18 => MachineEncodedMemoryEffect::AtomicFenceV1,
                19 => MachineEncodedMemoryEffect::CatalogFenceV1,
                20 => MachineEncodedMemoryEffect::PrivateCallbackMaterializationV1,
                11 => MachineEncodedMemoryEffect::AtomicPointerV1 {
                    pointer_operand: cursor.u16()?,
                    form: crate::AtomicForm::from_ordinal(cursor.byte()?)
                        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                },
                8 => MachineEncodedMemoryEffect::HostedReadByteV1 {
                    stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                },
                6 => MachineEncodedMemoryEffect::HostedWriteByteV1 {
                    stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                },
                9 => MachineEncodedMemoryEffect::CopyBytesV1 {
                    source_pointer_operand: cursor.u16()?,
                    destination_pointer_operand: cursor.u16()?,
                    count_operand: cursor.u16()?,
                },
                16 => MachineEncodedMemoryEffect::ReadEncodedScalarV1 {
                    pointer_operand: cursor.u16()?,
                    codec_operand: cursor.u16()?,
                },
                17 => MachineEncodedMemoryEffect::WriteEncodedScalarV1 {
                    pointer_operand: cursor.u16()?,
                    codec_operand: cursor.u16()?,
                },
                13 => MachineEncodedMemoryEffect::ScanByteSequenceV1 {
                    base_pointer_operand: cursor.u16()?,
                    count_operand: cursor.u16()?,
                },
                14 => MachineEncodedMemoryEffect::CompareByteSequencesV1 {
                    left_base_operand: cursor.u16()?,
                    left_count_operand: cursor.u16()?,
                    right_base_operand: cursor.u16()?,
                    right_count_operand: cursor.u16()?,
                },
                15 => MachineEncodedMemoryEffect::ZeroBytesV1 {
                    destination_pointer_operand: cursor.u16()?,
                    count_operand: cursor.u16()?,
                },
                0 => MachineEncodedMemoryEffect::NoneV1,
                25 => MachineEncodedMemoryEffect::InstalledInterruptCpuCheckV1 {
                    table_pointer_operand: cursor.u16()?,
                    task_pointer_operand: cursor.u16()?,
                    scratch_pointer_operand: cursor.u16()?,
                    stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                },
                26 => MachineEncodedMemoryEffect::InstalledInterruptTaskCommitV1 {
                    table_pointer_operand: cursor.u16()?,
                    task_pointer_operand: cursor.u16()?,
                    scratch_pointer_operand: cursor.u16()?,
                    stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                },
                27 => MachineEncodedMemoryEffect::InstalledInterruptTablePublicationV1 {
                    table_pointer_operand: cursor.u16()?,
                    scratch_pointer_operand: cursor.u16()?,
                },
                22 => MachineEncodedMemoryEffect::TaskRegisterActivationV1 {
                    table_pointer_operand: cursor.u16()?,
                    table_byte_count_operand: cursor.u16()?,
                    task_pointer_operand: cursor.u16()?,
                    selector_operand: cursor.u16()?,
                    scratch_pointer_operand: cursor.u16()?,
                    stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                },
                23 => MachineEncodedMemoryEffect::SegmentDescriptorReloadV1 {
                    table_pointer_operand: cursor.u16()?,
                    table_byte_count_operand: cursor.u16()?,
                    selector_operand: cursor.u16()?,
                    scratch_pointer_operand: cursor.u16()?,
                    stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                },
                21 => MachineEncodedMemoryEffect::WriteFixedPointerV1 {
                    pointer_operand: cursor.u16()?,
                    byte_count: cursor.u16()?,
                },
                7 => MachineEncodedMemoryEffect::WritePointerV1 {
                    pointer_operand: cursor.u16()?,
                },
                5 => MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                    pointer_operand: cursor.u16()?,
                    index_operand: cursor.u16()?,
                    byte_count: cursor.u16()?,
                },
                3 => MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: cursor.u16()?,
                    byte_count: cursor.u16()?,
                },
                4 => MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                    stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                    byte_count: cursor.u16()?,
                },
                10 => MachineEncodedMemoryEffect::ReadFrameStorageV1 {
                    stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                    byte_count: cursor.u16()?,
                },
                1 => MachineEncodedMemoryEffect::ReadActivationStackV1 {
                    stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                    byte_count: cursor.u16()?,
                },
                2 if allow_scalar_call => {
                    MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                        stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                        byte_count: cursor.u16()?,
                    }
                }
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            let stack = match cursor.byte()? {
                3 => MachineEncodedStackEffect::TemporaryBytesV1 {
                    stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                    byte_count: cursor.u16()?,
                },
                0 => MachineEncodedStackEffect::UnchangedV1,
                1 => MachineEncodedStackEffect::PopBytesV1 {
                    stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                    byte_count: cursor.u16()?,
                },
                2 if allow_scalar_call => MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                    stack_pointer: register_model::RegisterViewId(cursor.u16()?),
                    return_address_byte_count: cursor.u16()?,
                },
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            let trap = match cursor.byte()? {
                5 => MachineEncodedTrapBehavior::ExplicitCrashV1,
                6 => MachineEncodedTrapBehavior::TrappingIntegerV1,
                3 => MachineEncodedTrapBehavior::HostedExitReturnedV1,
                4 => MachineEncodedTrapBehavior::HostedReadFailureV1,
                2 => MachineEncodedTrapBehavior::HostedWriteFailureV1,
                0 => MachineEncodedTrapBehavior::NeverV1,
                1 => MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            let control = match cursor.byte()? {
                9 => MachineEncodedControlEffect::CrashV1,
                10 => MachineEncodedControlEffect::FallThroughOrTrapV1,
                15 => MachineEncodedControlEffect::FallThroughOrStopV1,
                16 => MachineEncodedControlEffect::PrivilegedStopV1,
                7 => MachineEncodedControlEffect::HostedExitOrTrapV1,
                8 => MachineEncodedControlEffect::HostedReadReturnOrTrapV1,
                6 => MachineEncodedControlEffect::HostedWriteReturnOrTrapV1,
                0 => MachineEncodedControlEffect::FallThroughV1,
                1 => MachineEncodedControlEffect::ConditionalRelativeBranchV1,
                2 => MachineEncodedControlEffect::ReturnFromActivationStackV1,
                3 => MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                    target: register_model::RegisterViewId(cursor.u16()?),
                },
                4 if allow_scalar_call => MachineEncodedControlEffect::DirectRelativeCallV1,
                13 if allow_scalar_call => MachineEncodedControlEffect::IndirectTableCallOrTrapV1,
                14 if allow_scalar_call => MachineEncodedControlEffect::IndirectExternalCallV1,
                11 if allow_scalar_call => MachineEncodedControlEffect::SupervisorCallV1,
                12 => MachineEncodedControlEffect::AtomicRetryUntilCommitV1,
                5 if allow_jump => MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            Ok(MachineEncodedEffects {
                external_operand_reads,
                external_operand_writes,
                implicit_unit_uses,
                implicit_unit_defs,
                implicit_unit_clobbers,
                memory,
                stack,
                trap,
                control,
            })
        }

        fn decode_u16s(
            cursor: &mut Cursor<'_>,
        ) -> Result<Vec<u16>, PreAllocationMachineEffectDecodeError> {
            let count = cursor.length()?;
            let mut values = Vec::with_capacity(count.min(cursor.remaining()));
            for _ in 0..count {
                values.push(cursor.u16()?);
            }
            Ok(values)
        }

        #[cfg(test)]
        mod local_slot_tests {
            use super::{
                Cursor, OperationId, PreAllocationMachineEffectDecodeError,
                decode_local_storage_slot,
            };
            use crate::LocalStorageSlotId;
            use semantic_vocabulary::{BlockId, PlaceId};

            #[test]
            fn structural_call_argument_slot_roundtrip_binds_operation_and_argument_index() {
                let mut identities = std::collections::BTreeSet::new();
                for (operation, argument_index) in [(1, 0), (1, 1), (2, 0), (u64::MAX, u32::MAX)] {
                    let slot = LocalStorageSlotId::StructuralCallArgument {
                        operation: OperationId::new(operation).unwrap(),
                        argument_index,
                    };
                    let mut encoded = Vec::new();
                    slot.encode_identity(&mut encoded);
                    let mut expected = vec![5];
                    crate::codec_bytes::push_encoded(&mut expected, operation.to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut expected, argument_index.to_le_bytes());
                    assert_eq!(encoded, expected);
                    assert!(identities.insert(encoded.clone()));
                    let mut cursor = Cursor::new(&encoded);
                    let decoded = decode_local_storage_slot(&mut cursor).unwrap();
                    assert_eq!(decoded, slot);
                    assert_eq!(cursor.remaining(), 0);
                    let mut reencoded = Vec::new();
                    decoded.encode_identity(&mut reencoded);
                    assert_eq!(encoded, reencoded);
                    for truncated_length in 0..encoded.len() {
                        assert!(
                            decode_local_storage_slot(&mut Cursor::new(
                                &encoded[..truncated_length]
                            ))
                            .is_err()
                        );
                    }
                    encoded[1..9].fill(0);
                    assert_eq!(
                        decode_local_storage_slot(&mut Cursor::new(&encoded)),
                        Err(PreAllocationMachineEffectDecodeError::InvalidField)
                    );
                }
            }

            #[test]
            fn owned_entry_slot_codec_retains_place_without_operation_or_block_identity() {
                let place = PlaceId::new(0x0102_0304_0506_0708).unwrap();
                let slot = LocalStorageSlotId::StructuralParameter { place };
                assert_eq!(slot.operation(), None);
                assert_eq!(slot.structural_place(), Some(place));
                let mut encoded = Vec::new();
                slot.encode_identity(&mut encoded);
                assert_eq!(encoded, [4, 8, 7, 6, 5, 4, 3, 2, 1]);
                let mut cursor = Cursor::new(&encoded);
                assert_eq!(decode_local_storage_slot(&mut cursor).unwrap(), slot);
                assert_eq!(cursor.remaining(), 0);

                for changed in [
                    LocalStorageSlotId::StructuralParameter {
                        place: PlaceId::new(place.get() + 1).unwrap(),
                    },
                    LocalStorageSlotId::StructuralBlockParameter {
                        block: BlockId::new(1).unwrap(),
                        place,
                    },
                    LocalStorageSlotId::Structural {
                        operation: OperationId::new(1).unwrap(),
                        place,
                    },
                ] {
                    let mut changed_bytes = Vec::new();
                    changed.encode_identity(&mut changed_bytes);
                    assert_ne!(changed_bytes, encoded);
                }
            }

            #[test]
            fn owned_entry_slot_decoder_rejects_zero_truncation_and_unknown_tag() {
                let encoded = [4, 1, 0, 0, 0, 0, 0, 0, 0];
                for length in 0..encoded.len() {
                    assert!(
                        decode_local_storage_slot(&mut Cursor::new(&encoded[..length])).is_err()
                    );
                }
                let mut zero_place = encoded;
                zero_place[1] = 0;
                assert_eq!(
                    decode_local_storage_slot(&mut Cursor::new(&zero_place)),
                    Err(PreAllocationMachineEffectDecodeError::InvalidField)
                );
                let mut unknown_tag = encoded;
                unknown_tag[0] = u8::MAX;
                assert_eq!(
                    decode_local_storage_slot(&mut Cursor::new(&unknown_tag)),
                    Err(PreAllocationMachineEffectDecodeError::InvalidField)
                );
            }
        }

        #[cfg(test)]
        mod packed_tests {
            use super::{Cursor, SelectedInstructionKind, decode_kind};
            #[test]
            fn packed_instruction_decode_retains_width_and_rejects_invalid_footprints() {
                for tag in [46, 47] {
                    for raw in 0..=8 {
                        let mut bytes = vec![tag];
                        crate::codec_bytes::push_encoded(&mut bytes, 37_u32.to_le_bytes());
                        bytes.push(raw);
                        let decoded = decode_kind(&mut Cursor::new(&bytes), true, true, true);
                        if let Some(width) = crate::PackedByteWidth::from_byte_size(raw) {
                            let expected = if tag == 46 {
                                SelectedInstructionKind::LoadPacked {
                                    byte_offset: 37,
                                    width,
                                }
                            } else {
                                SelectedInstructionKind::StorePacked {
                                    byte_offset: 37,
                                    width,
                                }
                            };
                            assert_eq!(decoded.unwrap(), expected);
                        } else {
                            assert!(decoded.is_err());
                        }
                        for length in 0..bytes.len() {
                            assert!(
                                decode_kind(&mut Cursor::new(&bytes[..length]), true, true, true)
                                    .is_err()
                            );
                        }
                    }
                }
            }
        }

        fn trapping_form(tag: u8) -> Option<TrappingForm> {
            TrappingForm::ALL
                .into_iter()
                .find(|form| trapping_family_tag(*form) == tag)
        }

        pub fn saturating_kind(tag: u8) -> Option<(SaturatingOperation, SaturatingCarrier)> {
            [
                SaturatingOperation::Add,
                SaturatingOperation::Subtract,
                SaturatingOperation::Divide,
                SaturatingOperation::Remainder,
                SaturatingOperation::Multiply,
            ]
            .into_iter()
            .flat_map(|operation| {
                SaturatingCarrier::ALL
                    .into_iter()
                    .map(move |carrier| (operation, carrier))
            })
            .find(|(operation, carrier)| saturating_family_tag(*operation, *carrier) == tag)
        }

        #[cfg(test)]
        mod float_result_policy_tests {
            use super::{Cursor, decode_alternative, decode_kind};
            use crate::{
                FloatResultPolicyForm, MachineAlternative, MachineAlternativeApplicability,
                MachineAlternativeKey, MachineEncodedEffects, MachineLatencyKnowledge,
                MachineSemanticKind, MachineSizeKnowledge, SelectedInstructionKind,
                encode_machine_alternative_identity,
            };

            #[test]
            fn float_result_policy_closed_payloads_decode_and_alternatives_round_trip() {
                for form in FloatResultPolicyForm::ALL {
                    let bytes = [231, form.machine_payload()];
                    let mut cursor = Cursor::new(&bytes);
                    assert_eq!(
                        decode_kind(&mut cursor, true, true, true).unwrap(),
                        SelectedInstructionKind::FloatResultPolicy { form }
                    );
                    assert_eq!(cursor.remaining(), 0);
                    let input_count = form.operand_count() as u16;
                    let alternative = MachineAlternative {
                        key: MachineAlternativeKey {
                            family: MachineSemanticKind::FloatResultPolicy(form).into(),
                            variant: 0,
                        },
                        applicability: MachineAlternativeApplicability::Always,
                        size: MachineSizeKnowledge::ExactBytes(32),
                        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                        encoded: MachineEncodedEffects::fallthrough_v1(
                            (0..input_count).collect(),
                            vec![input_count],
                        ),
                    };
                    let mut encoded = Vec::new();
                    encode_machine_alternative_identity(&mut encoded, &alternative);
                    let mut cursor = Cursor::new(&encoded);
                    assert_eq!(decode_alternative(&mut cursor).unwrap(), alternative);
                    assert_eq!(cursor.remaining(), 0);
                    for payload in (2..=5).chain(8..=u8::MAX) {
                        encoded[2] = payload;
                        assert!(decode_alternative(&mut Cursor::new(&encoded)).is_err());
                        assert!(
                            decode_kind(&mut Cursor::new(&[231, payload]), true, true, true)
                                .is_err()
                        );
                    }
                }
                assert!(decode_kind(&mut Cursor::new(&[231]), true, true, true).is_err());
                assert!(decode_alternative(&mut Cursor::new(&[231])).is_err());
            }

            #[test]
            fn extended_family_round_trip_does_not_alias_low_byte_families() {
                let alternative = MachineAlternative {
                    key: MachineAlternativeKey {
                        family: MachineSemanticKind::InterruptControl.into(),
                        variant: 0,
                    },
                    applicability: MachineAlternativeApplicability::Always,
                    size: MachineSizeKnowledge::ExactBytes(1),
                    latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                    encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![]),
                };
                let mut encoded = Vec::new();
                encode_machine_alternative_identity(&mut encoded, &alternative);
                assert_eq!(&encoded[..2], &[0, 1]);
                let mut cursor = Cursor::new(&encoded);
                assert_eq!(decode_alternative(&mut cursor).unwrap(), alternative);
                assert_eq!(cursor.remaining(), 0);
                for tag in [270_u16, u16::MAX] {
                    encoded[..2].copy_from_slice(&tag.to_le_bytes());
                    assert!(decode_alternative(&mut Cursor::new(&encoded)).is_err());
                }
            }
        }

        fn decode_call_target(
            cursor: &mut Cursor<'_>,
        ) -> Result<crate::SelectedCallTarget, PreAllocationMachineEffectDecodeError> {
            Ok(match cursor.byte()? {
                0 => crate::SelectedCallTarget::Machine(
                    MachineId::new(cursor.u64()?)
                        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                ),
                1 => crate::SelectedCallTarget::Dynamic {
                    interface: cursor.u32()?,
                    slot: cursor.u32()?,
                },
                2 => crate::SelectedCallTarget::Callable {
                    contract: semantic_vocabulary::CallableContractId::new(cursor.u64()?)
                        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                },
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            })
        }

        #[cfg(test)]
        mod port_write_tests {
            use super::*;
            #[test]
            fn port_write_decoder_retains_extreme_operands_and_rejects_truncation() {
                for (port, value) in [(0_u16, 0_u8), (0x3f8, 0x41), (u16::MAX, u8::MAX)] {
                    let [low, high] = port.to_le_bytes();
                    let bytes = [239, low, high, value];
                    let mut cursor = Cursor::new(&bytes);
                    assert_eq!(
                        decode_kind(&mut cursor, true, true, true).unwrap(),
                        SelectedInstructionKind::PortWrite { port, value }
                    );
                    assert_eq!(cursor.remaining(), 0);
                    for length in 0..bytes.len() {
                        assert!(
                            decode_kind(&mut Cursor::new(&bytes[..length]), true, true, true)
                                .is_err()
                        );
                    }
                }
            }
        }

        #[cfg(test)]
        #[test]
        fn private_address_effect_codec_keeps_the_complete_function_identity() {
            let caller = terminal_psi::TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([73; 32]),
            };
            let identity =
                function_identity::MachineFunctionIdentity::callback_thunk(caller, 17).unwrap();
            let kind = SelectedInstructionKind::ImageAddress {
                reference: crate::SelectedImageAddressReference::PrivateFunction { identity },
            };
            let mut bytes = Vec::new();
            crate::kind_encoding::encode_kind(&mut bytes, 232, kind);
            let mut cursor = Cursor::new(&bytes);
            assert_eq!(decode_kind(&mut cursor, true, true, true), Ok(kind));
            assert_eq!(cursor.remaining(), 0);
            for length in 0..bytes.len() {
                assert!(decode_kind(&mut Cursor::new(&bytes[..length]), true, true, true).is_err());
            }
            bytes[4..36].fill(0);
            assert_eq!(
                decode_kind(&mut Cursor::new(&bytes), true, true, true),
                Err(PreAllocationMachineEffectDecodeError::InvalidField)
            );
        }

        #[cfg(test)]
        #[test]
        fn activation_base_codec_has_a_closed_address_only_payload() {
            let kind = SelectedInstructionKind::FrameAddress {
                slot: crate::FrameStorageSlotId::ActivationBase,
                byte_offset: 0,
            };
            let mut bytes = Vec::new();
            crate::kind_encoding::encode_kind(&mut bytes, 18, kind);
            assert_eq!(bytes, [18, 4, 0, 0, 0, 0]);
            assert_eq!(
                decode_kind(&mut Cursor::new(&bytes), true, true, true).unwrap(),
                kind
            );
            bytes[0] = 17;
            assert!(decode_kind(&mut Cursor::new(&bytes), true, true, true).is_err());
            bytes[0] = 18;
            bytes[2] = 1;
            assert!(decode_kind(&mut Cursor::new(&bytes), true, true, true).is_err());
            bytes.truncate(2);
            assert!(decode_kind(&mut Cursor::new(&bytes), true, true, true).is_err());
        }
    }
    mod ownership {
        use super::{
            ClaimId, Cursor, ObligationId, OwnershipEvent, PlaceId,
            PreAllocationMachineEffectDecodeError, StructuralTypeId, decode_ids, decode_machine,
        };
        use terminal_psi::StructuralPathSegment::Field;
        use terminal_psi::TerminalAffineCleanupAction::{DiscardResidual, InvokeNominal};
        pub fn decode_ownership(
            cursor: &mut Cursor<'_>,
        ) -> Result<Vec<OwnershipEvent>, PreAllocationMachineEffectDecodeError> {
            let count = cursor.length()?;
            let mut ownership = Vec::with_capacity(count.min(cursor.remaining()));
            for _ in 0..count {
                let event = match cursor.byte()? {
                    1 => OwnershipEvent::ClaimTransfer(decode_claim_ids(cursor)?),
                    2 => {
                        let count = cursor.length()?;
                        let mut receipts = Vec::with_capacity(count.min(cursor.remaining()));
                        for _ in 0..count {
                            let claim = ClaimId::new(cursor.u64()?)
                                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
                            receipts.push(terminal_psi::CompletionReceipt {
                                claim,
                                argument_index: cursor.u32()?,
                                path: decode_path(cursor)?,
                            });
                        }
                        OwnershipEvent::ClaimCompletion(receipts)
                    }
                    3 => {
                        let action_count = cursor.length()?;
                        let mut actions = Vec::with_capacity(action_count.min(cursor.remaining()));
                        for _ in 0..action_count {
                            actions.push(decode_cleanup(cursor)?);
                        }
                        OwnershipEvent::Cleanup(actions)
                    }
                    4 => OwnershipEvent::StructuralReturn(decode_claim_ids(cursor)?),
                    5 => OwnershipEvent::CrashFrontier(decode_claim_ids(cursor)?),
                    6 => OwnershipEvent::ExternalCompletionFrontier(decode_claim_ids(cursor)?),
                    _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
                };
                ownership.push(event);
            }
            Ok(ownership)
        }

        fn decode_claim_ids(
            cursor: &mut Cursor<'_>,
        ) -> Result<Vec<ClaimId>, PreAllocationMachineEffectDecodeError> {
            decode_ids(cursor, ClaimId::new)
        }

        fn decode_cleanup(
            cursor: &mut Cursor<'_>,
        ) -> Result<terminal_psi::TerminalAffineCleanupAction, PreAllocationMachineEffectDecodeError>
        {
            Ok(match cursor.byte()? {
                4 => terminal_psi::TerminalAffineCleanupAction::RetireHome(decode_place(cursor)?),
                1 => terminal_psi::TerminalAffineCleanupAction::DiscardRoot(decode_place(cursor)?),
                2 => DiscardResidual(terminal_psi::StructuralAffineDiscard {
                    place: decode_place(cursor)?,
                    path: decode_path(cursor)?,
                    structural_type: decode_structural_type(cursor)?,
                }),
                tag @ (3 | 5) => {
                    let place = decode_place(cursor)?;
                    let path = decode_path(cursor)?;
                    let structural_type = decode_structural_type(cursor)?;
                    let cleanup_machine = decode_machine(cursor)?;
                    let consumed_claims = decode_claim_ids(cursor)?;
                    let cleanup_receiver = match cursor.byte()? {
                        0 => None,
                        1 => Some(decode_place(cursor)?),
                        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
                    };
                    let requirement_obligations = decode_ids(cursor, ObligationId::new)?;
                    InvokeNominal(terminal_psi::NominalAffineCleanup {
                        retains_descendants: tag == 5,
                        place,
                        path,
                        structural_type,
                        cleanup_machine,
                        consumed_claims,
                        cleanup_receiver,
                        requirement_obligations,
                    })
                }
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            })
        }

        fn decode_path(
            cursor: &mut Cursor<'_>,
        ) -> Result<Vec<terminal_psi::StructuralPathSegment>, PreAllocationMachineEffectDecodeError>
        {
            let count = cursor.length()?;
            let mut path = Vec::with_capacity(count.min(cursor.remaining()));
            for _ in 0..count {
                path.push(match cursor.byte()? {
                    1 => Field(terminal_psi::TerminalIdentityIndex(cursor.u32()?)),
                    2 => terminal_psi::StructuralPathSegment::FixedIndex(cursor.u64()?),
                    3 => terminal_psi::StructuralPathSegment::Referent,
                    4 => terminal_psi::StructuralPathSegment::FixedRange {
                        start: cursor.u64()?,
                        end: cursor.u64()?,
                    },
                    6 => terminal_psi::StructuralPathSegment::RuntimeIndex {
                        index: semantic_vocabulary::ValueId::new(cursor.u64()?)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                        obligation: ObligationId::new(cursor.u64()?)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    },
                    7 => terminal_psi::StructuralPathSegment::Case(
                        semantic_vocabulary::StructuralCaseId::new(cursor.u64()?)
                            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                    ),
                    _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
                });
            }
            Ok(path)
        }

        fn decode_place(
            cursor: &mut Cursor<'_>,
        ) -> Result<PlaceId, PreAllocationMachineEffectDecodeError> {
            PlaceId::new(cursor.u64()?).ok_or(PreAllocationMachineEffectDecodeError::InvalidField)
        }

        fn decode_structural_type(
            cursor: &mut Cursor<'_>,
        ) -> Result<StructuralTypeId, PreAllocationMachineEffectDecodeError> {
            StructuralTypeId::new(cursor.u64()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::selected_instructions::effects::program::identity::encode_cleanup;
            use terminal_psi::TerminalIdentityIndex;
            use terminal_psi::{
                StructuralAffineDiscard, StructuralPathSegment, TerminalAffineCleanupAction,
            };

            #[test]
            fn nominal_cleanup_round_trip_retains_projected_path() {
                let mut previous_encoding = None;
                for path in [
                    Vec::new(),
                    vec![StructuralPathSegment::Field(TerminalIdentityIndex(1))],
                    vec![
                        StructuralPathSegment::Field(TerminalIdentityIndex(1)),
                        StructuralPathSegment::FixedIndex(1),
                    ],
                ] {
                    let action = TerminalAffineCleanupAction::InvokeNominal(
                        terminal_psi::NominalAffineCleanup {
                            retains_descendants: false,
                            place: PlaceId::new(1).unwrap(),
                            path,
                            structural_type: StructuralTypeId::new(2).unwrap(),
                            cleanup_machine: semantic_vocabulary::MachineId::new(3).unwrap(),
                            consumed_claims: vec![ClaimId::new(4).unwrap()],
                            cleanup_receiver: Some(PlaceId::new(5).unwrap()),
                            requirement_obligations: vec![ObligationId::new(6).unwrap()],
                        },
                    );
                    let mut encoded = Vec::new();
                    encode_cleanup(&mut encoded, &action);
                    let mut cursor = Cursor::new(&encoded);
                    assert_eq!(decode_cleanup(&mut cursor).unwrap(), action);
                    assert_eq!(cursor.remaining(), 0);
                    if let Some(previous) = previous_encoding {
                        assert_ne!(encoded, previous);
                    }
                    previous_encoding = Some(encoded);
                }

                let mut retired_envelope = super::super::MAGIC.to_vec();
                crate::codec_bytes::push_encoded(&mut retired_envelope, 33_u32.to_le_bytes());
                assert_eq!(
                    super::super::decode_terminal_pre_allocation_machine_effect_plan(
                        &retired_envelope
                    ),
                    Err(PreAllocationMachineEffectDecodeError::UnsupportedVersion(
                        33
                    ))
                );
            }
            #[test]
            fn cleanup_action_round_trip_retains_every_path_segment() {
                let action =
                    TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard {
                        place: PlaceId::new(1).unwrap(),
                        structural_type: StructuralTypeId::new(2).unwrap(),
                        path: vec![
                            StructuralPathSegment::Field(TerminalIdentityIndex(1)),
                            StructuralPathSegment::FixedIndex(3),
                            StructuralPathSegment::Referent,
                            StructuralPathSegment::FixedRange { start: 4, end: 8 },
                            StructuralPathSegment::RuntimeIndex {
                                index: semantic_vocabulary::ValueId::new(5).unwrap(),
                                obligation: ObligationId::new(6).unwrap(),
                            },
                            StructuralPathSegment::Case(
                                semantic_vocabulary::StructuralCaseId::new(7).unwrap(),
                            ),
                        ],
                    });
                let mut encoded = Vec::new();
                encode_cleanup(&mut encoded, &action);
                let mut cursor = Cursor::new(&encoded);
                assert_eq!(decode_cleanup(&mut cursor).unwrap(), action);
                assert_eq!(cursor.remaining(), 0);

                let mut retired_path = 1_u64.to_le_bytes().to_vec();
                retired_path.push(5);
                assert_eq!(
                    decode_path(&mut Cursor::new(&retired_path)),
                    Err(PreAllocationMachineEffectDecodeError::InvalidField)
                );

                let mut retired_envelope = super::super::MAGIC.to_vec();
                crate::codec_bytes::push_encoded(&mut retired_envelope, 32_u32.to_le_bytes());
                assert_eq!(
                    super::super::decode_terminal_pre_allocation_machine_effect_plan(
                        &retired_envelope
                    ),
                    Err(PreAllocationMachineEffectDecodeError::UnsupportedVersion(
                        32
                    ))
                );
            }
        }
    }
    mod values {
        use super::{
            Architecture, Cursor, MachineId, NativeTarget, ObjectFormat, ObligationId,
            PreAllocationMachineEffectDecodeError, RegisterConstraintFamily, RegisterConstraintKey,
            RegisterUnitId,
        };
        pub fn decode_target(
            cursor: &mut Cursor<'_>,
        ) -> Result<NativeTarget, PreAllocationMachineEffectDecodeError> {
            let architecture = match cursor.byte()? {
                0 => Architecture::Aarch64,
                1 => Architecture::X86_64,
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            let object_format = match cursor.byte()? {
                0 => ObjectFormat::Elf,
                1 => ObjectFormat::MachO,
                2 => ObjectFormat::Coff,
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            let pointer_size = usize::try_from(cursor.u64()?)
                .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)?;
            let pointer_alignment = usize::try_from(cursor.u64()?)
                .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)?;
            Ok(NativeTarget {
                architecture,
                object_format,
                pointer_size,
                pointer_alignment,
            })
        }

        pub(super) fn decode_constraint_key(
            cursor: &mut Cursor<'_>,
        ) -> Result<RegisterConstraintKey, PreAllocationMachineEffectDecodeError> {
            let family = match cursor.byte()? {
                0 => RegisterConstraintFamily::Call,
                1 => RegisterConstraintFamily::Return,
                2 => RegisterConstraintFamily::SystemCall,
                3 => RegisterConstraintFamily::InlineAssembly,
                4 => RegisterConstraintFamily::Instruction,
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            Ok(RegisterConstraintKey {
                family,
                variant: cursor.u32()?,
            })
        }

        pub fn decode_units(
            cursor: &mut Cursor<'_>,
        ) -> Result<Vec<RegisterUnitId>, PreAllocationMachineEffectDecodeError> {
            let count = cursor.length()?;
            let mut units = Vec::with_capacity(count.min(cursor.remaining()));
            for _ in 0..count {
                units.push(RegisterUnitId(cursor.u16()?));
            }
            Ok(units)
        }

        pub(super) fn decode_ids<T>(
            cursor: &mut Cursor<'_>,
            constructor: impl Fn(u64) -> Option<T>,
        ) -> Result<Vec<T>, PreAllocationMachineEffectDecodeError> {
            let count = cursor.length()?;
            let mut values = Vec::with_capacity(count.min(cursor.remaining()));
            for _ in 0..count {
                values.push(
                    constructor(cursor.u64()?)
                        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                );
            }
            Ok(values)
        }

        pub(super) fn decode_machine(
            cursor: &mut Cursor<'_>,
        ) -> Result<MachineId, PreAllocationMachineEffectDecodeError> {
            MachineId::new(cursor.u64()?).ok_or(PreAllocationMachineEffectDecodeError::InvalidField)
        }

        pub(super) fn decode_obligation(
            cursor: &mut Cursor<'_>,
        ) -> Result<ObligationId, PreAllocationMachineEffectDecodeError> {
            ObligationId::new(cursor.u64()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)
        }
        pub(super) fn decode_accepted_fact(
            cursor: &mut Cursor<'_>,
        ) -> Result<
            optimization_core::AcceptedObligationFactIdentity,
            PreAllocationMachineEffectDecodeError,
        > {
            Ok(optimization_core::AcceptedObligationFactIdentity::from_bytes(cursor.array()?))
        }
    }

    pub(crate) use framing::{
        decode_terminal_pre_allocation_machine_effect_plan,
        encode_terminal_pre_allocation_machine_effect_plan,
    };
    use instruction::decode_instruction;
    pub use instruction::{
        decode_alternative, decode_local_storage_slot, decode_provenance, saturating_kind,
    };
    pub use ownership::decode_ownership;
    use values::{
        decode_accepted_fact, decode_constraint_key, decode_ids, decode_machine, decode_obligation,
    };
    pub use values::{decode_target, decode_units};

    const MAGIC: &[u8; 8] = b"OMGMFX\0\0";
    const VERSION: u32 = 49;
}

pub use cursor::Cursor;
pub use error::PreAllocationMachineEffectDecodeError;
pub use v6::{
    decode_alternative, decode_local_storage_slot, decode_ownership, decode_provenance,
    decode_target, decode_units, saturating_kind,
};
pub(crate) use v6::{
    decode_terminal_pre_allocation_machine_effect_plan,
    encode_terminal_pre_allocation_machine_effect_plan,
};
