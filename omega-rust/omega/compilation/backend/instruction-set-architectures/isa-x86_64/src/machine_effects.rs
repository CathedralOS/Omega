/*
We build and validate the x86-64 machine-effect catalog
(`x86_64_machine_effect_catalog` and `validate_x86_64_machine_effect_catalog`),
mapping every x86-64 register-constraint key and normalized foreign call to its
exact encoded register, flag, memory, stack, control-flow, and trap effects and
byte-size bounds.
*/

use register_model::{TargetRegisterEnvironmentConstraintKeys, ValidatedRegisterConstraintCatalog};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey, MachineBarrier,
    MachineCallEffect, MachineCleanupEffect, MachineEffectCatalog, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
    MachineMemoryEffect, MachineSemanticKind as Semantic, MachineSizeKnowledge as Size,
    MachineTrapBehavior, SaturatingCarrier, SaturatingOperation, SelectedConstraintKeys,
    isa_machine_effect_catalog_plumbing,
};
use target::{Architecture, NativeTarget, ObjectFormat};

mod memory {
    use register_model::{RegisterConstraintKey, ValidatedRegisterConstraintCatalog};
    use selected_instructions::{
        MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey, MachineBarrier,
        MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration,
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
        MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
        MachineMemoryEffect, MachineSemanticKind as Semantic, MachineSizeKnowledge as Size,
        MachineTrapBehavior,
    };

    macro_rules! declared_semantics {
        () => {
            Semantic::AccessorScalarAccess(_)
                | Semantic::CopyBytes
                | Semantic::ZeroBytes
                | Semantic::LoadEncodedScalar
                | Semantic::StoreEncodedScalar
                | Semantic::ScanByteSequence
                | Semantic::CompareByteSequences
                | Semantic::LoadPacked3
                | Semantic::LoadPacked5
                | Semantic::LoadPacked6
                | Semantic::LoadPacked7
                | Semantic::StorePacked
                | Semantic::Store
                | Semantic::AddressOffset
                | Semantic::Load8Indexed
                | Semantic::Load8
                | Semantic::Load16
                | Semantic::Load32
                | Semantic::Load64
                | Semantic::Store64
                | Semantic::FrameAddress
                | Semantic::CallUnit
        };
    }
    pub(super) use declared_semantics;

    pub(super) fn declaration(
        semantic: Semantic,
        constraint: RegisterConstraintKey,
        constraints: &ValidatedRegisterConstraintCatalog,
    ) -> MachineEffectDeclaration {
        let row = constraints
            .catalog()
            .constraint(constraint)
            .expect("canonical memory instruction constraint");
        let stack_pointer = crate::x86_64_physical_register_model_ref()
            .view_named("rsp")
            .expect("canonical stack pointer")
            .id;
        let (memory, trap, reads, writes, encoded_memory, size) = match semantic {
            Semantic::AccessorScalarAccess(form) => (
                if form.write {
                    MachineMemoryEffect::WritePointerV1
                } else {
                    MachineMemoryEffect::ReadPointerV1
                },
                MachineTrapBehavior::MayArchitecturalFaultV1,
                if form.write { vec![0, 1] } else { vec![0] },
                if form.write { vec![] } else { vec![1] },
                if form.write {
                    MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
                } else {
                    MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: u16::from(form.bytes),
                    }
                },
                crate::encoder_resolved(7, Some(9)),
            ),

            Semantic::CopyBytes => (
                MachineMemoryEffect::CopyBytesV1,
                MachineTrapBehavior::MayArchitecturalFaultV1,
                vec![0, 1, 2],
                vec![3, 4],
                MachineEncodedMemoryEffect::CopyBytesV1 {
                    source_pointer_operand: 0,
                    destination_pointer_operand: 1,
                    count_operand: 2,
                },
                Size::ExactBytes(36),
            ),
            Semantic::ZeroBytes => (
                MachineMemoryEffect::ZeroBytesV1,
                MachineTrapBehavior::MayArchitecturalFaultV1,
                vec![0, 1],
                vec![2],
                MachineEncodedMemoryEffect::ZeroBytesV1 {
                    destination_pointer_operand: 0,
                    count_operand: 1,
                },
                Size::ExactBytes(25),
            ),
            Semantic::LoadEncodedScalar => (
                MachineMemoryEffect::ReadEncodedScalarV1,
                MachineTrapBehavior::MayArchitecturalFaultV1,
                vec![0, 1],
                vec![2, 3, 4, 5],
                MachineEncodedMemoryEffect::ReadEncodedScalarV1 {
                    pointer_operand: 0,
                    codec_operand: 1,
                },
                crate::encoder_resolved(16, None),
            ),
            Semantic::StoreEncodedScalar => (
                MachineMemoryEffect::WriteEncodedScalarV1,
                MachineTrapBehavior::MayArchitecturalFaultV1,
                vec![0, 1, 2],
                vec![3, 4, 5],
                MachineEncodedMemoryEffect::WriteEncodedScalarV1 {
                    pointer_operand: 0,
                    codec_operand: 1,
                },
                crate::encoder_resolved(16, None),
            ),
            Semantic::ScanByteSequence => (
                MachineMemoryEffect::ScanByteSequenceV1,
                MachineTrapBehavior::MayArchitecturalFaultV1,
                vec![0, 1],
                vec![2, 3, 4],
                MachineEncodedMemoryEffect::ScanByteSequenceV1 {
                    base_pointer_operand: 0,
                    count_operand: 1,
                },
                crate::encoder_resolved(16, None),
            ),
            Semantic::CompareByteSequences => (
                MachineMemoryEffect::CompareByteSequencesV1,
                MachineTrapBehavior::MayArchitecturalFaultV1,
                vec![0, 1, 2, 3],
                vec![4, 5, 6, 7],
                MachineEncodedMemoryEffect::CompareByteSequencesV1 {
                    left_base_operand: 0,
                    left_count_operand: 1,
                    right_base_operand: 2,
                    right_count_operand: 3,
                },
                crate::encoder_resolved(16, None),
            ),
            Semantic::LoadPacked3
            | Semantic::LoadPacked5
            | Semantic::LoadPacked6
            | Semantic::LoadPacked7 => {
                let byte_count = match semantic {
                    Semantic::LoadPacked3 => 3,
                    Semantic::LoadPacked5 => 5,
                    Semantic::LoadPacked6 => 6,
                    _ => 7,
                };
                (
                    MachineMemoryEffect::ReadPointerV1,
                    MachineTrapBehavior::MayArchitecturalFaultV1,
                    vec![0],
                    vec![1, 2],
                    MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count,
                    },
                    crate::encoder_resolved(
                        byte_count * 8 + (byte_count - 1) * 7,
                        Some(byte_count * 9 + (byte_count - 1) * 7),
                    ),
                )
            }
            Semantic::StorePacked => (
                MachineMemoryEffect::WritePointerV1,
                MachineTrapBehavior::MayArchitecturalFaultV1,
                vec![0, 1],
                vec![2],
                MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
                crate::encoder_resolved(32, Some(83)),
            ),
            Semantic::Store => (
                MachineMemoryEffect::WritePointerV1,
                MachineTrapBehavior::MayArchitecturalFaultV1,
                vec![0, 1],
                vec![],
                MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
                crate::encoder_resolved(7, Some(9)),
            ),
            Semantic::AddressOffset => (
                MachineMemoryEffect::NoneV1,
                MachineTrapBehavior::NeverV1,
                vec![0],
                vec![1],
                MachineEncodedMemoryEffect::NoneV1,
                crate::encoder_resolved(7, Some(8)),
            ),
            Semantic::Load8Indexed => (
                MachineMemoryEffect::ReadPointerV1,
                MachineTrapBehavior::MayArchitecturalFaultV1,
                vec![0, 1],
                vec![2],
                MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                    pointer_operand: 0,
                    index_operand: 1,
                    byte_count: 1,
                },
                Size::ExactBytes(9),
            ),
            Semantic::Load8 | Semantic::Load16 => (
                MachineMemoryEffect::ReadPointerV1,
                MachineTrapBehavior::MayArchitecturalFaultV1,
                vec![0],
                vec![1],
                MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: if semantic == Semantic::Load8 { 1 } else { 2 },
                },
                crate::encoder_resolved(8, Some(9)),
            ),
            Semantic::Load32 | Semantic::Load64 => (
                MachineMemoryEffect::ReadPointerV1,
                MachineTrapBehavior::MayArchitecturalFaultV1,
                vec![0],
                vec![1],
                MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: if semantic == Semantic::Load32 { 4 } else { 8 },
                },
                crate::encoder_resolved(7, Some(8)),
            ),
            Semantic::Store64 => (
                MachineMemoryEffect::WriteFrameStorageV1,
                MachineTrapBehavior::MayArchitecturalFaultV1,
                vec![0],
                vec![1],
                MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                    stack_pointer,
                    byte_count: 8,
                },
                crate::encoder_resolved(15, Some(31)),
            ),
            Semantic::FrameAddress => (
                MachineMemoryEffect::NoneV1,
                MachineTrapBehavior::NeverV1,
                vec![],
                vec![0],
                MachineEncodedMemoryEffect::NoneV1,
                crate::encoder_resolved(8, Some(23)),
            ),
            Semantic::CallUnit => (
                MachineMemoryEffect::NoneV1,
                MachineTrapBehavior::NeverV1,
                row.operands.iter().map(|operand| operand.operand).collect(),
                vec![],
                MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                    stack_pointer,
                    byte_count: 8,
                },
                Size::ExactBytes(5),
            ),
            _ => unreachable!("ordinary address/call declaration only"),
        };
        let is_call = semantic == Semantic::CallUnit;
        MachineEffectDeclaration {
            semantic,
            constraint,
            memory,
            trap,
            barrier: if matches!(semantic, Semantic::AccessorScalarAccess(_)) {
                MachineBarrier::ExternalEffect
            } else if is_call {
                MachineBarrier::Call
            } else {
                MachineBarrier::None
            },
            call: if is_call {
                MachineCallEffect::InternalNormalReturnV1 {
                    pre_call_stack_alignment: 16,
                }
            } else {
                MachineCallEffect::NoneV1
            },
            cleanup: MachineCleanupEffect::NoneV1,
            alternatives: vec![MachineAlternative {
                key: MachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                },
                applicability: MachineAlternativeApplicability::Always,
                size,
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded: MachineEncodedEffects {
                    external_operand_reads: reads,
                    external_operand_writes: writes,
                    implicit_unit_uses: row.implicit_uses.clone(),
                    implicit_unit_defs: row.implicit_defs.clone(),
                    implicit_unit_clobbers: row.clobbers.clone(),
                    memory: encoded_memory,
                    stack: if is_call {
                        MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                            stack_pointer,
                            return_address_byte_count: 8,
                        }
                    } else {
                        MachineEncodedStackEffect::UnchangedV1
                    },
                    trap: if matches!(semantic, Semantic::FrameAddress | Semantic::AddressOffset) {
                        MachineEncodedTrapBehavior::NeverV1
                    } else {
                        MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                    },
                    control: if is_call {
                        MachineEncodedControlEffect::DirectRelativeCallV1
                    } else {
                        MachineEncodedControlEffect::FallThroughV1
                    },
                },
            }],
        }
    }
}
mod scalar_call {
    use register_model::{RegisterConstraintKey, ValidatedRegisterConstraintCatalog};
    use selected_instructions::{
        MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey, MachineBarrier,
        MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration,
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
        MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
        MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
    };

