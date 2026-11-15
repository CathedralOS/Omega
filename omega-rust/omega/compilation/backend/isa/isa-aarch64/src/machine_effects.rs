/*
We build and validate the AArch64 machine-effect catalog
(`aarch64_machine_effect_catalog` and `validate_aarch64_machine_effect_catalog`),
mapping every AArch64 register-constraint key and normalized foreign call to its
exact encoded register, flag, memory, stack, control-flow, and trap effects.
*/

use register_model::{PreparedRegisterConstraintCatalog, TargetRegisterEnvironmentConstraintKeys};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey, MachineBarrier,
    MachineCallEffect, MachineCleanupEffect, MachineEffectCatalog, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior, SelectedConstraintKeys,
    isa_machine_effect_catalog_plumbing,
};
use target::{Architecture, NativeTarget, ObjectFormat};

use crate::saturating_forms::SaturatingRealization;

mod scalar_call {
    use register_model::{PreparedRegisterConstraintCatalog, RegisterConstraintKey};
    use selected_instructions::{
        MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey, MachineBarrier,
        MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration,
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
        MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
        MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
    };

    pub(super) fn declaration(
        semantic: MachineSemanticKind,
        constraint: RegisterConstraintKey,
        constraints: &PreparedRegisterConstraintCatalog,
    ) -> MachineEffectDeclaration {
        let row = constraints
            .catalog()
            .constraint(constraint)
            .expect("canonical AArch64 catalog contains its scalar-call constraint");
        MachineEffectDeclaration::call(
            semantic,
            constraint,
            16,
            4,
            (semantic == MachineSemanticKind::NormalizedForeignCall).then_some(20),
            MachineEncodedEffects {
                external_operand_reads: row.operand_reads(),
                external_operand_writes: row.operand_writes(),
                implicit_unit_uses: row.implicit_uses.clone(),
                implicit_unit_defs: row.implicit_defs.clone(),
                implicit_unit_clobbers: row.clobbers.clone(),
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::DirectRelativeCallV1,
            },
        )
    }

    pub fn aarch64_normalized_foreign_call_declaration(
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
        let mut register_destinations = 0_u16;
        let mut field_destinations = 0_u16;
        let mut callback_byte_count = 0_u16;
        for callback in &record.call.callbacks {
            if Some(callback.terminal_operation) != record.invocation_site.operation() {
                return None;
            }
            match callback.destination.clone() {
                calling_conventions::NativePlace::Parameter(_) => {
                    if !matches!(
                        callback.application.placement.locations.as_slice(),
                        [calling_conventions::ValueLocation::Register { .. }]
                    ) {
                        return None;
                    }
                    register_destinations = register_destinations.checked_add(1)?;
                    callback_byte_count = callback_byte_count.checked_add(8)?;
                }
                calling_conventions::NativePlace::Field { .. } => {
                    field_destinations = field_destinations.checked_add(1)?;
                    let [calling_conventions::ValueLocation::Register { register, .. }] =
                        callback.application.placement.locations.as_slice()
                    else {
                        return None;
                    };
                    let destination =
                        crate::Aarch64CallbackMaterializationDestination::RecordField {
                            root_register: *register,
                            placements: callback.field_placements.clone()?,
                        };
                    let size =
                        crate::selected_form_encoding::callback_group_bytes(&destination)?.len();
                    callback_byte_count =
                        callback_byte_count.checked_add(u16::try_from(size).ok()?)?;
                }
            }
        }
        if supervisor && !record.call.callbacks.is_empty() {
            return None;
        }
        let variant =
            u32::from(register_destinations != 0) | (u32::from(field_destinations != 0) << 1);
        let byte_count = if supervisor || table.is_some() {
            20
        } else {
            4_u16.checked_add(callback_byte_count)?
        };
        Some(MachineEffectDeclaration {
            semantic: MachineSemanticKind::NormalizedForeignCall,
            constraint: row.key,
            memory: if field_destinations != 0 {
                MachineMemoryEffect::PrivateCallbackMaterializationV1
            } else if table.is_some() {
                MachineMemoryEffect::ReadPointerV1
            } else {
                MachineMemoryEffect::NoneV1
            },
            trap: if field_destinations != 0 || table.is_some() {
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
                encoded: MachineEncodedEffects {
                    external_operand_reads: row.operand_reads(),
                    external_operand_writes: row.operand_writes(),
                    implicit_unit_uses: row.implicit_uses.clone(),
                    implicit_unit_defs: row.implicit_defs.clone(),
                    implicit_unit_clobbers: row.clobbers.clone(),
                    memory: match table {
                        Some((_, byte_offset)) => {
                            let position =
                            selected_instructions::calls::constraints::foreign_argument_registers(
                                &record.call,
                            )
                            .iter()
                            .position(|(parameter, _)| *parameter == 0)?;
                            MachineEncodedMemoryEffect::ReadTableFieldV1 {
                                pointer_operand: u16::try_from(position).ok()?,
                                byte_offset,
                            }
                        }
                        None if field_destinations != 0 => {
                            MachineEncodedMemoryEffect::PrivateCallbackMaterializationV1
                        }
                        None => MachineEncodedMemoryEffect::NoneV1,
                    },
                    stack: MachineEncodedStackEffect::UnchangedV1,
                    trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    control: if supervisor {
                        MachineEncodedControlEffect::SupervisorCallV1
                    } else if table.is_some() {
                        MachineEncodedControlEffect::IndirectExternalCallV1
                    } else {
                        MachineEncodedControlEffect::DirectRelativeCallV1
                    },
                },
            }],
        })
    }

    pub(crate) fn normalized_foreign_constraint(
        record: &selected_instructions::SelectedNormalizedForeignCall,
    ) -> Option<register_model::RegisterInstructionConstraint> {
        let target = record.call.binding.native_target()?;
        let abi = super::aarch64_selected_abi(target).ok()?;
        let key = if record.call.binding.system_call().is_some() {
            *crate::aarch64_linux_system_call_keys().first()?
        } else {
            match abi {
                super::Aarch64SelectedAbi::Aapcs64 => {
                    *crate::aarch64_aapcs64_normalized_foreign_call_keys().first()?
                }
                super::Aarch64SelectedAbi::Darwin => {
                    *crate::aarch64_darwin_normalized_foreign_call_keys().first()?
                }
            }
        };
        let physical = crate::validated_aarch64_physical_register_model();
        let catalog = crate::aarch64_register_constraint_catalog_for(physical);
        let base = catalog.constraint(key)?;
        selected_instructions::calls::constraints::instantiate_foreign_call_constraint(
            &record.call,
            base,
            physical,
            |register| crate::aarch64_fixed_register_view(physical, register),
        )
    }

    #[cfg(test)]
    mod tests {
        use target::NativeTarget;

        #[test]
        fn system_call_effects_reject_user_space_call_state_and_missing_clobbers() {
            let physical = crate::validated_aarch64_physical_register_model();
            let constraints = crate::validate_aarch64_register_constraint_catalog(
                crate::aarch64_register_constraint_catalog(physical),
                physical,
            )
            .unwrap();
            let target = NativeTarget::linux_arm64();
            let catalog = crate::aarch64_machine_effect_catalog(target, &constraints).unwrap();
            crate::validate_aarch64_machine_effect_catalog(target, &constraints, catalog.clone())
                .expect("kernel entry effects");
            let other =
                crate::aarch64_machine_effect_catalog(NativeTarget::macos_arm64(), &constraints)
                    .unwrap();
            isa_conformance::system_call::effects_conformance(
                &constraints,
                &catalog,
                &crate::aarch64_linux_system_call_keys(),
                physical.model().view_named("sp").unwrap().id,
                &other,
            );
        }
    }
}
pub use scalar_call::aarch64_normalized_foreign_call_declaration;
pub(crate) use scalar_call::normalized_foreign_constraint;

use scalar_call::declaration as scalar_call_declaration;

