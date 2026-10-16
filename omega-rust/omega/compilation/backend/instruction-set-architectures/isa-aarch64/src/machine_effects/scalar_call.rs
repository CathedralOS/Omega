//! Exact AAPCS64 scalar-call semantic and encoded machine effects.

use register_model::{RegisterConstraintKey, ValidatedRegisterConstraintCatalog};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey, MachineBarrier,
    MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior,
};

pub(super) fn declaration(
    semantic: MachineSemanticKind,
    constraint: RegisterConstraintKey,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> MachineEffectDeclaration {
    let row = constraints
        .catalog()
        .constraint(constraint)
        .expect("canonical AArch64 catalog contains its scalar-call constraint");
    let arity = row
        .operands
        .iter()
        .take_while(|operand| operand.access == register_model::RegisterOperandAccess::Use)
        .count();
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
            size: MachineSizeKnowledge::ExactBytes(4),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: MachineEncodedEffects {
                external_operand_reads: (0..arity as u16).collect(),
                external_operand_writes: (arity as u16..row.operands.len() as u16).collect(),
                implicit_unit_uses: row.implicit_uses.clone(),
                implicit_unit_defs: row.implicit_defs.clone(),
                implicit_unit_clobbers: row.clobbers.clone(),
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::DirectRelativeCallV1,
            },
        }],
    }
}

/// An imported foreign call shares the internal-call row effects — operand
/// views, implicit uses/defs, and clobbers all come from the exact selected
/// constraint row — but its callee is an external import, so the call effect
/// declares the external normal-return form.
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
            alternative.size = MachineSizeKnowledge::ExactBytes(20);
            alternative.encoded.memory = MachineEncodedMemoryEffect::NoneV1;
            alternative.encoded.stack = MachineEncodedStackEffect::UnchangedV1;
            alternative.encoded.control = MachineEncodedControlEffect::SupervisorCallV1;
        }
        return declaration;
    }
    declaration.call = MachineCallEffect::ExternalNormalReturnV1 {
        pre_call_stack_alignment: 16,
    };
    declaration
}

/// Instantiate the complete AArch64 effects from the retained call contract.
/// The supplied operand row must equal independent reconstruction from the
/// canonical zero-operand mechanism template; it cannot assert its own effects.
pub fn aarch64_normalized_foreign_call_declaration(
    record: &selected_instructions::SelectedNormalizedForeignCall,
    row: &register_model::RegisterInstructionConstraint,
) -> Option<MachineEffectDeclaration> {
    let expected = normalized_foreign_constraint(record)?;
    if *row != expected {
        return None;
    }
    let supervisor = record.call.binding.system_call().is_some();
    let mut register_destinations = 0_u16;
    let mut field_destinations = 0_u16;
    for callback in &record.call.callbacks {
        if Some(callback.terminal_operation) != record.invocation_site.operation() {
            return None;
        }
        match callback.destination {
            calling_conventions::NativePlace::Parameter(_) => {
                if !matches!(
                    callback.application.placement.locations.as_slice(),
                    [calling_conventions::ValueLocation::Register { .. }]
                ) {
                    return None;
                }
                register_destinations = register_destinations.checked_add(1)?;
            }
            calling_conventions::NativePlace::Field { .. } => {
                field_destinations = field_destinations.checked_add(1)?;
            }
        }
    }
    if supervisor && !record.call.callbacks.is_empty() {
        return None;
    }
    let variant = u32::from(register_destinations != 0) | (u32::from(field_destinations != 0) << 1);
    let byte_count = if supervisor {
        20
    } else {
        4_u16
            .checked_add(register_destinations.checked_mul(8)?)?
            .checked_add(field_destinations.checked_mul(12)?)?
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
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: if supervisor {
                    MachineEncodedControlEffect::SupervisorCallV1
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
    use super::*;
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
        for key in crate::aarch64_linux_system_call_keys() {
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
                        row.call = MachineCallEffect::ExternalNormalReturnV1 {
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
                                stack_pointer: physical.model().view_named("sp").unwrap().id,
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
        let other_target = NativeTarget::macos_arm64();
        let other = crate::aarch64_machine_effect_catalog(other_target, &constraints).unwrap();
        assert!(
            other
                .selected_keys
                .call_normalized_foreign
                .iter()
                .all(|key| { key.family != register_model::RegisterConstraintFamily::SystemCall })
        );
    }
}