    pub(crate) fn user_direct_call_effects(
        row: &register_model::RegisterInstructionConstraint,
        stack_pointer: register_model::RegisterViewId,
    ) -> MachineEncodedEffects {
        MachineEncodedEffects {
            external_operand_reads: row.operand_reads(),
            external_operand_writes: row.operand_writes(),
            implicit_unit_uses: row.implicit_uses.clone(),
            implicit_unit_defs: row.implicit_defs.clone(),
            implicit_unit_clobbers: row.clobbers.clone(),
            memory: MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                stack_pointer,
                byte_count: 8,
            },
            stack: MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                stack_pointer,
                return_address_byte_count: 8,
            },
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::DirectRelativeCallV1,
        }
    }

    pub(super) fn declaration(
        semantic: MachineSemanticKind,
        constraint: RegisterConstraintKey,
        constraints: &ValidatedRegisterConstraintCatalog,
    ) -> MachineEffectDeclaration {
        let row = constraints
            .catalog()
            .constraint(constraint)
            .expect("canonical x86-64 catalog contains its scalar-call constraint");
        let stack_pointer = crate::x86_64_physical_register_model_ref()
            .view_named("rsp")
            .expect("canonical x86-64 model declares rsp")
            .id;
        MachineEffectDeclaration::call(
            semantic,
            constraint,
            16,
            5,
            (semantic == MachineSemanticKind::NormalizedForeignCall).then_some(12),
            user_direct_call_effects(row, stack_pointer),
        )
    }

    pub fn x86_64_normalized_foreign_call_declaration(
        record: &selected_instructions::SelectedNormalizedForeignCall,
        row: &register_model::RegisterInstructionConstraint,
    ) -> Option<MachineEffectDeclaration> {
        let expected = normalized_foreign_constraint(record)?;
        if *row != expected {
            return None;
        }
        let supervisor = record.call.binding.system_call().is_some();
        let table = if record.call.binding.table_dispatch().is_some() {
            Some(crate::selected_form_encoding::table_call::table_address(
                record,
            )?)
        } else {
            None
        };
        let stack_pointer = crate::x86_64_physical_register_model_ref()
            .view_named("rsp")?
            .id;
        let mut register_destinations = 0_u16;
        let mut field_destinations = 0_u16;
        let mut stack_destinations = 0_u16;
        let mut callback_byte_count = 0_u16;
        for callback in &record.call.callbacks {
            if Some(callback.terminal_operation) != record.invocation_site.operation() {
                return None;
            }
            match callback.destination.clone() {
                calling_conventions::NativePlace::Parameter(_) => {
                    let bytes = match callback.application.placement.locations.as_slice() {
                        [calling_conventions::ValueLocation::Register { .. }] => {
                            register_destinations = register_destinations.checked_add(1)?;
                            7
                        }
                        [
                            calling_conventions::ValueLocation::Stack {
                                stack_byte_offset, ..
                            },
                        ] if *stack_byte_offset <= i32::MAX as u32 => {
                            stack_destinations = stack_destinations.checked_add(1)?;
                            if *stack_byte_offset <= i8::MAX as u32 {
                                12
                            } else {
                                15
                            }
                        }
                        _ => return None,
                    };
                    callback_byte_count = callback_byte_count.checked_add(bytes)?;
                }
                calling_conventions::NativePlace::Field { .. } => {
                    field_destinations = field_destinations.checked_add(1)?;
                    let [calling_conventions::ValueLocation::Register { register, .. }] =
                        callback.application.placement.locations.as_slice()
                    else {
                        return None;
                    };
                    let destination =
                        crate::X86_64CallbackMaterializationDestination::RecordField {
                            root_register: *register,
                            placements: callback.field_placements.clone()?,
                        };
                    let bytes = u16::try_from(
                        crate::selected_form_encoding::callback_group_bytes(destination)?.len(),
                    )
                    .ok()?;
                    callback_byte_count = callback_byte_count.checked_add(bytes)?;
                }
            }
        }
        if supervisor && !record.call.callbacks.is_empty() {
            return None;
        }
        let variant = u32::from(register_destinations != 0)
            | (u32::from(field_destinations != 0) << 1)
            | (u32::from(stack_destinations != 0) << 2);
        let byte_count = if supervisor {
            12
        } else if table.is_some() {
            10
        } else {
            5_u16.checked_add(callback_byte_count)?
        };
        Some(MachineEffectDeclaration {
            semantic: MachineSemanticKind::NormalizedForeignCall,
            constraint: row.key,
            memory: if field_destinations != 0 || stack_destinations != 0 {
                MachineMemoryEffect::PrivateCallbackMaterializationV1
            } else if table.is_some() {
                MachineMemoryEffect::ReadPointerV1
            } else {
                MachineMemoryEffect::NoneV1
            },
            trap: if field_destinations != 0 || stack_destinations != 0 || table.is_some() {
                MachineTrapBehavior::MayArchitecturalFaultV1
            } else {
                MachineTrapBehavior::NeverV1
            },
            barrier: MachineBarrier::Call,
            call: if supervisor {
                MachineCallEffect::SupervisorNormalReturnV1
            } else {
                MachineCallEffect::ExternalNormalReturnV1 {
                    pre_call_stack_alignment: 16,
                }
            },
            cleanup: MachineCleanupEffect::NoneV1,
            alternatives: vec![MachineAlternative {
                key: MachineAlternativeKey {
                    family: selected_instructions::MachineAlternativeFamily::NormalizedForeignCall,
                    variant,
                },
                applicability: MachineAlternativeApplicability::Always,
                size: MachineSizeKnowledge::ExactBytes(byte_count),
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded: {
                    let mut encoded = user_direct_call_effects(row, stack_pointer);
                    if field_destinations != 0 || stack_destinations != 0 {
                        encoded.memory =
                            MachineEncodedMemoryEffect::PrivateCallbackMaterializationV1;
                    }
                    if supervisor {
                        encoded.memory = MachineEncodedMemoryEffect::NoneV1;
                        encoded.stack = MachineEncodedStackEffect::UnchangedV1;
                        encoded.control = MachineEncodedControlEffect::SupervisorCallV1;
                    }
                    if let Some((_, byte_offset)) = table {
                        let position =
                            selected_instructions::calls::constraints::foreign_argument_registers(
                                &record.call,
                            )
                            .iter()
                            .position(|(parameter, _)| *parameter == 0)?;
                        encoded.memory = MachineEncodedMemoryEffect::ReadTableFieldV1 {
                            pointer_operand: u16::try_from(position).ok()?,
                            byte_offset,
                        };
                        encoded.control = MachineEncodedControlEffect::IndirectExternalCallV1;
                    }
                    encoded
                },
            }],
        })
    }

    pub(crate) fn normalized_foreign_constraint(
        record: &selected_instructions::SelectedNormalizedForeignCall,
    ) -> Option<register_model::RegisterInstructionConstraint> {
        let target = record.call.binding.native_target()?;
        let abi = super::x86_64_selected_abi(target).ok()?;
        let key = if record.call.binding.system_call().is_some() {
            *crate::x86_64_linux_system_call_keys().first()?
        } else {
            match abi {
                super::X86_64SelectedAbi::SystemV => {
                    *crate::x86_64_system_v_normalized_foreign_call_keys().first()?
                }
                super::X86_64SelectedAbi::Microsoft => {
                    *crate::x86_64_microsoft_normalized_foreign_call_keys().first()?
                }
            }
        };
        let physical = crate::validated_x86_64_physical_register_model();
        let catalog = crate::x86_64_register_constraint_catalog_for(physical);
        let base = catalog.constraint(key)?;
        selected_instructions::calls::constraints::instantiate_foreign_call_constraint(
            &record.call,
            base,
            physical,
            |register| crate::x86_64_fixed_register_view(physical, register),
        )
    }

    #[cfg(test)]
    mod tests {
        use target::NativeTarget;

        #[test]
        fn system_call_effects_reject_user_space_call_state_and_missing_clobbers() {
            let physical = crate::validated_x86_64_physical_register_model();
            let constraints = crate::validate_x86_64_register_constraint_catalog(
                crate::x86_64_register_constraint_catalog(physical),
                physical,
            )
            .unwrap();
            let target = NativeTarget::linux_x64();
            let catalog = crate::x86_64_machine_effect_catalog(target, &constraints).unwrap();
            crate::validate_x86_64_machine_effect_catalog(target, &constraints, catalog.clone())
                .expect("kernel entry effects");
            let other =
                crate::x86_64_machine_effect_catalog(NativeTarget::macos_x64(), &constraints)
                    .unwrap();
            isa_conformance::system_call::effects_conformance(
                &constraints,
                &catalog,
                &crate::x86_64_linux_system_call_keys(),
                physical.model().view_named("rsp").unwrap().id,
                &other,
            );
        }
    }
}
pub use scalar_call::x86_64_normalized_foreign_call_declaration;
pub(crate) use scalar_call::{normalized_foreign_constraint, user_direct_call_effects};

use scalar_call::declaration as scalar_call_declaration;

use crate::register_model::{units, units_of, view_of};
use crate::selected_form_encoding::saturating_forms::SaturatingForm;
use crate::selected_form_encoding::trapping_forms::TrappingShape;
use crate::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64, X86_64_COMPARE_I64_IMMEDIATE,
    X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH, X86_64_COPY_I64, X86_64_MATERIALIZE_I64,
    X86_64_MICROSOFT_RETURN, X86_64_MICROSOFT_RETURN_UNIT, X86_64_SUBTRACT_I64,
    X86_64_SUBTRACT_I64_IMMEDIATE, X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT,
    x86_64_system_v_register_call_keys,
};

isa_machine_effect_catalog_plumbing!(
    X86_64MachineEffectCatalogValidationError,
    validate_x86_64_machine_effect_catalog,
    Architecture::X86_64,
    "x86-64",
    x86_64_machine_effect_catalog
);

