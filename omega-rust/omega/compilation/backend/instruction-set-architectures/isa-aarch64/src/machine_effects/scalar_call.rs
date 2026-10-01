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
        .constraints
        .iter()
        .find(|row| row.key == constraint)
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
    declaration.call = MachineCallEffect::DirectExternalNormalReturnV1 {
        pre_call_stack_alignment: 16,
    };
    // Callback rows materialize the private callback address inside the call
    // encoding: an `adrp`/`add` pair per register destination, plus a scratch
    // store for field destinations, precede the `bl` word. The applicable
    // alternative is the row's declared shape.
    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == constraint)
        .expect("canonical AArch64 catalog contains its normalized-foreign-call constraint");
    let shape = crate::register_model::aarch64_normalized_foreign_callback_shape(
        row,
        crate::aarch64_physical_register_model_ref(),
    );
    for alternative in &mut declaration.alternatives {
        alternative.key.variant = shape.alternative_variant();
        // Field destinations may repeat: one adrp/add/str group (12 bytes)
        // apiece, plus the 8-byte register group and the 4-byte `bl` word.
        alternative.size = if shape.field_scratch {
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4 + 12 + if shape.parameter_slot.is_some() { 8 } else { 0 },
                maximum_bytes: None,
            }
        } else if shape.parameter_slot.is_some() {
            MachineSizeKnowledge::ExactBytes(12)
        } else {
            alternative.size
        };
    }
    declaration
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
