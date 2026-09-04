use omega_selected_instructions::{
    MachineBarrier, MachineCallEffect, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSizeKnowledge, MachineTrapBehavior,
};
use omega_target::Architecture;

use crate::tests::*;

use super::fixture::{caller_machine, staged_homes, staged_selected};

fn first_call(
    plan: &mut omega_machine_optimizer::PreAllocationMachineEffectPlan,
) -> &mut omega_machine_optimizer::InstructionMachineEffects {
    plan.functions
        .iter_mut()
        .find(|function| function.machine == caller_machine())
        .unwrap()
        .blocks[0]
        .instructions
        .iter_mut()
        .find(|instruction| matches!(instruction.kind, SelectedInstructionKind::CallI64 { .. }))
        .unwrap()
}

#[test]
fn scalar_calls_retain_exact_effects_through_post_allocation_persistence() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selected = staged_selected(target);
        let effects =
            analyze_machine_effects(selected.selected(), selected.register_environment()).unwrap();
        let function = effects
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        let calls = function.blocks[0]
            .instructions
            .iter()
            .filter(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::CallI64 { .. })
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 3);
        let selected_calls = selected
            .selected()
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap()
            .blocks[0]
            .instructions
            .iter()
            .filter(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::CallI64 { .. })
            })
            .collect::<Vec<_>>();
        assert_eq!(selected_calls.len(), calls.len());
        for (call, selected_call) in calls.into_iter().zip(selected_calls) {
            assert_eq!(call.kind, selected_call.kind);
            assert_eq!(call.barrier, MachineBarrier::Call);
            assert_eq!(
                call.call,
                MachineCallEffect::DirectInternalNormalReturnV1 {
                    pre_call_stack_alignment: 16,
                }
            );
            assert_eq!(call.memory, MachineMemoryEffect::NoneV1);
            assert_eq!(call.trap, MachineTrapBehavior::NeverV1);
            assert_eq!(call.alternatives.len(), 1);
            let encoded = &call.alternatives[0].encoded;
            assert_eq!(encoded.external_operand_reads, [0, 1]);
            assert_eq!(encoded.external_operand_writes, [2]);
            assert_eq!(encoded.implicit_unit_uses, call.unit_uses);
            assert_eq!(encoded.implicit_unit_defs, call.unit_defs);
            assert_eq!(encoded.implicit_unit_clobbers, call.unit_clobbers);
            assert_eq!(
                encoded.control,
                MachineEncodedControlEffect::DirectRelativeCallV1
            );
            assert_eq!(
                encoded.trap,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            );
            match target.architecture {
                Architecture::X86_64 => {
                    assert_eq!(
                        call.alternatives[0].size,
                        MachineSizeKnowledge::ExactBytes(5)
                    );
                    assert!(matches!(
                        encoded.memory,
                        MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                            byte_count: 8,
                            ..
                        }
                    ));
                    assert!(matches!(
                        encoded.stack,
                        MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                            return_address_byte_count: 8,
                            ..
                        }
                    ));
                }
                Architecture::Aarch64 => {
                    assert_eq!(
                        call.alternatives[0].size,
                        MachineSizeKnowledge::ExactBytes(4)
                    );
                    assert_eq!(encoded.memory, MachineEncodedMemoryEffect::NoneV1);
                    assert_eq!(encoded.stack, MachineEncodedStackEffect::UnchangedV1);
                }
            }
        }

        let encoded_effects = effects.plan().encode();
        assert_eq!(
            omega_machine_optimizer::PreAllocationMachineEffectPlan::decode(&encoded_effects)
                .unwrap(),
            effects.plan().clone()
        );
        let mut legacy_effects = encoded_effects;
        legacy_effects[8..12].copy_from_slice(&8_u32.to_le_bytes());
        assert!(
            omega_machine_optimizer::PreAllocationMachineEffectPlan::decode(&legacy_effects)
                .is_err(),
            "V8 must not acquire the V9 scalar-call vocabulary"
        );

        let homes = staged_homes(target);
        let post = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
        let encoded_post = post.machine().plan().encode();
        assert_eq!(
            omega_machine_optimizer::PostAllocationMachinePlan::decode(&encoded_post).unwrap(),
            post.machine().plan().clone()
        );
        let mut legacy_post = encoded_post;
        legacy_post[8..12].copy_from_slice(&4_u32.to_le_bytes());
        assert!(
            omega_machine_optimizer::PostAllocationMachinePlan::decode(&legacy_post).is_err(),
            "V4 must not acquire the V5 scalar-call vocabulary"
        );
        let selected_stage = homes
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let encoding = stage_optimized_layout_independent_selected_form_encoding(
            selected_stage.selected(),
            &post,
            selected_stage.register_environment().physical(),
        )
        .expect("target-owned call templates must now reach selected-form encoding");
        assert_eq!(encoding.counts().ordinary_encoded_call_templates, 3);
        assert_eq!(encoding.counts().ordinary_deferred_internal_control, 3);
        assert_eq!(encoding.counts().ordinary_internal_fixups, 3);
        assert_eq!(encoding.counts().ordinary_encoded, 17);
        assert_eq!(encoding.counts().ordinary_deferred_control, 1);
    }
}

#[test]
fn scalar_call_effect_corruption_fails_independent_replay() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selected = staged_selected(target);
        let effects =
            analyze_machine_effects(selected.selected(), selected.register_environment()).unwrap();
        let environment = selected.register_environment();
        let catalog = match target.architecture {
            Architecture::X86_64 => {
                let catalog = omega_isa_x86_64::x86_64_machine_effect_catalog(
                    target,
                    environment.constraints(),
                )
                .unwrap();
                omega_isa_x86_64::validate_x86_64_machine_effect_catalog(
                    target,
                    environment.constraints(),
                    catalog,
                )
                .unwrap()
            }
            Architecture::Aarch64 => {
                let catalog = omega_isa_aarch64::aarch64_machine_effect_catalog(
                    target,
                    environment.constraints(),
                )
                .unwrap();
                omega_isa_aarch64::validate_aarch64_machine_effect_catalog(
                    target,
                    environment.constraints(),
                    catalog,
                )
                .unwrap()
            }
        };
        let replay = |plan| {
            omega_machine_optimizer::validate_pre_allocation_machine_effects(
                selected.selected(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                &catalog,
                plan,
            )
        };
        let mut corrupted = effects.plan().clone();
        first_call(&mut corrupted).unit_clobbers.pop();
        assert!(replay(corrupted).is_err());

        let mut corrupted = effects.plan().clone();
        first_call(&mut corrupted).call = MachineCallEffect::DirectInternalNormalReturnV1 {
            pre_call_stack_alignment: 8,
        };
        assert!(replay(corrupted).is_err());

        let mut corrupted = effects.plan().clone();
        first_call(&mut corrupted).alternatives[0].encoded.control =
            MachineEncodedControlEffect::FallThroughV1;
        assert!(replay(corrupted).is_err());

        let mut corrupted = effects.plan().clone();
        first_call(&mut corrupted).alternatives[0].encoded.trap =
            MachineEncodedTrapBehavior::NeverV1;
        assert!(replay(corrupted).is_err());

        let mut corrupted = effects.plan().clone();
        let call = first_call(&mut corrupted);
        if let MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
            return_address_byte_count,
            ..
        } = &mut call.alternatives[0].encoded.stack
        {
            *return_address_byte_count = 4;
        } else {
            call.alternatives[0].encoded.stack =
                MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                    stack_pointer: omega_register_model::RegisterViewId(0),
                    return_address_byte_count: 4,
                };
        }
        assert!(replay(corrupted).is_err());
    }
}
