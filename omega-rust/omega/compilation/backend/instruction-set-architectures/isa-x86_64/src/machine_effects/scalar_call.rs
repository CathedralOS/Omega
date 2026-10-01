//! Exact x86-64 call effects follow the selected ABI's canonical constraint row.

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
        .constraints
        .iter()
        .find(|row| row.key == constraint)
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
        call: MachineCallEffect::DirectInternalNormalReturnV1 {
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
            },
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
    // Callback rows materialize private function addresses inside the call
    // encoding: each private parameter slot is a `lea <destination>,
    // [rip+disp32]` prefix and each field destination a `lea r11` + slot
    // store pair, all before the call field. The row's implicit uses name
    // which families apply; the byte count grows with each destination, so
    // field-bearing rows resolve their size at encoding time.
    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == constraint)
        .expect("canonical x86-64 catalog contains its normalized-foreign-call constraint");
    let shape = crate::register_model::x86_64_normalized_foreign_callback_shape(
        row,
        crate::x86_64_physical_register_model_ref(),
    );
    let variant = shape.alternative_variant();
    if variant != 0 {
        for alternative in &mut declaration.alternatives {
            alternative.key.variant = variant;
            alternative.size = if shape.field_scratch {
                // One scratch prefix + slot store per field destination; the
                // roster, not the row, counts them.
                MachineSizeKnowledge::EncoderResolved {
                    minimum_bytes: 5 + u16::from(shape.parameter_slot.is_some()) * 7 + 14,
                    maximum_bytes: None,
                }
            } else {
                MachineSizeKnowledge::ExactBytes(12)
            };
        }
    }
    declaration
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
