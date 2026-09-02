use std::collections::BTreeSet;

use crate::tests::*;

use super::fixture::{caller_machine, staged_homes, staged_legality, staged_liveness};

#[test]
fn first_result_is_live_across_the_second_calls_exact_clobbers() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_liveness(target);
        let selected_function = staged
            .selected_stage()
            .selected()
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        let call2 = &selected_function.blocks[0].instructions[8];
        assert!(matches!(
            call2.kind,
            SelectedInstructionKind::CallI64 { .. }
        ));
        let function = staged
            .liveness()
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        let call2_liveness = function.blocks[0]
            .instructions
            .iter()
            .find(|instruction| instruction.instruction == call2.id)
            .unwrap();
        assert!(
            call2_liveness
                .virtual_live_in
                .contains(&VirtualRegisterId(5))
        );
        assert!(
            call2_liveness
                .virtual_live_out
                .contains(&VirtualRegisterId(5))
        );
        assert_eq!(call2_liveness.unit_clobbers, call2.clobbers);
        assert!(!call2_liveness.unit_clobbers.is_empty());
        assert_eq!(call2_liveness.unit_uses, call2.implicit_uses);
        assert_eq!(call2_liveness.unit_defs, call2.implicit_defs);
    }
}

#[test]
fn call_clobbers_remove_every_aliasing_home_at_the_live_across_call_point() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_legality(target);
        let selected = staged.live_range_stage().liveness_stage().selected_stage();
        let selected_function = selected
            .selected()
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        let call2 = &selected_function.blocks[0].instructions[8];
        let ranges = staged
            .live_range_stage()
            .ranges()
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        let call_points = ranges
            .architectural_units
            .iter()
            .flat_map(|row| {
                row.actions.iter().filter_map(|action| {
                    (action.instruction == call2.id
                        && action.kind == ArchitecturalUnitActionKind::Clobber)
                        .then_some(action.point)
                })
            })
            .collect::<BTreeSet<_>>();
        assert!(!call_points.is_empty());

        let legality = staged
            .legality()
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        let first_result = legality
            .virtual_registers
            .iter()
            .find(|register| register.virtual_register == VirtualRegisterId(5))
            .unwrap();
        let call_legality = first_result
            .points
            .iter()
            .filter(|point| call_points.contains(&point.point))
            .collect::<Vec<_>>();
        assert!(!call_legality.is_empty());

        let model = selected.register_environment().physical().model();
        for point in call_legality {
            assert!(!point.candidates.is_empty());
            for candidate in &point.candidates {
                let view = &model.views[usize::from(candidate.0)];
                assert!(
                    view.units
                        .iter()
                        .chain(&view.write_units)
                        .all(|unit| { call2.clobbers.binary_search(unit).is_err() })
                );
            }
        }
    }
}

#[test]
fn homes_preserve_the_live_result_and_every_fixed_call_operand() {
    for (target, convention_name) in [
        (NativeTarget::linux_x64(), "system-v-amd64"),
        (NativeTarget::linux_arm64(), "aapcs64"),
    ] {
        let staged = staged_homes(target);
        let selected = staged
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let selected_function = selected
            .selected()
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        let homes = staged
            .homes()
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        assert_eq!(homes.assignments.len(), 14);
        let model = selected.register_environment().physical().model();
        let convention = model
            .conventions
            .iter()
            .find(|convention| convention.name == convention_name)
            .unwrap();
        let first_result_home = homes
            .assignments
            .iter()
            .find(|assignment| assignment.virtual_register == VirtualRegisterId(5))
            .unwrap();
        let first_result_view = &model.views[usize::from(first_result_home.view.0)];
        assert!(
            first_result_view
                .units
                .iter()
                .chain(&first_result_view.write_units)
                .all(|unit| convention.callee_saved.binary_search(unit).is_ok())
        );

        for instruction_index in [4, 8, 12] {
            let instruction = &selected_function.blocks[0].instructions[instruction_index];
            for operand in &instruction.operands {
                let fixed_view = operand.fixed_view.expect("call operand must be ABI-fixed");
                let assignment = homes
                    .assignments
                    .iter()
                    .find(|assignment| assignment.virtual_register == operand.virtual_register)
                    .unwrap();
                assert_eq!(assignment.view, fixed_view);
            }
        }

        let repeated = staged_homes(target);
        assert_eq!(staged.homes(), repeated.homes());
        assert_eq!(staged.custody(), repeated.custody());
        assert!(matches!(
            stage_optimized_machine_effects(selected),
            Err(OptimizedMachineEffectPipelineError::Analysis(
                omega_machine_optimizer::MachineEffectError::ScalarCallMachineEffectsUnsupported {
                    instruction: SelectedInstructionId(4)
                }
            ))
        ));
    }
}