pub fn x86_64_machine_effect_catalog(
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<MachineEffectCatalog, X86_64MachineEffectCatalogValidationError> {
    if target.architecture != Architecture::X86_64
        || constraints.architecture() != Architecture::X86_64
    {
        return Err(X86_64MachineEffectCatalogValidationError::TargetArchitectureMismatch);
    }
    let selected_keys = x86_64_selected_constraint_keys(target)
        .ok_or(X86_64MachineEffectCatalogValidationError::UnsupportedTargetAbi)?;
    Ok(MachineEffectCatalog {
        target,
        register_constraints: constraints.identity(),
        selected_keys: selected_keys.clone(),
        declarations: selected_keys
            .declaration_keys()
            .into_iter()
            .map(|(semantic, constraint)| {
                if let Some(form) =
                    crate::selected_form_encoding::installed_interrupt::Form::from_semantic(
                        semantic,
                    )
                {
                    return Ok(
                        crate::selected_form_encoding::installed_interrupt::declaration(
                            form, constraint,
                        ),
                    );
                }
                if semantic == Semantic::MemoryFence {
                    return Ok(crate::selected_form_encoding::memory_fence::declaration(
                        constraint,
                    ));
                }
                if let Some(operation) =
                    crate::selected_form_encoding::register_access::operation(semantic)
                {
                    return Ok(crate::selected_form_encoding::register_access::declaration(
                        operation, constraint,
                    ));
                }
                if semantic == Semantic::PortWrite {
                    return Ok(crate::selected_form_encoding::port_write::declaration(
                        constraint,
                    ));
                }
                if semantic == Semantic::AtomicFence {
                    return Ok(crate::selected_form_encoding::atomic_fence::declaration(
                        constraint,
                    ));
                }
                if semantic == Semantic::PrivilegedCrash {
                    return Ok(
                        crate::selected_form_encoding::crash::privileged_declaration(
                            constraints.catalog().constraint(constraint).ok_or(
                                X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch,
                            )?,
                        ),
                    );
                }
                if semantic == Semantic::Crash {
                    return Ok(crate::selected_form_encoding::crash::declaration(
                        constraints.catalog().constraint(constraint).ok_or(
                            X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch,
                        )?,
                    ));
                }
                if let Semantic::FloatResultPolicy(form) = semantic {
                    return Ok(
                        crate::selected_form_encoding::float_result_policy::declaration(
                            form, constraint,
                        ),
                    );
                }
                if let Semantic::NearestIeeeFloatArithmetic(form) = semantic {
                    return crate::selected_form_encoding::ieee_float_arithmetic::declaration(
                        form, constraint,
                    )
                    .map_err(|_| {
                        X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch
                    });
                }
                if semantic == Semantic::HostedExitProcessI32 {
                    return crate::selected_form_encoding::hosted_exit_process::declaration(
                        target, constraint,
                    )
                    .map_err(|_| {
                        X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch
                    });
                }
                if semantic == Semantic::HostedReadByte {
                    return Ok(
                        crate::selected_form_encoding::hosted_read_byte::declaration(
                            target, constraint,
                        ),
                    );
                }
                if matches!(
                    semantic,
                    Semantic::SaveFloatingControl | Semantic::RestoreFloatingControl
                ) {
                    return Ok(
                        crate::selected_form_encoding::floating_control::declaration(
                            semantic, constraint,
                        ),
                    );
                }
                if semantic == Semantic::HostedWriteByteI32 {
                    return Ok(
                        crate::selected_form_encoding::hosted_write_byte::declaration(
                            target, constraint,
                        ),
                    );
                }
                Ok(if matches!(semantic, memory::declared_semantics!()) {
                    memory::declaration(semantic, constraint, constraints)
                } else if matches!(
                    semantic,
                    Semantic::CallScalar
                        | Semantic::CallAggregate
                        | Semantic::InvokeNoNormal
                        | Semantic::NormalizedForeignCall
                ) {
                    scalar_call_declaration(semantic, constraint, constraints)
                } else if matches!(semantic, Semantic::ReturnAggregate | Semantic::ReturnScalar) {
                    let mut returned = declaration(Semantic::ReturnScalar, &selected_keys)?;
                    returned.semantic = semantic;
                    returned.constraint = constraint;
                    returned.alternatives[0].key.family = semantic.into();
                    returned
                } else {
                    declaration(semantic, &selected_keys)?
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum X86_64SelectedAbi {
    SystemV,
    Microsoft,
}

pub(crate) fn x86_64_selected_abi(
    target: NativeTarget,
) -> Result<X86_64SelectedAbi, X86_64MachineEffectCatalogValidationError> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf | ObjectFormat::MachO) => {
            Ok(X86_64SelectedAbi::SystemV)
        }
        (Architecture::X86_64, ObjectFormat::Coff) => Ok(X86_64SelectedAbi::Microsoft),
        (Architecture::Aarch64, ObjectFormat::Elf)
        | (Architecture::Aarch64, ObjectFormat::MachO)
        | (Architecture::Aarch64, ObjectFormat::Coff) => {
            Err(X86_64MachineEffectCatalogValidationError::UnsupportedTargetAbi)
        }
    }
}

pub fn x86_64_selected_constraint_keys(target: NativeTarget) -> Option<SelectedConstraintKeys> {
    let Ok(abi) = x86_64_selected_abi(target) else {
        return None;
    };
    let microsoft = abi == X86_64SelectedAbi::Microsoft;
    let return_i64 = match abi {
        X86_64SelectedAbi::SystemV => X86_64_SYSTEM_V_RETURN,
        X86_64SelectedAbi::Microsoft => X86_64_MICROSOFT_RETURN,
    };
    let return_unit = match abi {
        X86_64SelectedAbi::SystemV => X86_64_SYSTEM_V_RETURN_UNIT,
        X86_64SelectedAbi::Microsoft => X86_64_MICROSOFT_RETURN_UNIT,
    };
    Some({
        let rows = TargetRegisterEnvironmentConstraintKeys {
            port_write: Some(crate::X86_64_PORT_WRITE),
            system_register_read: Some(crate::X86_64_FLAGS_SNAPSHOT),
            system_register_write: Some(crate::X86_64_FLAGS_RESTORE),
            global_descriptor_table: Some(crate::X86_64_GLOBAL_DESCRIPTOR_TABLE),
            task_register_activate: Some(crate::X86_64_TASK_REGISTER_ACTIVATE),
            task_register_read: Some(crate::X86_64_TASK_REGISTER_READ),
            installed_interrupt_cpu_check: Some(crate::X86_64_INSTALLED_INTERRUPT_CPU_CHECK),
            installed_interrupt_task_commit: Some(crate::X86_64_INSTALLED_INTERRUPT_TASK_COMMIT),
            installed_interrupt_table_publication: Some(
                crate::X86_64_INSTALLED_INTERRUPT_TABLE_PUBLICATION,
            ),
            code_segment_reload: Some(crate::X86_64_CODE_SEGMENT_RELOAD),
            data_segment_reload: Some(crate::X86_64_DATA_SEGMENT_RELOAD),
            segment_selector_read: Some(crate::X86_64_SEGMENT_SELECTOR_READ),
            control_register_read: Some(crate::X86_64_CONTROL_REGISTER_READ),
            control_register_write: Some(crate::X86_64_CONTROL_REGISTER_WRITE),
            port_register_read: Some(crate::X86_64_PORT_REGISTER_READ),
            port_register_write: Some(crate::X86_64_PORT_REGISTER_WRITE),
            model_specific_read: Some(crate::X86_64_MODEL_SPECIFIC_READ),
            model_specific_write: Some(crate::X86_64_MODEL_SPECIFIC_WRITE),
            processor_halt: Some(crate::X86_64_PROCESSOR_HALT),
            interrupt_control: Some(crate::X86_64_INTERRUPT_CONTROL),
            memory_fence: Some(crate::X86_64_MEMORY_FENCE),
            atomic_fence: Some(crate::X86_64_ATOMIC_FENCE),
            access_read: Some(crate::X86_64_ACCESSOR_READ),
            access_write: Some(crate::X86_64_ACCESSOR_WRITE),
            atomic_load: Some(crate::X86_64_ATOMIC_LOAD),
            atomic_store: Some(crate::X86_64_ATOMIC_STORE),
            atomic_update: Some(crate::X86_64_ATOMIC_UPDATE),
            atomic_bitwise_update: Some(crate::X86_64_ATOMIC_BITWISE_UPDATE),
            atomic_compare_exchange: Some(crate::X86_64_ATOMIC_COMPARE_EXCHANGE),
            atomic_compare_exchange_once: Some(crate::X86_64_ATOMIC_COMPARE_EXCHANGE_ONCE),
            crash: crate::X86_64_CRASH,
            privileged_crash: Some(crate::X86_64_PRIVILEGED_CRASH),
            load_packed: Some(crate::X86_64_LOAD_PACKED),
            store_packed: Some(crate::X86_64_STORE_PACKED),
            call_aggregate: match abi {
                X86_64SelectedAbi::SystemV => {
                    crate::register_model::x86_64_system_v_aggregate_call_keys()
                        .into_iter()
                        .chain(crate::x86_64_system_v_mixed_aggregate_call_keys())
                        .chain(crate::x86_64_indirect_aggregate_call_keys(false))
                        .collect()
                }
                X86_64SelectedAbi::Microsoft => {
                    crate::register_model::x86_64_microsoft_aggregate_call_keys()
                        .into_iter()
                        .chain(crate::x86_64_microsoft_mixed_aggregate_call_keys())
                        .chain(crate::x86_64_indirect_aggregate_call_keys(true))
                        .collect()
                }
            },
            return_aggregate: match abi {
                X86_64SelectedAbi::SystemV => {
                    crate::register_model::x86_64_system_v_aggregate_return_keys()
                }
                X86_64SelectedAbi::Microsoft => {
                    crate::register_model::x86_64_microsoft_aggregate_return_keys()
                }
            },
            hosted_read_byte: crate::hosted_syscall_target(target)
                .then_some(crate::X86_64_HOSTED_READ_BYTE),
            hosted_exit_process_i32: crate::hosted_syscall_target(target)
                .then_some(crate::X86_64_HOSTED_EXIT_PROCESS_I32),
            hosted_write_byte_i32: crate::hosted_syscall_target(target)
                .then_some(crate::X86_64_HOSTED_WRITE_BYTE_I32),
            load64: Some(crate::X86_64_LOAD64),
            load8: Some(crate::X86_64_LOAD8),
            load16: Some(crate::X86_64_LOAD16),
            load32: Some(crate::X86_64_LOAD32),
            load8_indexed: Some(crate::X86_64_LOAD8_INDEXED),
            copy_bytes: Some(crate::X86_64_COPY_BYTES),
            zero_bytes: Some(crate::X86_64_ZERO_BYTES),
            scan_byte_sequence: Some(crate::X86_64_SCAN_BYTE_SEQUENCE),
            load_encoded_scalar: Some(crate::X86_64_LOAD_ENCODED_SCALAR),
            store_encoded_scalar: Some(crate::X86_64_STORE_ENCODED_SCALAR),
            compare_byte_sequences: Some(crate::X86_64_COMPARE_BYTE_SEQUENCES),
            save_floating_control: Some(crate::X86_64_SAVE_FLOATING_CONTROL),
            restore_floating_control: Some(crate::X86_64_RESTORE_FLOATING_CONTROL),
            store: Some(crate::X86_64_STORE),
            address_offset: Some(crate::X86_64_ADDRESS_OFFSET),
            store64: Some(crate::X86_64_STORE64),
            frame_address: Some(crate::X86_64_FRAME_ADDRESS),
            byte_literal_image_address: Some(crate::X86_64_BYTE_LITERAL_IMAGE_ADDRESS),
            call_unit_mixed: match abi {
                X86_64SelectedAbi::SystemV => crate::x86_64_system_v_mixed_unit_call_keys(),
                X86_64SelectedAbi::Microsoft => crate::x86_64_microsoft_mixed_unit_call_keys(),
            },
            call_unit: match abi {
                X86_64SelectedAbi::SystemV => crate::x86_64_system_v_register_unit_call_keys(),
                X86_64SelectedAbi::Microsoft => crate::x86_64_microsoft_register_unit_call_keys(),
            },
            call_scalar: (match abi {
                X86_64SelectedAbi::SystemV => x86_64_system_v_register_call_keys(),
                X86_64SelectedAbi::Microsoft => crate::x86_64_microsoft_register_call_keys(),
            })
            .into_iter()
            .chain(crate::x86_64_float_scalar_call_keys(microsoft))
            .collect(),
            call_normalized_foreign: (match abi {
                X86_64SelectedAbi::SystemV => crate::x86_64_system_v_normalized_foreign_call_keys(),
                X86_64SelectedAbi::Microsoft => {
                    crate::x86_64_microsoft_normalized_foreign_call_keys()
                }
            })
            .into_iter()
            .chain(
                (target.object_format == ObjectFormat::Elf)
                    .then(crate::x86_64_linux_system_call_keys)
                    .into_iter()
                    .flatten(),
            )
            .collect(),
            materialize_i64: X86_64_MATERIALIZE_I64,
            materialize_boolean: crate::X86_64_MATERIALIZE_BOOLEAN,
            copy_i64: X86_64_COPY_I64,
            float32_to_bits: Some(crate::X86_64_FLOAT32_TO_BITS),
            float64_to_bits: Some(crate::X86_64_FLOAT64_TO_BITS),
            bits_to_float32: Some(crate::X86_64_BITS_TO_FLOAT32),
            bits_to_float64: Some(crate::X86_64_BITS_TO_FLOAT64),
            ieee_float_arithmetic: Some(crate::X86_64_IEEE_FLOAT_ARITHMETIC),
            ieee_float_directed_binary: Some(crate::X86_64_IEEE_FLOAT_DIRECTED_BINARY),
            ieee_float_directed_unary: Some(crate::X86_64_IEEE_FLOAT_DIRECTED_UNARY),
            ieee_float_unary: Some(crate::X86_64_IEEE_FLOAT_UNARY),
            ieee_float_conversion_control: Some(crate::X86_64_IEEE_FLOAT_CONVERSION_CONTROL),
            ieee_float_ternary: Some(crate::X86_64_IEEE_FLOAT_TERNARY),
            float_result_trapping: Some(crate::X86_64_FLOAT_RESULT_TRAPPING),
            float_result_saturating: Some(crate::X86_64_FLOAT_RESULT_SATURATING),
            add_i64: X86_64_ADD_I64,
            subtract_i64: X86_64_SUBTRACT_I64,
            multiply_i64: crate::X86_64_MULTIPLY_I64,
            saturating_subtract_unsigned:
                crate::register_model::X86_64_SATURATING_SUBTRACT_UNSIGNED,
            saturating_add_u64: crate::register_model::X86_64_SATURATING_ADD_U64,
            divide_u64: crate::register_model::X86_64_DIVIDE_U64,
            remainder_u64: crate::register_model::X86_64_REMAINDER_U64,
            remainder_i64: crate::register_model::X86_64_REMAINDER_I64,
            divide_i64: crate::register_model::X86_64_DIVIDE_I64,
            shift_i64: crate::register_model::X86_64_SHIFT_I64,
            saturating_add_clamped: crate::register_model::X86_64_SATURATING_ADD_CLAMPED,
            saturating_subtract_clamped: crate::register_model::X86_64_SATURATING_SUBTRACT_CLAMPED,
            saturating_divide_signed: crate::register_model::X86_64_SATURATING_DIVIDE_SIGNED,
            saturating_multiply_clamped: crate::register_model::X86_64_SATURATING_MULTIPLY_CLAMPED,
            saturating_multiply_u64: crate::register_model::X86_64_SATURATING_MULTIPLY_U64,
            trapping_binary: crate::X86_64_TRAPPING_BINARY,
            trapping_fixed_pair: crate::X86_64_TRAPPING_FIXED_PAIR,
            trapping_shift: crate::X86_64_TRAPPING_SHIFT,
            trapping_convert: crate::X86_64_TRAPPING_CONVERT,
            add_i64_immediate: X86_64_ADD_I64_IMMEDIATE,
            subtract_i64_immediate: X86_64_SUBTRACT_I64_IMMEDIATE,
            compare_i64_zero: X86_64_COMPARE_I64_ZERO,
            compare_i64: X86_64_COMPARE_I64,
            compare_i64_immediate: X86_64_COMPARE_I64_IMMEDIATE,
            conditional_branch: X86_64_CONDITIONAL_BRANCH,
            jump: crate::X86_64_JUMP,
            return_float: crate::x86_64_float_scalar_return_keys(microsoft),
            return_i64,
            return_unit,
        };
        let mut completed = SelectedConstraintKeys {
            rows,
            invoke_no_normal: Vec::new(),
        };
        completed.invoke_no_normal = completed.ordinary_call_constraints();
        completed
    })
}

fn declaration(
    semantic: Semantic,
    keys: &SelectedConstraintKeys,
) -> Result<MachineEffectDeclaration, X86_64MachineEffectCatalogValidationError> {
    let always = |width| alternative(semantic, 0, MachineAlternativeApplicability::Always, width);
    let alternatives = match semantic {
        Semantic::ExactDivideU64
        | Semantic::ExactRemainderU64
        | Semantic::WrappingRemainderI64
        | Semantic::WrappingDivideI64
        | Semantic::ExactDivideI64
        | Semantic::ExactRemainderI64
        | Semantic::SaturatingAdd(_)
        | Semantic::SaturatingSubtract(_)
        | Semantic::SaturatingDivide(_)
        | Semantic::SaturatingRemainder(_)
        | Semantic::SaturatingMultiply(_)
        | Semantic::TrappingInteger(_)
        | Semantic::BitwiseAndI64
        | Semantic::BitwiseOrI64
        | Semantic::BitwiseNotI64
        | Semantic::BitwiseXorI64
        | Semantic::ByteViewAddress
        | Semantic::WrappingAddI64
        | Semantic::ExactAddI64 => {
            vec![always(size(semantic))?]
        }
        Semantic::ExactSubtractI64 | Semantic::WrappingSubtractI64 => {
            binary_alias_ladder(semantic, [3, 3, 6, 6])?
        }
        Semantic::ExactMultiplyI64 | Semantic::WrappingMultiplyI64 => {
            binary_alias_ladder(semantic, [4, 4, 4, 7])?
        }
        Semantic::ConditionalBranchU64LessThan | Semantic::ConditionalBranchI64LessThan => {
            vec![always(Size::ExactBytes(6))?]
        }
        Semantic::WrappingShiftLeftI64
        | Semantic::WrappingShiftRightI64
        | Semantic::WrappingShiftRightU64
        | Semantic::ExactShiftLeftI64
        | Semantic::ExactShiftRightI64
        | Semantic::ExactShiftRightU64 => {
            vec![always(Size::ExactBytes(6))?]
        }
        Semantic::MaterializeI64 => vec![
            always(size(semantic))?,
            alternative(
                semantic,
                1,
                MachineAlternativeApplicability::OptimizationSelected {
                    optimization:
                        optimization_core::Optimization::X86SelectXorZeroI64MaterializationV1,
                },
                Size::ExactBytes(3),
            )?,
        ],
        _ => vec![always(size(semantic))?],
    };
    Ok(MachineEffectDeclaration {
        semantic,
        constraint: keys
            .for_semantic(semantic)
            .ok_or(X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch)?,
        memory: if matches!(semantic, Semantic::AtomicAccess(_)) {
            MachineMemoryEffect::AtomicPointerV1
        } else {
            MachineMemoryEffect::NoneV1
        },
        trap: if matches!(semantic, Semantic::TrappingInteger(_)) {
            MachineTrapBehavior::TrappingIntegerV1
        } else if matches!(semantic, Semantic::AtomicAccess(_)) {
            MachineTrapBehavior::MayArchitecturalFaultV1
        } else {
            MachineTrapBehavior::NeverV1
        },
        barrier: if matches!(
            semantic,
            Semantic::ConditionalBranchNonZero
                | Semantic::ConditionalBranchU64LessThan
                | Semantic::ConditionalBranchI64LessThan
                | Semantic::ReturnScalar
                | Semantic::ReturnAggregate
                | Semantic::Jump
                | Semantic::ReturnUnit
        ) {
            MachineBarrier::ControlFlow
        } else if matches!(
            semantic,
            Semantic::TrappingInteger(_) | Semantic::AtomicAccess(_)
        ) {
            MachineBarrier::ExternalEffect
        } else {
            MachineBarrier::None
        },
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives,
    })
}

fn binary_alias_ladder(
    semantic: Semantic,
    sizes: [u16; 4],
) -> Result<Vec<MachineAlternative>, X86_64MachineEffectCatalogValidationError> {
    Ok(vec![
        alternative(
            semantic,
            0,
            MachineAlternativeApplicability::ResultAliasesOperands {
                result: 2,
                left: 0,
                right: 1,
            },
            Size::ExactBytes(sizes[0]),
        )?,
        alternative(
            semantic,
            1,
            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                result: 2,
                aliased_operand: 0,
                distinct_operand: 1,
            },
            Size::ExactBytes(sizes[1]),
        )?,
        alternative(
            semantic,
            2,
            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                result: 2,
                aliased_operand: 1,
                distinct_operand: 0,
            },
            Size::ExactBytes(sizes[2]),
        )?,
        alternative(
            semantic,
            3,
            MachineAlternativeApplicability::ResultDistinctFromOperands {
                result: 2,
                left: 0,
                right: 1,
            },
            Size::ExactBytes(sizes[3]),
        )?,
    ])
}

fn alternative(
    semantic: Semantic,
    variant: u32,
    applicability: MachineAlternativeApplicability,
    size: Size,
) -> Result<MachineAlternative, X86_64MachineEffectCatalogValidationError> {
    Ok(MachineAlternative {
        key: MachineAlternativeKey {
            family: semantic.into(),
            variant,
        },
        applicability,
        size,
        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
        encoded: encoded_effects(semantic, variant)?,
    })
}

/// Classifies operands and implicit effects together: a new semantic must choose both.
fn encoded_effects(
    semantic: Semantic,
    variant: u32,
) -> Result<MachineEncodedEffects, X86_64MachineEffectCatalogValidationError> {
    let plain = MachineEncodedEffects::fallthrough_v1;
    let clobber_flags = |reads, writes| {
        let mut effects = plain(reads, writes);
        effects.implicit_unit_clobbers = units("condition_flags");
        effects
    };
    Ok(match semantic {
        Semantic::Crash => MachineEncodedEffects::crash_v1(&units("rip")),
        Semantic::PrivilegedCrash => {
            MachineEncodedEffects::privileged_stop_v1(&units("rip"), &units("control_flags"))
        }
        Semantic::TrappingInteger(form) => TrappingShape::of(form).encoded_effects(),
        Semantic::NearestIeeeFloatArithmetic(form) => {
            crate::selected_form_encoding::ieee_float_arithmetic::effects(form)
        }
        Semantic::FloatResultPolicy(form) => {
            crate::selected_form_encoding::float_result_policy::effects(form)
        }
        Semantic::AtomicAccess(form) => crate::selected_form_encoding::atomic::effects(form),
        Semantic::InstalledInterruptCpuCheck
        | Semantic::InstalledInterruptTaskCommit
        | Semantic::InstalledInterruptTablePublication => {
            let form =
                crate::selected_form_encoding::installed_interrupt::Form::from_semantic(semantic)
                    .ok_or(X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch)?;
            crate::selected_form_encoding::installed_interrupt::effects(form)
        }
        Semantic::PortRegisterRead
        | Semantic::PortRegisterWrite
        | Semantic::ModelSpecificRead
        | Semantic::ModelSpecificWrite
        | Semantic::ProcessorHalt
        | Semantic::InterruptControl
        | Semantic::ControlRegisterRead
        | Semantic::ControlRegisterWrite
        | Semantic::TaskRegisterActivate
        | Semantic::TaskRegisterRead
        | Semantic::CodeSegmentReload
        | Semantic::DataSegmentReload
        | Semantic::SegmentSelectorRead
        | Semantic::GlobalDescriptorTableLoad
        | Semantic::GlobalDescriptorTableStore
        | Semantic::SystemRegisterRead
        | Semantic::SystemRegisterWrite => {
            let operation = crate::selected_form_encoding::register_access::operation(semantic)
                .ok_or(X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch)?;
            crate::selected_form_encoding::register_access::effects(operation)
        }
        Semantic::PortWrite => crate::selected_form_encoding::port_write::effects(),
        Semantic::MemoryFence => MachineEncodedEffects::catalog_fence_v1(),
        Semantic::AtomicFence => MachineEncodedEffects::atomic_fence_v1(),
        Semantic::CompareI64Zero | Semantic::CompareI64Immediate | Semantic::CompareI64 => {
            let reads = if semantic == Semantic::CompareI64 {
                vec![0, 1]
            } else {
                vec![0]
            };
            let mut effects = plain(reads, vec![]);
            effects.implicit_unit_defs = units("condition_flags");
            effects
        }
        Semantic::MaterializeBooleanEqual
        | Semantic::MaterializeBooleanU64LessThan
        | Semantic::MaterializeBooleanI64LessThan
        | Semantic::MaterializeBooleanU64LessOrEqual
        | Semantic::MaterializeBooleanI64LessOrEqual => {
            let mut effects = plain(vec![], vec![0]);
            effects.implicit_unit_uses = units("condition_flags");
            effects
        }
        Semantic::MaterializeI64 if variant == 1 => clobber_flags(vec![], vec![0]),
        Semantic::MaterializeI64 | Semantic::ImageAddress => plain(vec![], vec![0]),
        Semantic::Float32ToBits
        | Semantic::Float64ToBits
        | Semantic::BitsToFloat32
        | Semantic::BitsToFloat64
        | Semantic::CopyI64
        | Semantic::ZeroExtendU8
        | Semantic::ZeroExtendU16
        | Semantic::SignExtendI8
        | Semantic::SignExtendI16
        | Semantic::SignExtendI32
        | Semantic::ZeroExtendU32
        | Semantic::BitwiseNotI64
        | Semantic::ExactAddI64Immediate
        | Semantic::ExactSubtractI64Immediate => plain(vec![0], vec![1]),
        Semantic::ByteViewAddress | Semantic::WrappingAddI64 | Semantic::ExactAddI64 => {
            plain(vec![0, 1], vec![2])
        }
        Semantic::ExactDivideU64 => {
            let mut effects = clobber_flags(vec![0, 1, 3], vec![2]);
            effects.implicit_unit_clobbers = units_of(&["rdx", "condition_flags"]);
            effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
            effects
        }
        Semantic::WrappingRemainderI64
        | Semantic::ExactRemainderU64
        | Semantic::WrappingDivideI64
        | Semantic::ExactDivideI64
        | Semantic::ExactRemainderI64 => {
            let mut effects = clobber_flags(vec![0, 1], vec![2, 3]);
            effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
            effects
        }
        Semantic::SaturatingAdd(carrier)
        | Semantic::SaturatingSubtract(carrier)
        | Semantic::SaturatingDivide(carrier)
        | Semantic::SaturatingRemainder(carrier)
        | Semantic::SaturatingMultiply(carrier) => {
            let (reads, writes) = saturating_form(semantic, carrier).operand_reads_and_writes();
            let mut effects = clobber_flags(reads, writes);
            if matches!(
                semantic,
                Semantic::SaturatingDivide(_) | Semantic::SaturatingRemainder(_)
            ) {
                effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
            }
            if matches!(semantic, Semantic::SaturatingDivide(_)) {
                effects.implicit_unit_clobbers = units_of(&["rdx", "condition_flags"]);
            }
            effects
        }
        Semantic::ExactSubtractI64 | Semantic::WrappingSubtractI64 if variant == 0 => {
            clobber_flags(vec![], vec![2])
        }
        Semantic::BitwiseAndI64
        | Semantic::BitwiseOrI64
        | Semantic::BitwiseXorI64
        | Semantic::ExactSubtractI64
        | Semantic::WrappingSubtractI64
        | Semantic::ExactMultiplyI64
        | Semantic::WrappingMultiplyI64
        | Semantic::WrappingShiftLeftI64
        | Semantic::WrappingShiftRightI64
        | Semantic::WrappingShiftRightU64
        | Semantic::ExactShiftLeftI64
        | Semantic::ExactShiftRightI64
        | Semantic::ExactShiftRightU64 => clobber_flags(vec![0, 1], vec![2]),
        Semantic::Jump
        | Semantic::ConditionalBranchNonZero
        | Semantic::ConditionalBranchU64LessThan
        | Semantic::ConditionalBranchI64LessThan => {
            let mut effects = plain(vec![], vec![]);
            effects.implicit_unit_uses = if semantic == Semantic::Jump {
                units("rip")
            } else {
                units_of(&["condition_flags", "rip"])
            };
            effects.implicit_unit_defs = units("rip");
            effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
            effects.control = if semantic == Semantic::Jump {
                MachineEncodedControlEffect::UnconditionalRelativeBranchV1
            } else {
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            };
            effects
        }
        Semantic::ReturnScalar | Semantic::ReturnAggregate | Semantic::ReturnUnit => {
            let stack_pointer = view_of("rsp").id;
            let mut effects = plain(vec![], vec![]);
            effects.implicit_unit_uses = units("rsp");
            effects.implicit_unit_defs = units_of(&["rsp", "rip"]);
            effects.memory = MachineEncodedMemoryEffect::ReadActivationStackV1 {
                stack_pointer,
                byte_count: 8,
            };
            effects.stack = MachineEncodedStackEffect::PopBytesV1 {
                stack_pointer,
                byte_count: 8,
            };
            effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
            effects.control = MachineEncodedControlEffect::ReturnFromActivationStackV1;
            effects
        }
        // These forms need their declaration's exact memory or call context.
        Semantic::CallScalar
        | Semantic::InvokeNoNormal
        | Semantic::CallAggregate
        | Semantic::NormalizedForeignCall
        | Semantic::HostedExitProcessI32
        | Semantic::HostedReadByte
        | Semantic::SaveFloatingControl
        | Semantic::RestoreFloatingControl
        | Semantic::HostedWriteByteI32
        | memory::declared_semantics!() => {
            return Err(X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch);
        }
    })
}

fn saturating_form(semantic: Semantic, carrier: SaturatingCarrier) -> SaturatingForm {
    let operation = match semantic {
        Semantic::SaturatingAdd(_) => SaturatingOperation::Add,
        Semantic::SaturatingSubtract(_) => SaturatingOperation::Subtract,
        Semantic::SaturatingDivide(_) => SaturatingOperation::Divide,
        Semantic::SaturatingRemainder(_) => SaturatingOperation::Remainder,
        Semantic::SaturatingMultiply(_) => SaturatingOperation::Multiply,
        _ => unreachable!("saturating semantics carry their operation"),
    };
    SaturatingForm::of(operation, carrier)
}

fn size(semantic: Semantic) -> Size {
    match semantic {
        Semantic::PortRegisterRead
        | Semantic::PortRegisterWrite
        | Semantic::ModelSpecificRead
        | Semantic::ModelSpecificWrite
        | Semantic::SystemRegisterRead
        | Semantic::SystemRegisterWrite
        | Semantic::ControlRegisterRead
        | Semantic::GlobalDescriptorTableLoad
        | Semantic::GlobalDescriptorTableStore
        | Semantic::ControlRegisterWrite => crate::encoder_resolved(1, Some(40)),
        Semantic::TaskRegisterActivate => crate::encoder_resolved(1, Some(512)),
        Semantic::TaskRegisterRead => crate::encoder_resolved(1, Some(8)),
        Semantic::InstalledInterruptCpuCheck
        | Semantic::InstalledInterruptTaskCommit
        | Semantic::InstalledInterruptTablePublication => crate::encoder_resolved(1, Some(2048)),
        Semantic::CodeSegmentReload | Semantic::DataSegmentReload => {
            crate::encoder_resolved(1, Some(512))
        }
        Semantic::SegmentSelectorRead => crate::encoder_resolved(1, Some(7)),
        Semantic::AccessorScalarAccess(_) => crate::encoder_resolved(7, Some(9)),
        Semantic::PortWrite => Size::ExactBytes(x86_encoding::IMMEDIATE_PORT_WRITE_WIDTH as u16),
        Semantic::AtomicFence | Semantic::MemoryFence => Size::ExactBytes(3),
        Semantic::ProcessorHalt | Semantic::InterruptControl => Size::ExactBytes(1),
        Semantic::FloatResultPolicy(form) => {
            crate::selected_form_encoding::float_result_policy::size(form)
        }
        Semantic::AtomicAccess(_) => crate::encoder_resolved(1, None),
        Semantic::NearestIeeeFloatArithmetic(_) => {
            unreachable!("float arithmetic uses its dedicated declaration")
        }
        Semantic::Crash => Size::ExactBytes(2),
        Semantic::PrivilegedCrash => Size::ExactBytes(4),
        Semantic::MaterializeBooleanEqual
        | Semantic::MaterializeBooleanU64LessThan
        | Semantic::MaterializeBooleanI64LessThan
        | Semantic::MaterializeBooleanU64LessOrEqual
        | Semantic::MaterializeBooleanI64LessOrEqual => Size::ExactBytes(8),
        Semantic::Jump => Size::ExactBytes(5),
        Semantic::CompareI64Zero | Semantic::CompareI64 | Semantic::CopyI64 => Size::ExactBytes(3),
        Semantic::CompareI64Immediate => Size::ExactBytes(7),
        Semantic::Float32ToBits | Semantic::BitsToFloat32 => crate::encoder_resolved(4, Some(5)),
        Semantic::Float64ToBits | Semantic::BitsToFloat64 => Size::ExactBytes(5),
        Semantic::ZeroExtendU8
        | Semantic::ZeroExtendU16
        | Semantic::SignExtendI8
        | Semantic::SignExtendI16 => Size::ExactBytes(4),
        Semantic::ZeroExtendU32 | Semantic::SignExtendI32 => Size::ExactBytes(3),
        Semantic::MaterializeI64 => Size::ExactBytes(10),
        Semantic::ImageAddress => Size::ExactBytes(7),
        Semantic::ExactDivideU64 => Size::ExactBytes(3),
        Semantic::WrappingRemainderI64 => Size::ExactBytes(17),
        Semantic::ExactRemainderU64 => Size::ExactBytes(9),
        Semantic::WrappingDivideI64 => Size::ExactBytes(16),
        Semantic::ExactDivideI64 => Size::ExactBytes(5),
        Semantic::ExactRemainderI64 => Size::ExactBytes(8),
        Semantic::SaturatingAdd(carrier)
        | Semantic::SaturatingSubtract(carrier)
        | Semantic::SaturatingDivide(carrier)
        | Semantic::SaturatingRemainder(carrier)
        | Semantic::SaturatingMultiply(carrier) => {
            Size::ExactBytes(saturating_form(semantic, carrier).byte_count())
        }
        Semantic::TrappingInteger(form) => Size::ExactBytes(TrappingShape::of(form).byte_count()),
        Semantic::BitwiseAndI64
        | Semantic::BitwiseOrI64
        | Semantic::BitwiseNotI64
        | Semantic::BitwiseXorI64 => crate::encoder_resolved(3, Some(6)),
        Semantic::ByteViewAddress | Semantic::WrappingAddI64 | Semantic::ExactAddI64 => {
            crate::encoder_resolved(4, Some(5))
        }
        Semantic::ExactAddI64Immediate => crate::encoder_resolved(4, Some(8)),
        Semantic::ExactSubtractI64Immediate => crate::encoder_resolved(4, Some(8)),
        Semantic::ConditionalBranchNonZero
        | Semantic::ConditionalBranchU64LessThan
        | Semantic::ConditionalBranchI64LessThan => crate::encoder_resolved(2, Some(6)),
        Semantic::ReturnScalar | Semantic::ReturnAggregate | Semantic::ReturnUnit => {
            Size::ExactBytes(1)
        }
        Semantic::ExactSubtractI64
        | Semantic::WrappingSubtractI64
        | Semantic::ExactMultiplyI64
        | Semantic::WrappingMultiplyI64
        | Semantic::WrappingShiftLeftI64
        | Semantic::WrappingShiftRightI64
        | Semantic::WrappingShiftRightU64
        | Semantic::ExactShiftLeftI64
        | Semantic::ExactShiftRightI64
        | Semantic::ExactShiftRightU64 => {
            unreachable!("subtraction, multiplication, and shifts declare explicit alternatives")
        }
        #[allow(unreachable_patterns)]
        Semantic::CallScalar
        | Semantic::InvokeNoNormal
        | Semantic::CallAggregate
        | Semantic::NormalizedForeignCall
        | Semantic::HostedExitProcessI32
        | Semantic::HostedReadByte
        | Semantic::SaveFloatingControl
        | Semantic::RestoreFloatingControl
        | Semantic::HostedWriteByteI32
        | memory::declared_semantics!() => {
            unreachable!("semantics with a dedicated declaration have no generic size")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Architecture, MachineAlternativeApplicability, MachineBarrier, MachineCallEffect,
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
        MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
        MachineTrapBehavior, NativeTarget, ObjectFormat, Semantic, Size,
        ValidatedRegisterConstraintCatalog, X86_64_CONDITIONAL_BRANCH, X86_64_COPY_I64,
        X86_64_MICROSOFT_RETURN, X86_64_MICROSOFT_RETURN_UNIT, X86_64_SUBTRACT_I64,
        X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT,
        X86_64MachineEffectCatalogValidationError, validate_x86_64_machine_effect_catalog,
        x86_64_machine_effect_catalog, x86_64_system_v_register_call_keys,
    };
    use crate::{
        X86_64RegisterConstraintCatalogValidationError,
        validate_x86_64_register_constraint_catalog, x86_64_physical_register_model,
        x86_64_register_constraint_catalog,
    };
    use register_model::validate_physical_register_model;
    use selected_instructions::{
        IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatToIntegerDomain,
        MachineAlternativeFamily, MachineEffectCatalogValidationError, SaturatingCarrier,
        SaturatingOperation, TrappingForm, TrappingOperation,
    };

    #[test]
    fn effect_classification_keeps_operand_and_implicit_effects_together() {
        let comparison = super::encoded_effects(Semantic::CompareI64, 0).unwrap();
        assert_eq!(comparison.external_operand_reads, [0, 1]);
        assert_eq!(
            comparison.implicit_unit_defs,
            super::units("condition_flags")
        );

        let subtract = super::encoded_effects(Semantic::WrappingSubtractI64, 0).unwrap();
        assert!(subtract.external_operand_reads.is_empty());
        assert_eq!(subtract.external_operand_writes, [2]);
        assert_eq!(
            subtract.implicit_unit_clobbers,
            super::units("condition_flags")
        );

        // LEA-based addition and ordinary moves preserve the comparison flags.
        for semantic in [Semantic::ExactAddI64, Semantic::CopyI64] {
            let effects = super::encoded_effects(semantic, 0).unwrap();
            assert!(effects.implicit_unit_uses.is_empty());
            assert!(effects.implicit_unit_defs.is_empty());
            assert!(effects.implicit_unit_clobbers.is_empty());
        }
        let zero = super::encoded_effects(Semantic::MaterializeI64, 1).unwrap();
        assert_eq!(zero.implicit_unit_clobbers, super::units("condition_flags"));

        let returned = super::encoded_effects(Semantic::ReturnScalar, 0).unwrap();
        assert_eq!(
            returned.control,
            MachineEncodedControlEffect::ReturnFromActivationStackV1
        );
        assert!(matches!(
            returned.stack,
            MachineEncodedStackEffect::PopBytesV1 { byte_count: 8, .. }
        ));
    }

    #[test]
    fn effect_classification_requires_context_for_calls_and_memory_forms() {
        for semantic in [
            Semantic::CallScalar,
            Semantic::NormalizedForeignCall,
            Semantic::Load64,
        ] {
            assert!(matches!(
                super::encoded_effects(semantic, 0),
                Err(X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch)
            ));
        }
    }

    fn constraints() -> ValidatedRegisterConstraintCatalog {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        validate_x86_64_register_constraint_catalog(
            x86_64_register_constraint_catalog(&physical),
            &physical,
        )
        .unwrap_or_else(|error: X86_64RegisterConstraintCatalogValidationError| panic!("{error}"))
    }

    macro_rules! eq {
        ($left:expr => $right:expr) => {
            assert_eq!($left, $right);
        };
    }

    #[test]
    fn raw_bit_classification_catalog_admits_real_flag_clobbers() {
        let constraints = constraints();
        let catalog =
            x86_64_machine_effect_catalog(NativeTarget::linux_x64(), &constraints).unwrap();
        let physical = x86_64_physical_register_model();
        let flags = &physical.view_named("condition_flags").unwrap().units;
        for format in [
            selected_instructions::IeeeFloatFormat::Binary32,
            selected_instructions::IeeeFloatFormat::Binary64,
        ] {
            for operation in [
                IeeeFloatArithmeticOperation::IsNan,
                IeeeFloatArithmeticOperation::IsInfinite,
                IeeeFloatArithmeticOperation::IsFinite,
                IeeeFloatArithmeticOperation::IsNormal,
                IeeeFloatArithmeticOperation::IsSubnormal,
                IeeeFloatArithmeticOperation::Classify,
            ] {
                let row = catalog
                    .declaration_by_semantic(Semantic::NearestIeeeFloatArithmetic(
                        IeeeFloatArithmeticForm { operation, format },
                    ))
                    .unwrap();
                assert_eq!(row.constraint, crate::X86_64_IEEE_FLOAT_CONVERSION_CONTROL);
                let encoded = &row.alternatives[0].encoded;
                assert!(
                    flags
                        .iter()
                        .all(|unit| encoded.implicit_unit_clobbers.contains(unit))
                );
                assert_eq!(encoded.external_operand_reads, [0]);
                assert_eq!(encoded.external_operand_writes, [1]);
            }
        }
    }

    #[test]
    fn saturating_catalog_binds_key_size_and_effects_for_every_carrier() {
        use SaturatingCarrier::{I64, U64};
        use SaturatingOperation::{Add, Divide, Multiply, Remainder, Subtract};
        let constraints = constraints();
        let physical = x86_64_physical_register_model();
        let rflags = physical
            .view_named("condition_flags")
            .unwrap()
            .units
            .clone();
        let rdx = physical.view_named("rdx").unwrap().units.clone();
        for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
            let catalog = x86_64_machine_effect_catalog(target, &constraints).unwrap();
            for operation in [Add, Subtract, Divide, Remainder, Multiply] {
                for carrier in SaturatingCarrier::ALL {
                    let semantic = match operation {
                        Add => Semantic::SaturatingAdd(carrier),
                        Subtract => Semantic::SaturatingSubtract(carrier),
                        Divide => Semantic::SaturatingDivide(carrier),
                        Remainder => Semantic::SaturatingRemainder(carrier),
                        Multiply => Semantic::SaturatingMultiply(carrier),
                    };
                    let (key, size) = match (operation, carrier.is_signed(), carrier) {
                        (Add, _, U64) => (crate::register_model::X86_64_SATURATING_ADD_U64, 19),
                        (Add, true, I64) => {
                            (crate::register_model::X86_64_SATURATING_ADD_CLAMPED, 25)
                        }
                        (Add, true, _) => {
                            (crate::register_model::X86_64_SATURATING_ADD_CLAMPED, 40)
                        }
                        (Add, false, _) => {
                            (crate::register_model::X86_64_SATURATING_ADD_CLAMPED, 23)
                        }
                        (Subtract, false, _) => (
                            crate::register_model::X86_64_SATURATING_SUBTRACT_UNSIGNED,
                            13,
                        ),
                        (Subtract, true, I64) => (
                            crate::register_model::X86_64_SATURATING_SUBTRACT_CLAMPED,
                            25,
                        ),
                        (Subtract, true, _) => (
                            crate::register_model::X86_64_SATURATING_SUBTRACT_CLAMPED,
                            40,
                        ),
                        (Divide, false, _) => (crate::register_model::X86_64_DIVIDE_U64, 3),
                        (Divide, true, I64) => {
                            (crate::register_model::X86_64_SATURATING_DIVIDE_SIGNED, 26)
                        }
                        (Divide, true, _) => {
                            (crate::register_model::X86_64_SATURATING_DIVIDE_SIGNED, 22)
                        }
                        (Remainder, false, _) => (crate::register_model::X86_64_REMAINDER_U64, 9),
                        (Remainder, true, _) => (crate::register_model::X86_64_REMAINDER_I64, 17),
                        (Multiply, _, U64) => {
                            (crate::register_model::X86_64_SATURATING_MULTIPLY_U64, 9)
                        }
                        (Multiply, true, I64) => (
                            crate::register_model::X86_64_SATURATING_MULTIPLY_CLAMPED,
                            29,
                        ),
                        (Multiply, true, _) => (
                            crate::register_model::X86_64_SATURATING_MULTIPLY_CLAMPED,
                            41,
                        ),
                        (Multiply, false, _) => (
                            crate::register_model::X86_64_SATURATING_MULTIPLY_CLAMPED,
                            24,
                        ),
                    };
                    let division = operation == Divide;
                    let remainder = operation == Remainder;
                    let three_operand = (operation, carrier) == (Add, U64)
                        || (operation == Subtract && !carrier.is_signed());
                    let (reads, writes): (Vec<u16>, Vec<u16>) = if division {
                        (vec![0, 1, 3], vec![2])
                    } else if three_operand {
                        (vec![0, 1], vec![2])
                    } else {
                        (vec![0, 1], vec![2, 3])
                    };
                    let declaration = catalog
                        .declaration_by_semantic(semantic)
                        .unwrap_or_else(|| panic!("{semantic:?} declared"));
                    assert_eq!(declaration.constraint, key, "{semantic:?}");
                    assert_eq!(declaration.alternatives.len(), 1);
                    let alternative = &declaration.alternatives[0];
                    assert_eq!(
                        alternative.key.family,
                        selected_instructions::MachineAlternativeFamily::from(semantic)
                    );
                    assert_eq!(alternative.size, Size::ExactBytes(size), "{semantic:?}");
                    eq!(alternative.applicability => MachineAlternativeApplicability::Always);
                    assert_eq!(
                        alternative.encoded.external_operand_reads, reads,
                        "{semantic:?}"
                    );
                    assert_eq!(
                        alternative.encoded.external_operand_writes, writes,
                        "{semantic:?}"
                    );
                    let mut clobbers = rflags.clone();
                    if division {
                        clobbers.extend(rdx.iter().copied());
                        clobbers.sort_unstable();
                        clobbers.dedup();
                    }
                    assert_eq!(alternative.encoded.implicit_unit_clobbers, clobbers);
                    assert_eq!(
                        alternative.encoded.trap
                            == MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                        division || remainder,
                        "{semantic:?}"
                    );
                    if target != NativeTarget::linux_x64() {
                        continue;
                    }
                    for corruption in 0..4 {
                        let mut changed = catalog.clone();
                        let alternative = &mut changed
                            .declaration_by_semantic_mut(semantic)
                            .unwrap()
                            .alternatives[0];
                        match corruption {
                            0 if division => alternative.encoded.external_operand_reads.truncate(2),
                            0 if three_operand => {
                                alternative.encoded.external_operand_reads.truncate(1);
                            }
                            0 => alternative.encoded.external_operand_writes.truncate(1),
                            1 => alternative.encoded.implicit_unit_clobbers.clear(),
                            2 => alternative.size = Size::ExactBytes(size + 1),
                            _ => {
                                let sibling =
                                    SaturatingCarrier::ALL[(usize::from(carrier.ordinal()) + 1)
                                        % SaturatingCarrier::ALL.len()];
                                alternative.key.family = match operation {
                                Add => selected_instructions::MachineAlternativeFamily::SaturatingAdd(sibling),
                                Subtract => selected_instructions::MachineAlternativeFamily::SaturatingSubtract(sibling),
                                Divide => selected_instructions::MachineAlternativeFamily::SaturatingDivide(sibling),
                                Remainder => selected_instructions::MachineAlternativeFamily::SaturatingRemainder(sibling),
                                Multiply => selected_instructions::MachineAlternativeFamily::SaturatingMultiply(sibling),
                            };
                            }
                        }
                        assert!(
                            validate_x86_64_machine_effect_catalog(target, &constraints, changed)
                                .is_err(),
                            "{semantic:?} corruption {corruption}"
                        );
                    }
                }
            }
        }
    }

    const TRAPPING_SIZES: [[u16; 8]; 9] = [
        [17, 17, 16, 10, 17, 17, 17, 10],
        [17, 17, 16, 10, 17, 17, 17, 10],
        [18, 18, 17, 11, 18, 18, 18, 7],
        [26, 28, 28, 26, 13, 13, 13, 13],
        [29, 31, 31, 29, 16, 16, 16, 16],
        [25, 25, 24, 27, 25, 25, 25, 27],
        [14, 14, 14, 14, 14, 14, 14, 14],
        [14, 14, 13, 3, 14, 14, 14, 14],
        [14, 14, 14, 14, 14, 14, 14, 3],
    ];

    #[test]
    fn trapping_catalog_binds_row_size_and_trap_effects_for_every_form() {
        let constraints = constraints();
        let rflags = x86_64_physical_register_model()
            .view_named("condition_flags")
            .unwrap()
            .units
            .clone();
        for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
            let catalog = x86_64_machine_effect_catalog(target, &constraints).unwrap();
            validate_x86_64_machine_effect_catalog(target, &constraints, catalog.clone()).unwrap();
            for form in TrappingForm::ALL {
                let semantic = Semantic::TrappingInteger(form);
                let (key, reads, writes): (_, Vec<u16>, Vec<u16>) =
                    match (form.operation, form.carrier) {
                        (TrappingOperation::Divide | TrappingOperation::Remainder, _)
                        | (TrappingOperation::Multiply, SaturatingCarrier::U64) => {
                            (crate::X86_64_TRAPPING_FIXED_PAIR, vec![0, 1], vec![2, 3])
                        }
                        (TrappingOperation::ShiftLeft | TrappingOperation::ShiftRight, _) => {
                            (crate::X86_64_TRAPPING_SHIFT, vec![0, 1], vec![2, 3])
                        }
                        (TrappingOperation::Convert { .. }, _) => {
                            (crate::X86_64_TRAPPING_CONVERT, vec![0], vec![1, 2])
                        }
                        _ => (crate::X86_64_TRAPPING_BINARY, vec![0, 1], vec![2, 3]),
                    };
                let size = TRAPPING_SIZES[usize::from(form.operation.ordinal())]
                    [usize::from(form.carrier.ordinal())];
                let declaration = catalog
                    .declaration_by_semantic(semantic)
                    .unwrap_or_else(|| panic!("{form:?} declared"));
                assert_eq!(declaration.constraint, key, "{form:?}");
                assert_eq!(declaration.barrier, MachineBarrier::ExternalEffect);
                assert_eq!(declaration.trap, MachineTrapBehavior::TrappingIntegerV1);
                assert_eq!(declaration.memory, MachineMemoryEffect::NoneV1);
                assert_eq!(declaration.call, MachineCallEffect::NoneV1);
                assert_eq!(declaration.alternatives.len(), 1);
                let alternative = &declaration.alternatives[0];
                eq!(alternative.key.family => MachineAlternativeFamily::TrappingInteger(form));
                assert_eq!(alternative.key.variant, 0);
                eq!(alternative.applicability => MachineAlternativeApplicability::Always);
                assert_eq!(alternative.size, Size::ExactBytes(size), "{form:?}");
                assert_eq!(
                    alternative.encoded,
                    MachineEncodedEffects {
                        external_operand_reads: reads,
                        external_operand_writes: writes,
                        implicit_unit_uses: vec![],
                        implicit_unit_defs: vec![],
                        implicit_unit_clobbers: rflags.clone(),
                        memory: MachineEncodedMemoryEffect::NoneV1,
                        stack: MachineEncodedStackEffect::UnchangedV1,
                        trap: MachineEncodedTrapBehavior::TrappingIntegerV1,
                        control: MachineEncodedControlEffect::FallThroughOrTrapV1,
                    },
                    "{form:?}"
                );
                if target != NativeTarget::linux_x64() {
                    continue;
                }
                let corruption = form.carrier.ordinal();
                let mut changed = catalog.clone();
                let row = changed.declaration_by_semantic_mut(semantic).unwrap();
                let alternative = &mut row.alternatives[0];
                match corruption {
                    0 => alternative.encoded.external_operand_writes.truncate(1),
                    1 => alternative.encoded.implicit_unit_clobbers.clear(),
                    2 => alternative.size = Size::ExactBytes(size + 1),
                    3 => alternative.encoded.trap = MachineEncodedTrapBehavior::NeverV1,
                    4 => alternative.encoded.control = MachineEncodedControlEffect::FallThroughV1,
                    5 => {
                        alternative.key.family = MachineAlternativeFamily::TrappingInteger(
                            TrappingForm::ALL
                                [(usize::from(form.ordinal()) + 1) % TrappingForm::COUNT],
                        );
                    }
                    6 => row.trap = MachineTrapBehavior::NeverV1,
                    _ => row.barrier = MachineBarrier::None,
                }
                assert!(
                    validate_x86_64_machine_effect_catalog(target, &constraints, changed).is_err(),
                    "{form:?} corruption {corruption}"
                );
            }
        }
    }

    #[test]
    fn wrapping_remainder_catalog_defines_scratch_and_clobbers_flags() {
        let constraints = constraints();
        for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
            let catalog = x86_64_machine_effect_catalog(target, &constraints).unwrap();
            let declaration = catalog
                .declaration_by_semantic(Semantic::WrappingRemainderI64)
                .unwrap();
            eq!(declaration.constraint => crate::register_model::X86_64_REMAINDER_I64);
            let alternative = &declaration.alternatives[0];
            assert_eq!(alternative.size, Size::ExactBytes(17));
            assert_eq!(alternative.encoded.external_operand_reads, [0, 1]);
            assert_eq!(alternative.encoded.external_operand_writes, [2, 3]);
            assert_eq!(
                alternative.encoded.implicit_unit_clobbers,
                x86_64_physical_register_model()
                    .view_named("condition_flags")
                    .unwrap()
                    .units
            );
            for corruption in 0..3 {
                let mut changed = catalog.clone();
                let alternative = &mut changed
                    .declaration_by_semantic_mut(Semantic::WrappingRemainderI64)
                    .unwrap()
                    .alternatives[0];
                match corruption {
                    0 => alternative.encoded.external_operand_writes.truncate(1),
                    1 => alternative.encoded.implicit_unit_clobbers.clear(),
                    _ => alternative.size = Size::ExactBytes(3),
                }
                assert!(
                    validate_x86_64_machine_effect_catalog(target, &constraints, changed).is_err()
                );
            }
        }
    }

    #[test]
    fn integer_normalization_catalog_binds_identity_size_and_effects() {
        for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
            let constraints = constraints();
            let catalog = x86_64_machine_effect_catalog(target, &constraints).unwrap();
            validate_x86_64_machine_effect_catalog(target, &constraints, catalog.clone()).unwrap();
            for semantic in [
                Semantic::ZeroExtendU16,
                Semantic::SignExtendI8,
                Semantic::SignExtendI16,
                Semantic::SignExtendI32,
            ] {
                let declaration = catalog.declaration_by_semantic(semantic).unwrap();
                assert_eq!(declaration.constraint, X86_64_COPY_I64);
                assert_eq!(declaration.alternatives.len(), 1);
                let alternative = &declaration.alternatives[0];
                assert_eq!(alternative.key.family, semantic.into());
                assert_eq!(
                    alternative.size,
                    Size::ExactBytes(if semantic == Semantic::SignExtendI32 {
                        3
                    } else {
                        4
                    })
                );
                eq!(alternative.encoded => MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]));
                for corruption in 0..3 {
                    let mut changed = catalog.clone();
                    let row = changed.declaration_by_semantic_mut(semantic).unwrap();
                    match corruption {
                        0 => row.alternatives[0].key.family = Semantic::CopyI64.into(),
                        1 => row.alternatives[0].size = Size::ExactBytes(1),
                        _ => row.alternatives[0].encoded.external_operand_reads.clear(),
                    }
                    assert!(
                        validate_x86_64_machine_effect_catalog(target, &constraints, changed)
                            .is_err()
                    );
                }
            }
        }
    }

    #[test]
    fn catalog_declares_alias_safe_subtraction_and_control_barriers() {
        for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
            let constraints = constraints();
            let catalog = x86_64_machine_effect_catalog(target, &constraints).unwrap();
            let subtract = catalog
                .declaration_by_semantic(Semantic::ExactSubtractI64)
                .unwrap();
            let add = catalog
                .declaration_by_semantic(Semantic::ExactAddI64)
                .unwrap();
            let less_than_branch = catalog
                .declaration_by_semantic(Semantic::ConditionalBranchU64LessThan)
                .unwrap();
            let signed_less_than_branch = catalog
                .declaration_by_semantic(Semantic::ConditionalBranchI64LessThan)
                .unwrap();
            assert_eq!(less_than_branch.constraint, X86_64_CONDITIONAL_BRANCH);
            assert_eq!(less_than_branch.alternatives.len(), 1);
            eq!(less_than_branch.alternatives[0].size => Size::ExactBytes(6));
            assert_eq!(
                less_than_branch.alternatives[0].encoded.control,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            );
            eq!(signed_less_than_branch.constraint => X86_64_CONDITIONAL_BRANCH);
            assert_eq!(signed_less_than_branch.alternatives.len(), 1);
            assert_eq!(
                signed_less_than_branch.alternatives[0].key.family,
                selected_instructions::MachineAlternativeFamily::ConditionalBranchI64LessThan
            );
            eq!(signed_less_than_branch.alternatives[0].size => Size::ExactBytes(6));
            eq!(add.alternatives[0].applicability => MachineAlternativeApplicability::Always);
            assert_eq!(subtract.constraint, X86_64_SUBTRACT_I64);
            let register_effects = constraints
                .catalog()
                .constraint(subtract.constraint)
                .unwrap();
            assert!(register_effects.implicit_uses.is_empty());
            assert!(register_effects.implicit_defs.is_empty());
            assert!(!register_effects.clobbers.is_empty());
            assert_eq!(subtract.alternatives.len(), 4);
            assert_eq!(
                subtract.alternatives[0].applicability,
                MachineAlternativeApplicability::ResultAliasesOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                }
            );
            assert_eq!(
                subtract.alternatives[1].applicability,
                MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result: 2,
                    aliased_operand: 0,
                    distinct_operand: 1,
                }
            );
            assert_eq!(
                subtract.alternatives[2].applicability,
                MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result: 2,
                    aliased_operand: 1,
                    distinct_operand: 0,
                }
            );
            assert_eq!(
                subtract.alternatives[3].applicability,
                MachineAlternativeApplicability::ResultDistinctFromOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                }
            );
            let expected_barrier = |semantic: Semantic| match semantic {
                Semantic::ConditionalBranchNonZero
                | Semantic::ConditionalBranchU64LessThan
                | Semantic::ConditionalBranchI64LessThan
                | Semantic::ReturnScalar
                | Semantic::ReturnAggregate
                | Semantic::Jump
                | Semantic::ReturnUnit
                | Semantic::Crash
                | Semantic::PrivilegedCrash => MachineBarrier::ControlFlow,
                Semantic::CallScalar
                | Semantic::InvokeNoNormal
                | Semantic::CallUnit
                | Semantic::CallAggregate
                | Semantic::NormalizedForeignCall => MachineBarrier::Call,
                Semantic::HostedReadByte
                | Semantic::SaveFloatingControl
                | Semantic::RestoreFloatingControl
                | Semantic::HostedWriteByteI32
                | Semantic::HostedExitProcessI32
                | Semantic::TrappingInteger(_)
                | Semantic::PortWrite
                | Semantic::SystemRegisterRead
                | Semantic::SystemRegisterWrite
                | Semantic::ControlRegisterRead
                | Semantic::ControlRegisterWrite
                | Semantic::GlobalDescriptorTableLoad
                | Semantic::GlobalDescriptorTableStore
                | Semantic::TaskRegisterActivate
                | Semantic::TaskRegisterRead
                | Semantic::CodeSegmentReload
                | Semantic::DataSegmentReload
                | Semantic::SegmentSelectorRead
                | Semantic::InstalledInterruptCpuCheck
                | Semantic::InstalledInterruptTaskCommit
                | Semantic::InstalledInterruptTablePublication
                | Semantic::PortRegisterRead
                | Semantic::PortRegisterWrite
                | Semantic::ModelSpecificRead
                | Semantic::ModelSpecificWrite
                | Semantic::ProcessorHalt
                | Semantic::InterruptControl
                | Semantic::MemoryFence
                | Semantic::AtomicFence
                | Semantic::AtomicAccess(_)
                | Semantic::AccessorScalarAccess(_) => MachineBarrier::ExternalEffect,
                Semantic::NearestIeeeFloatArithmetic(IeeeFloatArithmeticForm {
                    operation:
                        IeeeFloatArithmeticOperation::TruncateToInteger {
                            domain: IeeeFloatToIntegerDomain::Trapping,
                            ..
                        },
                    ..
                }) => MachineBarrier::ExternalEffect,
                Semantic::FloatResultPolicy(form) if form.can_trap() => {
                    MachineBarrier::ExternalEffect
                }
                _ => MachineBarrier::None,
            };
            for row in &catalog.declarations {
                assert_eq!(
                    row.barrier,
                    expected_barrier(row.semantic),
                    "{:?}",
                    row.semantic
                );
            }
            assert!(catalog.declarations.iter().any(|row| matches!(
                row.semantic,
                Semantic::NearestIeeeFloatArithmetic(IeeeFloatArithmeticForm {
                    operation: IeeeFloatArithmeticOperation::TruncateToInteger {
                        domain: IeeeFloatToIntegerDomain::Trapping,
                        ..
                    },
                    ..
                })
            )));
            assert!(catalog.declarations.iter().any(|row| matches!(
                row.semantic,
                Semantic::FloatResultPolicy(form) if form.can_trap()
            )));
            assert!(catalog.declarations.iter().any(|row| matches!(
                row.semantic,
                Semantic::FloatResultPolicy(form) if !form.can_trap()
            )));
            let scalar_call = catalog.declaration_by_semantic(Semantic::CallScalar);
            {
                let scalar_call = scalar_call.expect("supported target declares scalar call");
                assert_eq!(
                    scalar_call.call,
                    MachineCallEffect::InternalNormalReturnV1 {
                        pre_call_stack_alignment: 16,
                    }
                );
                eq!(scalar_call.alternatives[0].size => Size::ExactBytes(5));
                assert!(matches!(
                    scalar_call.alternatives[0].encoded.memory,
                    MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                        byte_count: 8,
                        ..
                    }
                ));
                assert!(matches!(
                    scalar_call.alternatives[0].encoded.stack,
                    MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                        return_address_byte_count: 8,
                        ..
                    }
                ));
            }
            let return_unit = catalog
                .declaration_by_semantic(Semantic::ReturnUnit)
                .unwrap();
            assert!(
                constraints
                    .catalog()
                    .constraint(return_unit.constraint)
                    .unwrap()
                    .operands
                    .is_empty()
            );
            assert!(validate_x86_64_machine_effect_catalog(target, &constraints, catalog).is_ok());
            let ordinary = x86_64_machine_effect_catalog(target, &constraints).unwrap();
            for semantic in [
                Semantic::Load64,
                Semantic::Store64,
                Semantic::FrameAddress,
                Semantic::CallUnit,
            ] {
                assert!(
                    ordinary
                        .declarations
                        .iter()
                        .any(|row| row.semantic == semantic)
                );
            }
            let load = ordinary.declaration_by_semantic(Semantic::Load64).unwrap();
            assert_eq!(load.memory, MachineMemoryEffect::ReadPointerV1);
            assert_eq!(
                load.alternatives[0].encoded.memory,
                MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: 8
                }
            );
            assert_eq!(
                load.alternatives[0].encoded.trap,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            );
        }
    }

    #[test]
    fn semantic_validator_rejects_structural_and_target_specific_corruption() {
        let target = NativeTarget::linux_x64();
        let constraints = constraints();
        let mut missing = x86_64_machine_effect_catalog(target, &constraints).unwrap();
        missing.declarations.pop();
        assert!(matches!(
            validate_x86_64_machine_effect_catalog(target, &constraints, missing),
            Err(X86_64MachineEffectCatalogValidationError::Structural(
                MachineEffectCatalogValidationError::DeclarationRosterMismatch
            ))
        ));

        let mut wrong_size = x86_64_machine_effect_catalog(target, &constraints).unwrap();
        wrong_size
            .declaration_by_semantic_mut(Semantic::ExactSubtractI64)
            .unwrap()
            .alternatives[0]
            .size = Size::ExactBytes(4);
        assert_eq!(
            validate_x86_64_machine_effect_catalog(target, &constraints, wrong_size),
            Err(X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch)
        );

        let mut bad_alias = x86_64_machine_effect_catalog(target, &constraints).unwrap();
        bad_alias
            .declaration_by_semantic_mut(Semantic::ExactSubtractI64)
            .unwrap()
            .alternatives[0]
            .applicability = MachineAlternativeApplicability::ResultAliasesOperand {
            result: 9,
            operand: 0,
        };
        assert!(matches!(
            validate_x86_64_machine_effect_catalog(target, &constraints, bad_alias),
            Err(X86_64MachineEffectCatalogValidationError::Structural(
                MachineEffectCatalogValidationError::InvalidAlternativeApplicability(
                    Semantic::ExactSubtractI64
                )
            ))
        ));

        let target = NativeTarget::windows_x64();
        let mut wrong_memory = x86_64_machine_effect_catalog(target, &constraints).unwrap();
        wrong_memory
            .declaration_by_semantic_mut(Semantic::Store64)
            .unwrap()
            .alternatives[0]
            .encoded
            .memory = MachineEncodedMemoryEffect::NoneV1;
        assert!(
            validate_x86_64_machine_effect_catalog(target, &constraints, wrong_memory).is_err()
        );
    }

    #[test]
    fn selected_keys_declare_the_supported_x86_64_pairs() {
        let constraints = constraints();
        let sysv_call = x86_64_system_v_register_call_keys()[0];
        let microsoft_call = crate::x86_64_microsoft_register_call_keys()[0];
        for (target, return_i64, return_unit, call) in [
            (
                NativeTarget::linux_x64(),
                X86_64_SYSTEM_V_RETURN,
                X86_64_SYSTEM_V_RETURN_UNIT,
                sysv_call,
            ),
            (
                NativeTarget::macos_x64(),
                X86_64_SYSTEM_V_RETURN,
                X86_64_SYSTEM_V_RETURN_UNIT,
                sysv_call,
            ),
            (
                NativeTarget::windows_x64(),
                X86_64_MICROSOFT_RETURN,
                X86_64_MICROSOFT_RETURN_UNIT,
                microsoft_call,
            ),
            (
                NativeTarget::uefi_x64(),
                X86_64_MICROSOFT_RETURN,
                X86_64_MICROSOFT_RETURN_UNIT,
                microsoft_call,
            ),
        ] {
            let catalog = x86_64_machine_effect_catalog(target, &constraints).unwrap();
            let keys = &catalog.selected_keys;
            assert_eq!(keys.return_i64, return_i64);
            assert_eq!(keys.return_unit, return_unit);
            assert!(keys.call_scalar.contains(&call));
            assert_eq!(keys.call_scalar[0], call);
            let hosted = target == NativeTarget::linux_x64() || target == NativeTarget::macos_x64();
            assert_eq!(
                keys.hosted_write_byte_i32.is_some(),
                hosted,
                "hosted write byte is declared only for the exact Linux and Intel macOS targets"
            );
            assert_eq!(keys.hosted_read_byte.is_some(), hosted);
            assert_eq!(keys.hosted_exit_process_i32.is_some(), hosted);
        }
    }

    #[test]
    fn selected_keys_fail_closed_on_undeclared_pairs() {
        let constraints = constraints();
        let aarch64_elf = NativeTarget {
            architecture: Architecture::Aarch64,
            object_format: ObjectFormat::Elf,
            pointer_size: 8,
            pointer_alignment: 8,
        };
        assert_eq!(
            x86_64_machine_effect_catalog(aarch64_elf, &constraints),
            Err(X86_64MachineEffectCatalogValidationError::TargetArchitectureMismatch)
        );
        assert_eq!(
            crate::machine_effects::x86_64_selected_abi(aarch64_elf),
            Err(X86_64MachineEffectCatalogValidationError::UnsupportedTargetAbi)
        );
    }

    #[test]
    fn hosted_rows_follow_the_declared_target_not_the_format_shape() {
        let constraints = constraints();
        let undeclared_elf = NativeTarget {
            pointer_size: 4,
            pointer_alignment: 4,
            ..NativeTarget::linux_x64()
        };
        let keys = x86_64_machine_effect_catalog(undeclared_elf, &constraints)
            .unwrap()
            .selected_keys;
        assert_eq!(keys.return_i64, X86_64_SYSTEM_V_RETURN);
        assert!(keys.hosted_write_byte_i32.is_none());
        assert!(keys.hosted_read_byte.is_none());
        assert!(keys.hosted_exit_process_i32.is_none());
    }
}