use crate::{
    AARCH64_AAPCS64_RETURN, AARCH64_AAPCS64_RETURN_UNIT, AARCH64_ADD_I64,
    AARCH64_ADD_I64_IMMEDIATE, AARCH64_COMPARE_I64, AARCH64_COMPARE_I64_IMMEDIATE,
    AARCH64_COMPARE_I64_ZERO, AARCH64_CONDITIONAL_BRANCH, AARCH64_COPY_I64, AARCH64_DARWIN_RETURN,
    AARCH64_DARWIN_RETURN_UNIT, AARCH64_MATERIALIZE_I64, AARCH64_SUBTRACT_I64,
    AARCH64_SUBTRACT_I64_IMMEDIATE, aarch64_aapcs64_register_call_keys,
};

isa_machine_effect_catalog_plumbing!(
    Aarch64MachineEffectCatalogValidationError,
    validate_aarch64_machine_effect_catalog,
    Architecture::Aarch64,
    "AArch64",
    aarch64_machine_effect_catalog
);

pub fn aarch64_machine_effect_catalog(
    target: NativeTarget,
    constraints: &PreparedRegisterConstraintCatalog,
) -> Result<MachineEffectCatalog, Aarch64MachineEffectCatalogValidationError> {
    if target.architecture != Architecture::Aarch64
        || constraints.architecture() != Architecture::Aarch64
    {
        return Err(Aarch64MachineEffectCatalogValidationError::TargetArchitectureMismatch);
    }
    let selected_keys = aarch64_selected_constraint_keys(target)
        .ok_or(Aarch64MachineEffectCatalogValidationError::UnsupportedTargetAbi)?;
    Ok(MachineEffectCatalog {
        target,
        register_constraints: constraints.identity(),
        selected_keys: selected_keys.clone(),
        declarations: selected_keys
            .declaration_keys()
            .into_iter()
            .map(|(semantic, constraint)| {
                if semantic == MachineSemanticKind::MemoryFence {
                    return Ok(crate::selected_form_encoding::memory_fence::declaration(
                        constraint,
                    ));
                }
                if matches!(
                    semantic,
                    MachineSemanticKind::SystemRegisterRead
                        | MachineSemanticKind::SystemRegisterWrite
                ) {
                    return Ok(crate::selected_form_encoding::register_access::declaration(
                        semantic == MachineSemanticKind::SystemRegisterRead,
                        constraint,
                    ));
                }
                if semantic == MachineSemanticKind::AtomicFence {
                    return Ok(crate::selected_form_encoding::atomic_fence::declaration(
                        constraint,
                    ));
                }
                if let MachineSemanticKind::AtomicAccess(form) = semantic {
                    return Ok(crate::selected_form_encoding::atomic::declaration(
                        form, constraint,
                    ));
                }
                if let MachineSemanticKind::NearestIeeeFloatArithmetic(form) = semantic {
                    return Ok(
                        crate::selected_form_encoding::ieee_float_arithmetic::declaration(
                            form, constraint,
                        ),
                    );
                }
                if let MachineSemanticKind::FloatResultPolicy(form) = semantic {
                    return Ok(
                        crate::selected_form_encoding::float_result_policy::declaration(
                            form, constraint,
                        ),
                    );
                }
                if semantic == MachineSemanticKind::Crash {
                    return Ok(crate::selected_form_encoding::crash::declaration(
                        constraints.catalog().constraint(constraint).ok_or(
                            Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch,
                        )?,
                    ));
                }
                if let MachineSemanticKind::TrappingInteger(form) = semantic {
                    return Ok(crate::selected_form_encoding::trapping_forms::declaration(
                        form, constraint,
                    ));
                }
                if semantic == MachineSemanticKind::HostedExitProcessI32 {
                    return crate::selected_form_encoding::hosted_exit_process::declaration(
                        target, constraint,
                    )
                    .map_err(|_| {
                        Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch
                    });
                }
                if semantic == MachineSemanticKind::HostedReadByte {
                    return Ok(
                        crate::selected_form_encoding::hosted_read_byte::declaration(
                            target, constraint,
                        ),
                    );
                }
                if matches!(
                    semantic,
                    MachineSemanticKind::SaveFloatingControl
                        | MachineSemanticKind::RestoreFloatingControl
                ) {
                    return Ok(
                        crate::selected_form_encoding::floating_control::declaration(
                            semantic, constraint,
                        ),
                    );
                }
                if semantic == MachineSemanticKind::HostedWriteByteI32 {
                    return Ok(
                        crate::selected_form_encoding::hosted_write_byte::declaration(
                            target, constraint,
                        ),
                    );
                }
                Ok(
                    if matches!(
                        semantic,
                        MachineSemanticKind::ReturnAggregate | MachineSemanticKind::ReturnScalar
                    ) {
                        let mut returned =
                            declaration(MachineSemanticKind::ReturnScalar, &selected_keys)?;
                        returned.semantic = semantic;
                        returned.constraint = constraint;
                        returned.alternatives[0].key.family = semantic.into();
                        returned
                    } else if matches!(
                        semantic,
                        MachineSemanticKind::CallScalar
                            | MachineSemanticKind::InvokeNoNormal
                            | MachineSemanticKind::CallUnit
                            | MachineSemanticKind::CallAggregate
                            | MachineSemanticKind::NormalizedForeignCall
                    ) {
                        scalar_call_declaration(semantic, constraint, constraints)
                    } else {
                        declaration(semantic, &selected_keys)?
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Aarch64SelectedAbi {
    Aapcs64,
    Darwin,
}

pub(crate) fn aarch64_selected_abi(
    target: NativeTarget,
) -> Result<Aarch64SelectedAbi, Aarch64MachineEffectCatalogValidationError> {
    match (target.architecture, target.object_format) {
        (Architecture::Aarch64, ObjectFormat::Elf) => Ok(Aarch64SelectedAbi::Aapcs64),
        (Architecture::Aarch64, ObjectFormat::MachO) => Ok(Aarch64SelectedAbi::Darwin),
        (Architecture::Aarch64, ObjectFormat::Coff)
        | (Architecture::X86_64, ObjectFormat::Elf)
        | (Architecture::X86_64, ObjectFormat::MachO)
        | (Architecture::X86_64, ObjectFormat::Coff) => {
            Err(Aarch64MachineEffectCatalogValidationError::UnsupportedTargetAbi)
        }
    }
}

pub fn aarch64_selected_constraint_keys(target: NativeTarget) -> Option<SelectedConstraintKeys> {
    let Ok(abi) = aarch64_selected_abi(target) else {
        return None;
    };
    let darwin = abi == Aarch64SelectedAbi::Darwin;
    let return_i64 = match abi {
        Aarch64SelectedAbi::Aapcs64 => AARCH64_AAPCS64_RETURN,
        Aarch64SelectedAbi::Darwin => AARCH64_DARWIN_RETURN,
    };
    let return_unit = match abi {
        Aarch64SelectedAbi::Aapcs64 => AARCH64_AAPCS64_RETURN_UNIT,
        Aarch64SelectedAbi::Darwin => AARCH64_DARWIN_RETURN_UNIT,
    };
    Some({
        let rows = TargetRegisterEnvironmentConstraintKeys {
            port_write: None,
            system_register_read: Some(crate::AARCH64_SYSTEM_REGISTER_READ),
            system_register_write: Some(crate::AARCH64_SYSTEM_REGISTER_WRITE),
            global_descriptor_table: None,
            task_register_activate: None,
            task_register_read: None,
            installed_interrupt_cpu_check: None,
            installed_interrupt_task_commit: None,
            installed_interrupt_table_publication: None,
            code_segment_reload: None,
            data_segment_reload: None,
            segment_selector_read: None,
            control_register_read: None,
            control_register_write: None,
            port_register_read: None,
            port_register_write: None,
            model_specific_read: None,
            model_specific_write: None,
            processor_halt: None,
            interrupt_control: None,
            memory_fence: Some(crate::AARCH64_MEMORY_FENCE),
            atomic_fence: Some(crate::AARCH64_ATOMIC_FENCE),
            access_read: Some(crate::AARCH64_ACCESSOR_READ),
            access_write: Some(crate::AARCH64_ACCESSOR_WRITE),
            atomic_load: Some(crate::AARCH64_ATOMIC_LOAD),
            atomic_store: Some(crate::AARCH64_ATOMIC_STORE),
            atomic_update: Some(crate::AARCH64_ATOMIC_UPDATE),
            atomic_bitwise_update: Some(crate::AARCH64_ATOMIC_UPDATE),
            atomic_compare_exchange: Some(crate::AARCH64_ATOMIC_COMPARE_EXCHANGE),
            atomic_compare_exchange_once: Some(crate::AARCH64_ATOMIC_COMPARE_EXCHANGE_ONCE),
            crash: crate::AARCH64_CRASH,
            privileged_crash: None,
            copy_bytes: Some(crate::AARCH64_COPY_BYTES),
            zero_bytes: Some(crate::AARCH64_ZERO_BYTES),
            scan_byte_sequence: Some(crate::AARCH64_SCAN_BYTE_SEQUENCE),
            load_encoded_scalar: Some(crate::AARCH64_LOAD_ENCODED_SCALAR),
            store_encoded_scalar: Some(crate::AARCH64_STORE_ENCODED_SCALAR),
            compare_byte_sequences: Some(crate::AARCH64_COMPARE_BYTE_SEQUENCES),
            save_floating_control: Some(crate::AARCH64_SAVE_FLOATING_CONTROL),
            restore_floating_control: Some(crate::AARCH64_RESTORE_FLOATING_CONTROL),
            load_packed: Some(crate::AARCH64_LOAD_PACKED),
            store_packed: Some(crate::AARCH64_STORE_PACKED),
            hosted_read_byte: if target == NativeTarget::linux_arm64() {
                Some(crate::AARCH64_HOSTED_READ_BYTE)
            } else if target == NativeTarget::macos_arm64() {
                Some(crate::AARCH64_DARWIN_HOSTED_READ_BYTE)
            } else {
                None
            },
            hosted_exit_process_i32: if target == NativeTarget::linux_arm64() {
                Some(crate::AARCH64_HOSTED_EXIT_PROCESS_I32)
            } else if target == NativeTarget::macos_arm64() {
                Some(crate::AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32)
            } else {
                None
            },
            hosted_write_byte_i32: if target == NativeTarget::linux_arm64() {
                Some(crate::AARCH64_HOSTED_WRITE_BYTE_I32)
            } else if target == NativeTarget::macos_arm64() {
                Some(crate::AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32)
            } else {
                None
            },
            load64: Some(crate::AARCH64_LOAD64),
            load8: Some(crate::AARCH64_LOAD8),
            load16: Some(crate::AARCH64_LOAD16),
            load32: Some(crate::AARCH64_LOAD32),
            load8_indexed: Some(crate::AARCH64_LOAD8_INDEXED),
            store: Some(crate::AARCH64_STORE),
            address_offset: Some(crate::AARCH64_ADDRESS_OFFSET),
            store64: Some(crate::AARCH64_STORE64),
            frame_address: Some(crate::AARCH64_FRAME_ADDRESS),
            byte_literal_image_address: Some(crate::AARCH64_BYTE_LITERAL_IMAGE_ADDRESS),
            call_unit_mixed: match abi {
                Aarch64SelectedAbi::Aapcs64 => crate::aarch64_aapcs64_mixed_unit_call_keys(),
                Aarch64SelectedAbi::Darwin => crate::aarch64_darwin_mixed_unit_call_keys(),
            },
            call_unit: match abi {
                Aarch64SelectedAbi::Aapcs64 => crate::aarch64_aapcs64_register_unit_call_keys(),
                Aarch64SelectedAbi::Darwin => crate::aarch64_darwin_register_unit_call_keys(),
            },
            call_scalar: (match abi {
                Aarch64SelectedAbi::Aapcs64 => aarch64_aapcs64_register_call_keys(),
                Aarch64SelectedAbi::Darwin => crate::aarch64_darwin_register_call_keys(),
            })
            .into_iter()
            .chain(crate::aarch64_float_scalar_call_keys(darwin))
            .collect(),
            call_normalized_foreign: (match abi {
                Aarch64SelectedAbi::Aapcs64 => {
                    crate::aarch64_aapcs64_normalized_foreign_call_keys()
                }
                Aarch64SelectedAbi::Darwin => crate::aarch64_darwin_normalized_foreign_call_keys(),
            })
            .into_iter()
            .chain(
                (target.object_format == ObjectFormat::Elf)
                    .then(crate::aarch64_linux_system_call_keys)
                    .into_iter()
                    .flatten(),
            )
            .collect(),
            materialize_i64: AARCH64_MATERIALIZE_I64,
            materialize_boolean: crate::AARCH64_MATERIALIZE_BOOLEAN,
            call_aggregate: crate::aarch64_register_aggregate_call_keys(darwin)
                .into_iter()
                .chain(crate::aarch64_mixed_aggregate_call_keys(darwin))
                .chain(crate::aarch64_indirect_aggregate_call_keys(darwin))
                .collect(),
            return_aggregate: crate::aarch64_register_aggregate_return_keys(darwin),
            copy_i64: AARCH64_COPY_I64,
            float32_to_bits: Some(crate::AARCH64_FLOAT32_TO_BITS),
            float64_to_bits: Some(crate::AARCH64_FLOAT64_TO_BITS),
            ieee_float_arithmetic: Some(crate::AARCH64_IEEE_FLOAT_ARITHMETIC),
            ieee_float_directed_binary: Some(crate::AARCH64_IEEE_FLOAT_ARITHMETIC),
            ieee_float_directed_unary: Some(crate::AARCH64_IEEE_FLOAT_UNARY),
            ieee_float_unary: Some(crate::AARCH64_IEEE_FLOAT_UNARY),
            ieee_float_conversion_control: Some(crate::AARCH64_IEEE_FLOAT_UNARY),
            ieee_float_ternary: Some(crate::AARCH64_IEEE_FLOAT_TERNARY),
            float_result_trapping: Some(crate::AARCH64_FLOAT_RESULT_TRAPPING),
            float_result_saturating: Some(crate::AARCH64_FLOAT_RESULT_SATURATING),
            bits_to_float32: Some(crate::AARCH64_BITS_TO_FLOAT32),
            bits_to_float64: Some(crate::AARCH64_BITS_TO_FLOAT64),
            add_i64: AARCH64_ADD_I64,
            subtract_i64: AARCH64_SUBTRACT_I64,
            multiply_i64: crate::AARCH64_MULTIPLY_I64,
            saturating_subtract_unsigned:
                crate::register_model::AARCH64_SATURATING_SUBTRACT_UNSIGNED,
            saturating_add_u64: crate::register_model::AARCH64_SATURATING_ADD_U64,
            divide_u64: crate::register_model::AARCH64_DIVIDE_U64,
            remainder_u64: crate::register_model::AARCH64_REMAINDER_U64,
            remainder_i64: crate::register_model::AARCH64_REMAINDER_I64,
            divide_i64: crate::register_model::AARCH64_DIVIDE_I64,
            shift_i64: crate::register_model::AARCH64_SHIFT_I64,
            saturating_add_clamped: crate::register_model::AARCH64_SATURATING_ADD_CLAMPED,
            saturating_subtract_clamped: crate::register_model::AARCH64_SATURATING_SUBTRACT_CLAMPED,
            saturating_divide_signed: crate::register_model::AARCH64_SATURATING_DIVIDE_SIGNED,
            saturating_multiply_clamped: crate::register_model::AARCH64_SATURATING_MULTIPLY_CLAMPED,
            saturating_multiply_u64: crate::register_model::AARCH64_SATURATING_MULTIPLY_U64,
            trapping_binary: crate::register_model::AARCH64_TRAPPING_BINARY,
            trapping_fixed_pair: crate::register_model::AARCH64_TRAPPING_FIXED_PAIR,
            trapping_shift: crate::register_model::AARCH64_TRAPPING_SHIFT,
            trapping_convert: crate::register_model::AARCH64_TRAPPING_CONVERT,
            add_i64_immediate: AARCH64_ADD_I64_IMMEDIATE,
            subtract_i64_immediate: AARCH64_SUBTRACT_I64_IMMEDIATE,
            compare_i64_zero: AARCH64_COMPARE_I64_ZERO,
            compare_i64: AARCH64_COMPARE_I64,
            compare_i64_immediate: AARCH64_COMPARE_I64_IMMEDIATE,
            conditional_branch: AARCH64_CONDITIONAL_BRANCH,
            jump: crate::AARCH64_JUMP,
            return_float: crate::aarch64_float_scalar_return_keys(darwin),
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

macro_rules! form_alternative {
    ($family:expr, $size:expr, $encoded:expr) => {
        vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: $family,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: $size,
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: $encoded,
        }]
    };
}

fn declaration(
    semantic: MachineSemanticKind,
    keys: &SelectedConstraintKeys,
) -> Result<MachineEffectDeclaration, Aarch64MachineEffectCatalogValidationError> {
    Ok(MachineEffectDeclaration {
        semantic,
        constraint: keys
            .for_semantic(semantic)
            .ok_or(Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch)?,
        memory: if let MachineSemanticKind::AccessorScalarAccess(form) = semantic {
            if form.write {
                MachineMemoryEffect::WritePointerV1
            } else {
                MachineMemoryEffect::ReadPointerV1
            }
        } else if semantic == MachineSemanticKind::CopyBytes {
            MachineMemoryEffect::CopyBytesV1
        } else if semantic == MachineSemanticKind::ZeroBytes {
            MachineMemoryEffect::ZeroBytesV1
        } else if semantic == MachineSemanticKind::LoadEncodedScalar {
            MachineMemoryEffect::ReadEncodedScalarV1
        } else if semantic == MachineSemanticKind::StoreEncodedScalar {
            MachineMemoryEffect::WriteEncodedScalarV1
        } else if semantic == MachineSemanticKind::ScanByteSequence {
            MachineMemoryEffect::ScanByteSequenceV1
        } else if semantic == MachineSemanticKind::CompareByteSequences {
            MachineMemoryEffect::CompareByteSequencesV1
        } else if matches!(
            semantic,
            MachineSemanticKind::Load8
                | MachineSemanticKind::Load16
                | MachineSemanticKind::Load32
                | MachineSemanticKind::Load64
                | MachineSemanticKind::Load8Indexed
        ) || packed_load_width(semantic).is_some()
        {
            MachineMemoryEffect::ReadPointerV1
        } else if semantic == MachineSemanticKind::Store64 {
            MachineMemoryEffect::WriteFrameStorageV1
        } else if matches!(
            semantic,
            MachineSemanticKind::Store | MachineSemanticKind::StorePacked
        ) {
            MachineMemoryEffect::WritePointerV1
        } else {
            MachineMemoryEffect::NoneV1
        },
        trap: if matches!(
            semantic,
            MachineSemanticKind::AccessorScalarAccess(_)
                | MachineSemanticKind::CopyBytes
                | MachineSemanticKind::ZeroBytes
                | MachineSemanticKind::Load8
                | MachineSemanticKind::Load16
                | MachineSemanticKind::Load32
                | MachineSemanticKind::Load64
                | MachineSemanticKind::Load8Indexed
                | MachineSemanticKind::Store64
                | MachineSemanticKind::Store
                | MachineSemanticKind::LoadPacked3
                | MachineSemanticKind::LoadPacked5
                | MachineSemanticKind::LoadPacked6
                | MachineSemanticKind::LoadPacked7
                | MachineSemanticKind::StorePacked
                | MachineSemanticKind::ScanByteSequence
                | MachineSemanticKind::LoadEncodedScalar
                | MachineSemanticKind::StoreEncodedScalar
                | MachineSemanticKind::CompareByteSequences
        ) {
            MachineTrapBehavior::MayArchitecturalFaultV1
        } else {
            MachineTrapBehavior::NeverV1
        },
        barrier: if matches!(semantic, MachineSemanticKind::AccessorScalarAccess(_)) {
            MachineBarrier::ExternalEffect
        } else if matches!(
            semantic,
            MachineSemanticKind::ConditionalBranchNonZero
                | MachineSemanticKind::ConditionalBranchU64LessThan
                | MachineSemanticKind::ConditionalBranchI64LessThan
                | MachineSemanticKind::ReturnScalar
                | MachineSemanticKind::Jump
                | MachineSemanticKind::ReturnUnit
        ) {
            MachineBarrier::ControlFlow
        } else {
            MachineBarrier::None
        },
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: form_alternative!(
            semantic.into(),
            size(semantic),
            encoded_effects(semantic)?
        ),
    })
}

/// Classifies operands and implicit effects together, including explicit flag-transparent forms.
fn encoded_effects(
    semantic: MachineSemanticKind,
) -> Result<MachineEncodedEffects, Aarch64MachineEffectCatalogValidationError> {
    use MachineSemanticKind as S;
    let physical = crate::aarch64_physical_register_model();
    let units = |name: &str| {
        physical
            .view_named(name)
            .map(|view| view.units.clone())
            .ok_or(Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch)
    };
    let view = |name: &str| {
        physical
            .view_named(name)
            .map(|view| view.id)
            .ok_or(Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch)
    };
    let plain = MachineEncodedEffects::fallthrough_v1;
    let memory = |reads, writes, memory| {
        let mut effects = plain(reads, writes);
        effects.memory = memory;
        effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        effects
    };
    let bulk = |reads, writes, footprint| {
        let mut effects = memory(reads, writes, footprint);
        effects.implicit_unit_clobbers = units("nzcv")?;
        Ok::<_, Aarch64MachineEffectCatalogValidationError>(effects)
    };
    let load = |byte_count, writes| {
        memory(
            vec![0],
            writes,
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count,
            },
        )
    };
    Ok(match semantic {
        S::Crash => MachineEncodedEffects::crash_v1(&units("pc")?),
        S::SystemRegisterRead => crate::selected_form_encoding::register_access::effects(true),
        S::SystemRegisterWrite => crate::selected_form_encoding::register_access::effects(false),
        S::MemoryFence => MachineEncodedEffects::catalog_fence_v1(),
        S::AtomicFence => MachineEncodedEffects::atomic_fence_v1(),
        S::AtomicAccess(form) => crate::selected_form_encoding::atomic::effects(form),
        S::NearestIeeeFloatArithmetic(form) => {
            crate::selected_form_encoding::ieee_float_arithmetic::effects(form)
        }
        S::FloatResultPolicy(form) => {
            crate::selected_form_encoding::float_result_policy::effects(form)
        }
        S::AccessorScalarAccess(form) => {
            if form.write {
                memory(
                    vec![0, 1],
                    vec![],
                    MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
                )
            } else {
                load(u16::from(form.bytes), vec![1])
            }
        }
        S::CopyBytes => bulk(
            vec![0, 1, 2],
            vec![3, 4],
            MachineEncodedMemoryEffect::CopyBytesV1 {
                source_pointer_operand: 0,
                destination_pointer_operand: 1,
                count_operand: 2,
            },
        )?,
        S::ZeroBytes => bulk(
            vec![0, 1],
            vec![2],
            MachineEncodedMemoryEffect::ZeroBytesV1 {
                destination_pointer_operand: 0,
                count_operand: 1,
            },
        )?,
        S::ScanByteSequence => bulk(
            vec![0, 1],
            vec![2, 3, 4],
            MachineEncodedMemoryEffect::ScanByteSequenceV1 {
                base_pointer_operand: 0,
                count_operand: 1,
            },
        )?,
        S::CompareByteSequences => bulk(
            vec![0, 1, 2, 3],
            vec![4, 5, 6, 7],
            MachineEncodedMemoryEffect::CompareByteSequencesV1 {
                left_base_operand: 0,
                left_count_operand: 1,
                right_base_operand: 2,
                right_count_operand: 3,
            },
        )?,
        S::LoadEncodedScalar => bulk(
            vec![0, 1],
            vec![2, 3, 4, 5],
            MachineEncodedMemoryEffect::ReadEncodedScalarV1 {
                pointer_operand: 0,
                codec_operand: 1,
            },
        )?,
        S::StoreEncodedScalar => bulk(
            vec![0, 1, 2],
            vec![3, 4, 5],
            MachineEncodedMemoryEffect::WriteEncodedScalarV1 {
                pointer_operand: 0,
                codec_operand: 1,
            },
        )?,
        S::Load8 => load(1, vec![1]),
        S::Load16 => load(2, vec![1]),
        S::Load32 => load(4, vec![1]),
        S::Load64 => load(8, vec![1]),
        S::LoadPacked3 => load(3, vec![1, 2]),
        S::LoadPacked5 => load(5, vec![1, 2]),
        S::LoadPacked6 => load(6, vec![1, 2]),
        S::LoadPacked7 => load(7, vec![1, 2]),
        S::Load8Indexed => memory(
            vec![0, 1],
            vec![2],
            MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                pointer_operand: 0,
                index_operand: 1,
                byte_count: 1,
            },
        ),
        S::Store => memory(
            vec![0, 1],
            vec![],
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
        ),
        S::StorePacked => memory(
            vec![0, 1],
            vec![2],
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
        ),
        S::Store64 => {
            let mut effects = memory(
                vec![0],
                vec![1],
                MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                    stack_pointer: view("sp")?,
                    byte_count: 8,
                },
            );
            effects.implicit_unit_uses = units("sp")?;
            effects
        }
        S::FrameAddress => {
            let mut effects = plain(vec![], vec![0]);
            effects.implicit_unit_uses = units("sp")?;
            effects
        }
        S::CompareI64Zero | S::CompareI64Immediate | S::CompareI64 => {
            let reads = if semantic == S::CompareI64 {
                vec![0, 1]
            } else {
                vec![0]
            };
            let mut effects = plain(reads, vec![]);
            effects.implicit_unit_defs = units("nzcv")?;
            effects
        }
        S::MaterializeBooleanEqual
        | S::MaterializeBooleanU64LessThan
        | S::MaterializeBooleanI64LessThan
        | S::MaterializeBooleanU64LessOrEqual
        | S::MaterializeBooleanI64LessOrEqual => {
            let mut effects = plain(vec![], vec![0]);
            effects.implicit_unit_uses = units("nzcv")?;
            effects
        }
        S::MaterializeI64 | S::ImageAddress => plain(vec![], vec![0]),
        S::Float32ToBits
        | S::Float64ToBits
        | S::BitsToFloat32
        | S::BitsToFloat64
        | S::CopyI64
        | S::AddressOffset
        | S::ZeroExtendU8
        | S::ZeroExtendU16
        | S::SignExtendI8
        | S::SignExtendI16
        | S::SignExtendI32
        | S::ZeroExtendU32
        | S::BitwiseNotI64
        | S::ExactAddI64Immediate
        | S::ExactSubtractI64Immediate => plain(vec![0], vec![1]),
        S::SaturatingAdd(_)
        | S::SaturatingSubtract(_)
        | S::SaturatingDivide(_)
        | S::SaturatingRemainder(_)
        | S::SaturatingMultiply(_) => {
            let realization = SaturatingRealization::of_semantic(semantic)
                .ok_or(Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch)?;
            let mut effects = plain(
                vec![0, 1],
                (2..realization.operand_count() as u16).collect(),
            );
            if realization.defines_nzcv() {
                effects.implicit_unit_defs = units("nzcv")?;
            }
            effects
        }
        S::ExactDivideU64
        | S::ExactRemainderU64
        | S::WrappingRemainderI64
        | S::WrappingDivideI64
        | S::ExactDivideI64
        | S::ExactRemainderI64
        | S::ByteViewAddress
        | S::BitwiseAndI64
        | S::BitwiseOrI64
        | S::BitwiseXorI64
        | S::WrappingAddI64
        | S::WrappingSubtractI64
        | S::WrappingMultiplyI64
        | S::WrappingShiftLeftI64
        | S::WrappingShiftRightI64
        | S::WrappingShiftRightU64
        | S::ExactShiftLeftI64
        | S::ExactShiftRightI64
        | S::ExactShiftRightU64
        | S::ExactAddI64
        | S::ExactSubtractI64
        | S::ExactMultiplyI64 => plain(vec![0, 1], vec![2]),
        S::Jump
        | S::ConditionalBranchNonZero
        | S::ConditionalBranchU64LessThan
        | S::ConditionalBranchI64LessThan => {
            let mut effects = plain(vec![], vec![]);
            effects.implicit_unit_uses = units("pc")?;
            if semantic != S::Jump {
                effects.implicit_unit_uses.extend(units("nzcv")?);
                effects.implicit_unit_uses.sort_unstable();
                effects.implicit_unit_uses.dedup();
            }
            effects.implicit_unit_defs = units("pc")?;
            effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
            effects.control = if semantic == S::Jump {
                MachineEncodedControlEffect::UnconditionalRelativeBranchV1
            } else {
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            };
            effects
        }
        S::ReturnScalar | S::ReturnAggregate | S::ReturnUnit => {
            let mut effects = plain(vec![], vec![]);
            effects.implicit_unit_uses = units("x30")?;
            effects.implicit_unit_defs = units("pc")?;
            effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
            effects.control = MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                target: view("x30")?,
            };
            effects
        }
        // Foreign/call/trapping forms need context owned by their dedicated declaration.
        // x86-only semantics have no AArch64 realization.
        S::HostedExitProcessI32
        | S::HostedReadByte
        | S::SaveFloatingControl
        | S::RestoreFloatingControl
        | S::HostedWriteByteI32
        | S::CallUnit
        | S::CallScalar
        | S::InvokeNoNormal
        | S::CallAggregate
        | S::NormalizedForeignCall
        | S::TrappingInteger(_)
        | S::PrivilegedCrash
        | S::PortRegisterRead
        | S::PortRegisterWrite
        | S::ModelSpecificRead
        | S::ModelSpecificWrite
        | S::ProcessorHalt
        | S::InterruptControl
        | S::ControlRegisterRead
        | S::ControlRegisterWrite
        | S::GlobalDescriptorTableLoad
        | S::GlobalDescriptorTableStore
        | S::TaskRegisterActivate
        | S::TaskRegisterRead
        | S::InstalledInterruptCpuCheck
        | S::InstalledInterruptTaskCommit
        | S::InstalledInterruptTablePublication
        | S::CodeSegmentReload
        | S::DataSegmentReload
        | S::SegmentSelectorRead
        | S::PortWrite => {
            return Err(Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch);
        }
    })
}

fn size(semantic: MachineSemanticKind) -> MachineSizeKnowledge {
    if let Some(width) = packed_load_width(semantic) {
        return MachineSizeKnowledge::ExactBytes((2 * width - 1) * 4);
    }
    if let Some(realization) = SaturatingRealization::of_semantic(semantic) {
        return MachineSizeKnowledge::ExactBytes(realization.byte_size());
    }
    match semantic {
        MachineSemanticKind::PrivilegedCrash => {
            unreachable!("AArch64 has no privileged termination key")
        }
        MachineSemanticKind::Crash => MachineSizeKnowledge::ExactBytes(4),
        MachineSemanticKind::Store64 => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 8,
            maximum_bytes: Some(
                crate::immediate_offset::MAX_STORE64_BYTES
                    + crate::immediate_offset::MAX_ADDRESS_BYTES
                    + 8,
            ),
        },
        MachineSemanticKind::CopyBytes => MachineSizeKnowledge::ExactBytes(28),
        MachineSemanticKind::ZeroBytes => MachineSizeKnowledge::ExactBytes(24),
        MachineSemanticKind::LoadEncodedScalar | MachineSemanticKind::StoreEncodedScalar => {
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 16,
                maximum_bytes: None,
            }
        }
        MachineSemanticKind::ScanByteSequence => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 20,
            maximum_bytes: None,
        },
        MachineSemanticKind::CompareByteSequences => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 32,
            maximum_bytes: None,
        },
        MachineSemanticKind::StorePacked => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 24,
            maximum_bytes: Some(56),
        },
        MachineSemanticKind::MaterializeI64 => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 4,
            maximum_bytes: Some(16),
        },
        MachineSemanticKind::ImageAddress => MachineSizeKnowledge::ExactBytes(8),
        MachineSemanticKind::HostedExitProcessI32
        | MachineSemanticKind::HostedReadByte
        | MachineSemanticKind::SaveFloatingControl
        | MachineSemanticKind::RestoreFloatingControl
        | MachineSemanticKind::HostedWriteByteI32
        | MachineSemanticKind::CallUnit => {
            panic!("memory and Unit call forms are not admitted on this target")
        }
        MachineSemanticKind::CallScalar
        | MachineSemanticKind::InvokeNoNormal
        | MachineSemanticKind::CallAggregate
        | MachineSemanticKind::NormalizedForeignCall => {
            panic!("scalar calls use their dedicated declaration")
        }
        MachineSemanticKind::ExactDivideU64
        | MachineSemanticKind::WrappingDivideI64
        | MachineSemanticKind::ExactDivideI64 => MachineSizeKnowledge::ExactBytes(4),
        MachineSemanticKind::ExactRemainderU64
        | MachineSemanticKind::WrappingRemainderI64
        | MachineSemanticKind::ExactRemainderI64 => MachineSizeKnowledge::ExactBytes(8),
        MachineSemanticKind::FrameAddress => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 4,
            maximum_bytes: Some(crate::immediate_offset::MAX_ADDRESS_BYTES * 2 + 4),
        },
        MachineSemanticKind::AddressOffset => MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 4,
            maximum_bytes: Some(crate::immediate_offset::MAX_ADDRESS_BYTES),
        },
        MachineSemanticKind::ConditionalBranchNonZero
        | MachineSemanticKind::ConditionalBranchU64LessThan
        | MachineSemanticKind::ConditionalBranchI64LessThan => {
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes: Some(8),
            }
        }
        _ => MachineSizeKnowledge::ExactBytes(4),
    }
}

