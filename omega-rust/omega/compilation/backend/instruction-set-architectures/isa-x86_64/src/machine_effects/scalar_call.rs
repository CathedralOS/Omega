//! Exact x86-64 call effects follow the selected ABI's canonical constraint row.

use register_model::{RegisterConstraintKey, ValidatedRegisterConstraintCatalog};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey, MachineBarrier,
    MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior,
};

/// The canonical user-mode direct call: the row's external footprint and
/// implicit units under the return-address write/lifecycle pair. Supervisor
/// calls share this head and override the memory/stack/control contract.
pub(crate) fn user_direct_call_effects(
    row: &register_model::RegisterInstructionConstraint,
    stack_pointer: register_model::RegisterViewId,
) -> MachineEncodedEffects {
    MachineEncodedEffects {
        external_operand_reads: row
            .operands
            .iter()
            .filter(|operand| operand.access == register_model::RegisterOperandAccess::Use)
            .map(|operand| operand.operand)
            .collect(),
        external_operand_writes: row
            .operands
            .iter()
            .filter(|operand| operand.access == register_model::RegisterOperandAccess::Def)
            .map(|operand| operand.operand)
            .collect(),
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
    let physical = crate::x86_64_physical_register_model_ref();
    let stack_pointer = physical
        .view_named("rsp")
        .expect("canonical x86-64 model declares rsp")
        .id;
    MachineEffectDeclaration {
        semantic,
        constraint,
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::NeverV1,
        barrier: MachineBarrier::Call,
        call: MachineCallEffect::InternalNormalReturnV1 {
            pre_call_stack_alignment: 16,
        },
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(5),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: user_direct_call_effects(row, stack_pointer),
        }],
    }
}

/// An imported foreign call shares the internal-call row effects — operand
/// views, return-address stack lifecycle, implicit uses/defs, and clobbers all
/// come from the exact selected constraint row — but its callee is an external
/// import, so the call effect declares the external normal-return form.
pub(super) fn normalized_foreign_declaration(
    constraint: RegisterConstraintKey,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> MachineEffectDeclaration {
    let mut declaration = declaration(
        MachineSemanticKind::NormalizedForeignCall,
        constraint,
        constraints,
    );
    if constraint.family == register_model::RegisterConstraintFamily::SystemCall {
        declaration.call = MachineCallEffect::SupervisorNormalReturnV1;
        for alternative in &mut declaration.alternatives {
            // Fixed-width number materialization followed by supervisor entry.
            // The number is encoded inside the form, not a source operand.
            alternative.size = MachineSizeKnowledge::ExactBytes(12);
            alternative.encoded.memory = MachineEncodedMemoryEffect::NoneV1;
            alternative.encoded.stack = MachineEncodedStackEffect::UnchangedV1;
            alternative.encoded.control = MachineEncodedControlEffect::SupervisorCallV1;
        }
        return declaration;
    }
    declaration.call = MachineCallEffect::DirectExternalNormalReturnV1 {
        pre_call_stack_alignment: 16,
    };
    declaration
}

/// Instantiate the complete x86-64 effects from the retained call contract.
/// The supplied operand row must equal independent reconstruction from the
/// canonical zero-operand mechanism template; it cannot assert its own effects.
pub fn x86_64_normalized_foreign_call_declaration(
    record: &selected_instructions::SelectedNormalizedForeignCall,
    row: &register_model::RegisterInstructionConstraint,
) -> Option<MachineEffectDeclaration> {
    let expected = normalized_foreign_constraint(record)?;
    if *row != expected {
        return None;
    }
    let supervisor = record.call.binding.system_call().is_some();
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
        match callback.destination {
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
                if *register == calling_conventions::MachineRegister::X86R11
                    || callback.field_byte_offset? > i32::MAX as usize
                {
                    return None;
                }
                let bytes = if *register == calling_conventions::MachineRegister::X86R12 {
                    15
                } else {
                    14
                };
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
    } else {
        5_u16.checked_add(callback_byte_count)?
    };
    Some(MachineEffectDeclaration {
        semantic: MachineSemanticKind::NormalizedForeignCall,
        constraint: row.key,
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::NeverV1,
        barrier: MachineBarrier::Call,
        call: if supervisor {
            MachineCallEffect::SupervisorNormalReturnV1
        } else {
            MachineCallEffect::DirectExternalNormalReturnV1 {
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
                if supervisor {
                    encoded.memory = MachineEncodedMemoryEffect::NoneV1;
                    encoded.stack = MachineEncodedStackEffect::UnchangedV1;
                    encoded.control = MachineEncodedControlEffect::SupervisorCallV1;
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
    use super::*;
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
        for key in crate::x86_64_linux_system_call_keys() {
            let row = catalog
                .declarations
                .iter()
                .find(|row| row.constraint == key)
                .unwrap();
            assert_eq!(row.semantic, MachineSemanticKind::NormalizedForeignCall);
            assert_eq!(row.call, MachineCallEffect::SupervisorNormalReturnV1);
            let encoded = &row.alternatives[0].encoded;
            assert_eq!(
                encoded.control,
                MachineEncodedControlEffect::SupervisorCallV1
            );
            assert_eq!(encoded.memory, MachineEncodedMemoryEffect::NoneV1);
            assert_eq!(encoded.stack, MachineEncodedStackEffect::UnchangedV1);
            for corruption in 0..4 {
                let mut forged = catalog.clone();
                let row = forged
                    .declarations
                    .iter_mut()
                    .find(|row| row.constraint == key)
                    .unwrap();
                match corruption {
                    0 => {
                        row.call = MachineCallEffect::DirectExternalNormalReturnV1 {
                            pre_call_stack_alignment: 16,
                        }
                    }
                    1 => {
                        row.alternatives[0].encoded.control =
                            MachineEncodedControlEffect::DirectRelativeCallV1
                    }
                    2 => {
                        row.alternatives[0].encoded.stack =
                            MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                                stack_pointer: physical.model().view_named("rsp").unwrap().id,
                                return_address_byte_count: 8,
                            }
                    }
                    _ => row.alternatives[0].encoded.implicit_unit_clobbers.clear(),
                }
                // Target-neutral admission must catch these without relying
                // on equality against the ISA producer's entire catalog.
                assert!(
                    selected_instructions::validate_machine_effect_catalog(&constraints, forged)
                        .is_err()
                );
            }
        }
        let other_target = NativeTarget::macos_x64();
        let other = crate::x86_64_machine_effect_catalog(other_target, &constraints).unwrap();
        assert!(
            other
                .selected_keys
                .call_normalized_foreign
                .iter()
                .all(|key| { key.family != register_model::RegisterConstraintFamily::SystemCall })
        );
    }
}
