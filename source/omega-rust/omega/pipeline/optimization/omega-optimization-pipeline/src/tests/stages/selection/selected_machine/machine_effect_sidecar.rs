//! Machine-effect reconstruction for subtraction instructions and control barriers.

use crate::tests::*;

#[test]
fn machine_effect_sidecar_reconstructs_subtraction_and_control_barriers() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selected = staged_exact_subtract_conditional(target);
        let staged = stage_optimized_machine_effects(&selected).unwrap();
        assert_eq!(staged.custody().instruction_count(), 10);
        assert_eq!(
            staged.custody().source(),
            &StagedOptimizedMachineEffectSourceCustodyReceipt::Selected(selected.custody())
        );
        assert_eq!(
            &validate_optimized_machine_effect_custody(&selected, staged.effects()).unwrap(),
            staged.custody()
        );
        let instructions = staged
            .effects()
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

        let mut corrupted = staged.effects().plan().clone();
        corrupted.functions[0].blocks[1].instructions[2]
            .alternatives
            .clear();
        assert!(matches!(
            omega_machine_optimizer::validate_pre_allocation_machine_effects(
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
            Err(omega_machine_optimizer::MachineEffectError::InstructionMismatch { .. })
        ));
    }
}