const fn packed_load_width(semantic: MachineSemanticKind) -> Option<u16> {
    match semantic {
        MachineSemanticKind::LoadPacked3 => Some(3),
        MachineSemanticKind::LoadPacked5 => Some(5),
        MachineSemanticKind::LoadPacked6 => Some(6),
        MachineSemanticKind::LoadPacked7 => Some(7),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AARCH64_AAPCS64_RETURN, AARCH64_AAPCS64_RETURN_UNIT, AARCH64_CONDITIONAL_BRANCH,
        AARCH64_COPY_I64, AARCH64_DARWIN_RETURN, AARCH64_DARWIN_RETURN_UNIT, AARCH64_SUBTRACT_I64,
        Aarch64MachineEffectCatalogValidationError, Architecture, MachineAlternativeApplicability,
        MachineBarrier, MachineCallEffect, MachineEncodedControlEffect, MachineEncodedEffects,
        MachineSemanticKind, MachineSizeKnowledge, NativeTarget, ObjectFormat,
        PreparedRegisterConstraintCatalog, aarch64_machine_effect_catalog,
        validate_aarch64_machine_effect_catalog,
    };
    use crate::{
        Aarch64RegisterConstraintCatalogValidationError, aarch64_physical_register_model,
        aarch64_register_constraint_catalog, validate_aarch64_register_constraint_catalog,
    };
    use register_model::validate_physical_register_model;
    use selected_instructions::MachineEncodedStackEffect;

    #[test]
    fn effect_classification_keeps_operand_and_implicit_effects_together() {
        let physical = aarch64_physical_register_model();
        let flags = physical.view_named("nzcv").unwrap().units.clone();
        let comparison = super::encoded_effects(MachineSemanticKind::CompareI64).unwrap();
        assert_eq!(comparison.external_operand_reads, [0, 1]);
        assert_eq!(comparison.implicit_unit_defs, flags);

        let subtract = super::encoded_effects(MachineSemanticKind::WrappingSubtractI64).unwrap();
        assert_eq!(subtract.external_operand_reads, [0, 1]);
        assert_eq!(subtract.external_operand_writes, [2]);
        assert!(subtract.implicit_unit_uses.is_empty());
        assert!(subtract.implicit_unit_defs.is_empty());
        assert!(subtract.implicit_unit_clobbers.is_empty());

        let copied = super::encoded_effects(MachineSemanticKind::CopyBytes).unwrap();
        assert_eq!(copied.external_operand_reads, [0, 1, 2]);
        assert_eq!(copied.implicit_unit_clobbers, flags);
        assert_eq!(
            copied.trap,
            super::MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        );
        assert!(matches!(
            copied.memory,
            super::MachineEncodedMemoryEffect::CopyBytesV1 { .. }
        ));

        let loaded = super::encoded_effects(MachineSemanticKind::LoadPacked5).unwrap();
        assert_eq!(loaded.external_operand_writes, [1, 2]);
        assert!(matches!(
            loaded.memory,
            super::MachineEncodedMemoryEffect::ReadPointerV1 { byte_count: 5, .. }
        ));
        let returned = super::encoded_effects(MachineSemanticKind::ReturnScalar).unwrap();
        assert!(matches!(
            returned.control,
            MachineEncodedControlEffect::ReturnIndirectRegisterV1 { .. }
        ));
    }

    #[test]
    fn effect_classification_requires_context_and_rejects_other_architecture() {
        for semantic in [
            MachineSemanticKind::CallScalar,
            MachineSemanticKind::NormalizedForeignCall,
            MachineSemanticKind::PortWrite,
            MachineSemanticKind::PrivilegedCrash,
        ] {
            assert!(matches!(
                super::encoded_effects(semantic),
                Err(Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch)
            ));
        }
    }

    fn constraints() -> PreparedRegisterConstraintCatalog {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        validate_aarch64_register_constraint_catalog(
            aarch64_register_constraint_catalog(&physical),
            &physical,
        )
        .unwrap_or_else(|error: Aarch64RegisterConstraintCatalogValidationError| panic!("{error}"))
    }

    macro_rules! eq {
        ($left:expr => $right:expr) => {
            assert_eq!($left, $right);
        };
    }

    #[test]
    fn every_saturating_carrier_binds_its_shape_key_size_and_flags() {
        use selected_instructions::{SaturatingCarrier, SaturatingOperation};
        let constraints = constraints();
        let nzcv = crate::aarch64_physical_register_model()
            .view_named("nzcv")
            .unwrap()
            .units
            .clone();
        for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
            let catalog = aarch64_machine_effect_catalog(target, &constraints).unwrap();
            for carrier in SaturatingCarrier::ALL {
                let signed = carrier.is_signed();
                let narrow = carrier.is_narrow();
                for (operation, semantic) in [
                    (
                        SaturatingOperation::Add,
                        MachineSemanticKind::SaturatingAdd(carrier),
                    ),
                    (
                        SaturatingOperation::Subtract,
                        MachineSemanticKind::SaturatingSubtract(carrier),
                    ),
                    (
                        SaturatingOperation::Divide,
                        MachineSemanticKind::SaturatingDivide(carrier),
                    ),
                    (
                        SaturatingOperation::Remainder,
                        MachineSemanticKind::SaturatingRemainder(carrier),
                    ),
                    (
                        SaturatingOperation::Multiply,
                        MachineSemanticKind::SaturatingMultiply(carrier),
                    ),
                ] {
                    let (key, size, writes): (_, u16, &[u16]) = match operation {
                        SaturatingOperation::Add if carrier == SaturatingCarrier::U64 => {
                            (crate::register_model::AARCH64_SATURATING_ADD_U64, 8, &[2])
                        }
                        SaturatingOperation::Add => (
                            crate::register_model::AARCH64_SATURATING_ADD_CLAMPED,
                            if !narrow {
                                16
                            } else if signed {
                                28
                            } else {
                                16
                            },
                            &[2, 3],
                        ),
                        SaturatingOperation::Subtract if !signed => (
                            crate::register_model::AARCH64_SATURATING_SUBTRACT_UNSIGNED,
                            8,
                            &[2],
                        ),
                        SaturatingOperation::Subtract => (
                            crate::register_model::AARCH64_SATURATING_SUBTRACT_CLAMPED,
                            if narrow { 28 } else { 16 },
                            &[2, 3],
                        ),
                        SaturatingOperation::Divide if !signed => {
                            (crate::register_model::AARCH64_DIVIDE_U64, 4, &[2])
                        }
                        SaturatingOperation::Divide => (
                            crate::register_model::AARCH64_SATURATING_DIVIDE_SIGNED,
                            if narrow { 16 } else { 24 },
                            &[2, 3],
                        ),
                        SaturatingOperation::Remainder if !signed => {
                            (crate::register_model::AARCH64_REMAINDER_U64, 8, &[2])
                        }
                        SaturatingOperation::Remainder => {
                            (crate::register_model::AARCH64_REMAINDER_I64, 8, &[2])
                        }
                        SaturatingOperation::Multiply if carrier == SaturatingCarrier::U64 => (
                            crate::register_model::AARCH64_SATURATING_MULTIPLY_U64,
                            16,
                            &[2, 3],
                        ),
                        SaturatingOperation::Multiply => (
                            crate::register_model::AARCH64_SATURATING_MULTIPLY_CLAMPED,
                            if !narrow {
                                24
                            } else if signed {
                                28
                            } else {
                                16
                            },
                            &[2, 3],
                        ),
                    };
                    let declaration = catalog
                        .declaration_by_semantic(semantic)
                        .unwrap_or_else(|| panic!("{semantic:?} declared"));
                    assert_eq!(declaration.constraint, key, "{semantic:?}");
                    assert_eq!(declaration.alternatives.len(), 1);
                    let alternative = &declaration.alternatives[0];
                    assert_eq!(
                        alternative.size,
                        MachineSizeKnowledge::ExactBytes(size),
                        "{semantic:?}"
                    );
                    assert_eq!(alternative.encoded.external_operand_reads, [0, 1]);
                    assert_eq!(alternative.encoded.external_operand_writes, writes);
                    let defines_nzcv = !(operation == SaturatingOperation::Divide && !signed)
                        && operation != SaturatingOperation::Remainder;
                    assert_eq!(
                        alternative.encoded.implicit_unit_defs,
                        if defines_nzcv {
                            nzcv.clone()
                        } else {
                            Vec::new()
                        },
                        "{semantic:?}"
                    );
                    for corruption in 0..3 {
                        let mut changed = catalog.clone();
                        let alternative = &mut changed
                            .declaration_by_semantic_mut(semantic)
                            .unwrap()
                            .alternatives[0];
                        match corruption {
                            0 => alternative.encoded.external_operand_writes.clear(),
                            1 => alternative.size = MachineSizeKnowledge::ExactBytes(size + 4),
                            _ => {
                                let sibling = match carrier {
                                    SaturatingCarrier::I8 => SaturatingCarrier::U8,
                                    SaturatingCarrier::I16 => SaturatingCarrier::U16,
                                    SaturatingCarrier::I32 => SaturatingCarrier::U32,
                                    SaturatingCarrier::I64 => SaturatingCarrier::U64,
                                    SaturatingCarrier::U8 => SaturatingCarrier::I8,
                                    SaturatingCarrier::U16 => SaturatingCarrier::I16,
                                    SaturatingCarrier::U32 => SaturatingCarrier::I32,
                                    SaturatingCarrier::U64 => SaturatingCarrier::I64,
                                };
                                alternative.key.family = match operation {
                                    SaturatingOperation::Add => {
                                        MachineSemanticKind::SaturatingAdd(sibling)
                                    }
                                    SaturatingOperation::Subtract => {
                                        MachineSemanticKind::SaturatingSubtract(sibling)
                                    }
                                    SaturatingOperation::Divide => {
                                        MachineSemanticKind::SaturatingDivide(sibling)
                                    }
                                    SaturatingOperation::Remainder => {
                                        MachineSemanticKind::SaturatingRemainder(sibling)
                                    }
                                    SaturatingOperation::Multiply => {
                                        MachineSemanticKind::SaturatingMultiply(sibling)
                                    }
                                }
                                .into();
                            }
                        };
                        assert!(
                            validate_aarch64_machine_effect_catalog(target, &constraints, changed)
                                .is_err(),
                            "{semantic:?} corruption {corruption}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn wrapping_remainder_catalog_preserves_inputs_and_flags() {
        let constraints = constraints();
        for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
            let catalog = aarch64_machine_effect_catalog(target, &constraints).unwrap();
            let declaration = catalog
                .declaration_by_semantic(MachineSemanticKind::WrappingRemainderI64)
                .unwrap();
            eq!(declaration.constraint => crate::register_model::AARCH64_REMAINDER_I64);
            eq!(declaration.alternatives[0].size => MachineSizeKnowledge::ExactBytes(8));
            assert_eq!(
                declaration.alternatives[0].encoded,
                MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2])
            );
            for corruption in 0..3 {
                let mut changed = catalog.clone();
                let alternative = &mut changed
                    .declaration_by_semantic_mut(MachineSemanticKind::WrappingRemainderI64)
                    .unwrap()
                    .alternatives[0];
                match corruption {
                    0 => alternative.encoded.external_operand_reads.truncate(1),
                    1 => alternative.size = MachineSizeKnowledge::ExactBytes(4),
                    _ => alternative.key.family = MachineSemanticKind::ExactDivideU64.into(),
                };
                assert!(
                    validate_aarch64_machine_effect_catalog(target, &constraints, changed).is_err()
                );
            }
        }
    }

    #[test]
    fn bitwise_xor_catalog_binds_family_size_and_pure_effects() {
        for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
            let constraints = constraints();
            let catalog = aarch64_machine_effect_catalog(target, &constraints).unwrap();
            validate_aarch64_machine_effect_catalog(target, &constraints, catalog.clone()).unwrap();
            let declaration = catalog
                .declaration_by_semantic(MachineSemanticKind::BitwiseXorI64)
                .unwrap();
            assert_eq!(declaration.alternatives.len(), 1);
            let alternative = &declaration.alternatives[0];
            eq!(alternative.key.family => MachineSemanticKind::BitwiseXorI64.into());
            assert_eq!(alternative.size, MachineSizeKnowledge::ExactBytes(4));
            eq!(alternative.encoded => MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2]));
            for corruption in 0..3 {
                let mut changed = catalog.clone();
                let alternative = &mut changed
                    .declaration_by_semantic_mut(MachineSemanticKind::BitwiseXorI64)
                    .unwrap()
                    .alternatives[0];
                match corruption {
                    0 => alternative.key.family = MachineSemanticKind::BitwiseAndI64.into(),
                    1 => alternative.encoded.external_operand_reads.truncate(1),
                    _ => alternative.size = MachineSizeKnowledge::ExactBytes(8),
                }
                assert!(
                    validate_aarch64_machine_effect_catalog(target, &constraints, changed).is_err()
                );
            }
        }
    }

    #[test]
    fn integer_normalization_catalog_binds_identity_size_and_effects() {
        for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
            let constraints = constraints();
            let catalog = aarch64_machine_effect_catalog(target, &constraints).unwrap();
            validate_aarch64_machine_effect_catalog(target, &constraints, catalog.clone()).unwrap();
            for semantic in [
                MachineSemanticKind::ZeroExtendU16,
                MachineSemanticKind::SignExtendI8,
                MachineSemanticKind::SignExtendI16,
                MachineSemanticKind::SignExtendI32,
            ] {
                let declaration = catalog.declaration_by_semantic(semantic).unwrap();
                assert_eq!(declaration.constraint, AARCH64_COPY_I64);
                assert_eq!(declaration.alternatives.len(), 1);
                let alternative = &declaration.alternatives[0];
                assert_eq!(alternative.key.family, semantic.into());
                assert_eq!(alternative.size, MachineSizeKnowledge::ExactBytes(4));
                eq!(alternative.encoded => MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]));
                for corruption in 0..3 {
                    let mut changed = catalog.clone();
                    let row = changed.declaration_by_semantic_mut(semantic).unwrap();
                    match corruption {
                        0 => row.alternatives[0].key.family = MachineSemanticKind::CopyI64.into(),
                        1 => row.alternatives[0].size = MachineSizeKnowledge::ExactBytes(1),
                        _ => row.alternatives[0].encoded.external_operand_reads.clear(),
                    }
                    assert!(
                        validate_aarch64_machine_effect_catalog(target, &constraints, changed)
                            .is_err()
                    );
                }
            }
        }
    }

    #[test]
    fn catalog_declares_one_flag_transparent_subtraction_alternative() {
        for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
            let constraints = constraints();
            let catalog = aarch64_machine_effect_catalog(target, &constraints).unwrap();
            let subtract = catalog
                .declaration_by_semantic(MachineSemanticKind::ExactSubtractI64)
                .unwrap();
            assert_eq!(subtract.constraint, AARCH64_SUBTRACT_I64);
            let register_effects = constraints
                .catalog()
                .constraint(subtract.constraint)
                .unwrap();
            assert!(register_effects.implicit_uses.is_empty());
            assert!(register_effects.implicit_defs.is_empty());
            assert!(register_effects.clobbers.is_empty());
            assert_eq!(subtract.alternatives.len(), 1);
            eq!(subtract.alternatives[0].applicability => MachineAlternativeApplicability::Always);
            eq!(subtract.alternatives[0].size => MachineSizeKnowledge::ExactBytes(4));
            let less_than_branch = catalog
                .declaration_by_semantic(MachineSemanticKind::ConditionalBranchU64LessThan)
                .unwrap();
            let signed_less_than_branch = catalog
                .declaration_by_semantic(MachineSemanticKind::ConditionalBranchI64LessThan)
                .unwrap();
            assert_eq!(less_than_branch.constraint, AARCH64_CONDITIONAL_BRANCH);
            assert_eq!(less_than_branch.barrier, MachineBarrier::ControlFlow);
            assert_eq!(less_than_branch.alternatives.len(), 1);
            assert_eq!(
                less_than_branch.alternatives[0].size,
                MachineSizeKnowledge::EncoderResolved {
                    minimum_bytes: 4,
                    maximum_bytes: Some(8),
                }
            );
            assert_eq!(
                less_than_branch.alternatives[0].encoded.control,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            );
            eq!(signed_less_than_branch.constraint => AARCH64_CONDITIONAL_BRANCH);
            assert_eq!(signed_less_than_branch.alternatives.len(), 1);
            assert_eq!(
                signed_less_than_branch.alternatives[0].key.family,
                selected_instructions::MachineAlternativeFamily::ConditionalBranchI64LessThan
            );
            assert_eq!(
                signed_less_than_branch.alternatives[0].size,
                MachineSizeKnowledge::EncoderResolved {
                    minimum_bytes: 4,
                    maximum_bytes: Some(8),
                }
            );
            let scalar_call = catalog.declaration_by_semantic(MachineSemanticKind::CallScalar);
            {
                let scalar_call = scalar_call.expect("supported target declares scalar call");
                assert_eq!(
                    scalar_call.call,
                    MachineCallEffect::InternalNormalReturnV1 {
                        pre_call_stack_alignment: 16,
                    }
                );
                eq!(scalar_call.alternatives[0].size => MachineSizeKnowledge::ExactBytes(4));
                assert_eq!(
                    scalar_call.alternatives[0].encoded.control,
                    MachineEncodedControlEffect::DirectRelativeCallV1
                );
                assert_eq!(
                    scalar_call.alternatives[0].encoded.stack,
                    MachineEncodedStackEffect::UnchangedV1
                );
            }
            assert!(validate_aarch64_machine_effect_catalog(target, &constraints, catalog).is_ok());
            let catalog = aarch64_machine_effect_catalog(target, &constraints).unwrap();
            let return_unit = catalog
                .declaration_by_semantic(MachineSemanticKind::ReturnUnit)
                .unwrap();
            assert!(
                constraints
                    .catalog()
                    .constraint(return_unit.constraint)
                    .unwrap()
                    .operands
                    .is_empty()
            );
        }
    }

    #[test]
    fn semantic_validator_rejects_invented_alias_or_size_semantics() {
        let target = NativeTarget::linux_arm64();
        let constraints = constraints();
        let mut wrong = aarch64_machine_effect_catalog(target, &constraints).unwrap();
        let subtract = wrong
            .declaration_by_semantic_mut(MachineSemanticKind::ExactSubtractI64)
            .unwrap();
        subtract.alternatives[0].applicability =
            MachineAlternativeApplicability::ResultAliasesOperand {
                result: 2,
                operand: 0,
            };
        assert_eq!(
            validate_aarch64_machine_effect_catalog(target, &constraints, wrong),
            Err(Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch)
        );

        let mut wrong = aarch64_machine_effect_catalog(target, &constraints).unwrap();
        wrong
            .declaration_by_semantic_mut(MachineSemanticKind::MaterializeI64)
            .unwrap()
            .alternatives[0]
            .size = MachineSizeKnowledge::ExactBytes(4);
        assert_eq!(
            validate_aarch64_machine_effect_catalog(target, &constraints, wrong),
            Err(Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch)
        );
    }

    #[test]
    fn selected_keys_declare_the_supported_aarch64_pairs() {
        let constraints = constraints();
        for (target, darwin, return_i64, return_unit, read, exit, write) in [
            (
                NativeTarget::linux_arm64(),
                false,
                AARCH64_AAPCS64_RETURN,
                AARCH64_AAPCS64_RETURN_UNIT,
                crate::AARCH64_HOSTED_READ_BYTE,
                crate::AARCH64_HOSTED_EXIT_PROCESS_I32,
                crate::AARCH64_HOSTED_WRITE_BYTE_I32,
            ),
            (
                NativeTarget::macos_arm64(),
                true,
                AARCH64_DARWIN_RETURN,
                AARCH64_DARWIN_RETURN_UNIT,
                crate::AARCH64_DARWIN_HOSTED_READ_BYTE,
                crate::AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32,
                crate::AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32,
            ),
        ] {
            let keys = aarch64_machine_effect_catalog(target, &constraints)
                .unwrap()
                .selected_keys;
            assert_eq!(keys.return_i64, return_i64);
            assert_eq!(keys.return_unit, return_unit);
            assert_eq!(keys.hosted_read_byte, Some(read));
            assert_eq!(keys.hosted_exit_process_i32, Some(exit));
            assert_eq!(keys.hosted_write_byte_i32, Some(write));
            assert_eq!(
                keys.call_unit,
                if darwin {
                    crate::aarch64_darwin_register_unit_call_keys()
                } else {
                    crate::aarch64_aapcs64_register_unit_call_keys()
                }
            );
            assert_eq!(
                keys.call_unit_mixed,
                if darwin {
                    crate::aarch64_darwin_mixed_unit_call_keys()
                } else {
                    crate::aarch64_aapcs64_mixed_unit_call_keys()
                }
            );
            let mut expected_scalar = if darwin {
                crate::aarch64_darwin_register_call_keys()
            } else {
                crate::aarch64_aapcs64_register_call_keys()
            };
            expected_scalar.extend(crate::aarch64_float_scalar_call_keys(darwin));
            assert_eq!(keys.call_scalar, expected_scalar);
            assert_eq!(
                keys.call_aggregate,
                crate::aarch64_register_aggregate_call_keys(darwin)
                    .into_iter()
                    .chain(crate::aarch64_mixed_aggregate_call_keys(darwin))
                    .chain(crate::aarch64_indirect_aggregate_call_keys(darwin))
                    .collect::<Vec<_>>()
            );
            eq!(keys.return_aggregate => crate::aarch64_register_aggregate_return_keys(darwin));
            eq!(keys.return_float => crate::aarch64_float_scalar_return_keys(darwin));
        }
    }

    #[test]
    fn selected_keys_fail_closed_on_undeclared_pairs() {
        let constraints = constraints();
        let coff = NativeTarget {
            architecture: Architecture::Aarch64,
            object_format: ObjectFormat::Coff,
            pointer_size: 8,
            pointer_alignment: 8,
        };
        assert_eq!(
            aarch64_machine_effect_catalog(coff, &constraints),
            Err(Aarch64MachineEffectCatalogValidationError::UnsupportedTargetAbi)
        );
    }

    #[test]
    fn hosted_rows_follow_the_declared_target_not_the_format_shape() {
        let constraints = constraints();
        for (undeclared, return_i64) in [
            (
                NativeTarget {
                    pointer_size: 4,
                    pointer_alignment: 4,
                    ..NativeTarget::linux_arm64()
                },
                AARCH64_AAPCS64_RETURN,
            ),
            (
                NativeTarget {
                    pointer_size: 4,
                    pointer_alignment: 4,
                    ..NativeTarget::macos_arm64()
                },
                AARCH64_DARWIN_RETURN,
            ),
        ] {
            let keys = aarch64_machine_effect_catalog(undeclared, &constraints)
                .unwrap()
                .selected_keys;
            assert_eq!(keys.return_i64, return_i64);
            assert!(keys.hosted_write_byte_i32.is_none());
            assert!(keys.hosted_read_byte.is_none());
            assert!(keys.hosted_exit_process_i32.is_none());
        }
    }
}
