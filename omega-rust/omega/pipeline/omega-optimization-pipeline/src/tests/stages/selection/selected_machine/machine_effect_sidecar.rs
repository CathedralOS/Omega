//! Machine-effect reconstruction for subtraction instructions and control barriers.

use crate::tests::*;

#[test]
fn machine_effect_replay_rejects_a_different_program_or_environment() {
    let selected = staged_exact_subtract_conditional(NativeTarget::linux_x64());
    let other = staged_forwarded_conditional(NativeTarget::linux_x64());
    let arm = staged_exact_subtract_conditional(NativeTarget::linux_arm64());
    let effects =
        analyze_machine_effects(selected.selected(), selected.register_environment()).unwrap();
    assert!(
        validate_machine_effects(other.selected(), other.register_environment(), &effects).is_err()
    );
    assert!(
        validate_machine_effects(selected.selected(), arm.register_environment(), &effects)
            .is_err()
    );
    assert_eq!(
        effects,
        analyze_machine_effects(selected.selected(), selected.register_environment()).unwrap()
    );
}

#[test]
fn machine_effect_sidecar_reconstructs_subtraction_and_control_barriers() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selected = staged_exact_subtract_conditional(target);
        let staged =
            analyze_machine_effects(selected.selected(), selected.register_environment()).unwrap();
        assert_eq!(staged.receipt().instruction_count(), 10);
        assert_eq!(
            staged.receipt().selected(),
            selected.selected().receipt().identity()
        );
        validate_machine_effects(
            selected.selected(),
            selected.register_environment(),
            &staged,
        )
        .unwrap();
        let instructions = staged
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .collect::<Vec<_>>();
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| {
                    instruction.barrier == omega_selected_instructions::MachineBarrier::ControlFlow
                })
                .count(),
            3
        );
        let subtracts = instructions
            .iter()
            .filter(|instruction| {
                matches!(
                    instruction.kind,
                    SelectedInstructionKind::ExactSubtractI64 { .. }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(subtracts.len(), 2);
        for subtract in subtracts {
            assert_eq!(
                subtract.alternatives.len(),
                if target.architecture == omega_target::Architecture::X86_64 {
                    4
                } else {
                    1
                }
            );
            assert_eq!(
                subtract.unit_clobbers.is_empty(),
                target.architecture != omega_target::Architecture::X86_64
            );
            assert_eq!(subtract.provenance.obligations.len(), 1);
            assert_eq!(subtract.provenance.fuel.len(), 1);
        }

        let mut corrupted = staged.plan().clone();
        corrupted.functions[0].blocks[1].instructions[2]
            .alternatives
            .clear();
        assert!(matches!(
            omega_selected_instructions_to_machine_effects::validate_pre_allocation_machine_effects(
                selected.selected(),
                selected.register_environment().identity(),
                selected.register_environment().physical(),
                selected.register_environment().constraints(),
                selected.register_environment().reservations(),
                selected.register_environment().allocation_constraint_keys(),
                &match target.architecture {
                    omega_target::Architecture::X86_64 => {
                        omega_isa_x86_64::validate_x86_64_machine_effect_catalog(
                            target,
                            selected.register_environment().constraints(),
                            omega_isa_x86_64::x86_64_machine_effect_catalog(
                                target,
                                selected.register_environment().constraints(),
                            )
                            .unwrap(),
                        )
                        .unwrap()
                    }
                    omega_target::Architecture::Aarch64 => {
                        omega_isa_aarch64::validate_aarch64_machine_effect_catalog(
                            target,
                            selected.register_environment().constraints(),
                            omega_isa_aarch64::aarch64_machine_effect_catalog(
                                target,
                                selected.register_environment().constraints(),
                            )
                            .unwrap(),
                        )
                        .unwrap()
                    }
                },
                corrupted,
            ),
            Err(omega_selected_instructions_to_machine_effects::MachineEffectError::InstructionMismatch { .. })
        ));
    }
}
