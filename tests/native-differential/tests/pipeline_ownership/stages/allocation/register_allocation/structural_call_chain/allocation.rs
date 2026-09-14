use std::collections::{BTreeMap, BTreeSet};

use crate::tests::*;

use super::fixture::{caller_machine, staged_homes, staged_legality};

fn caller_function(
    selected: &StagedOptimizedSelectedInstructions,
) -> &selected_instructions::SelectedFunction {
    selected
        .selected()
        .plan()
        .functions
        .iter()
        .find(|function| function.machine == caller_machine())
        .unwrap()
}

fn caller_calls(
    selected: &StagedOptimizedSelectedInstructions,
) -> Vec<&selected_instructions::SelectedInstruction> {
    caller_function(selected)
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| {
            matches!(instruction.kind, SelectedInstructionKind::CallScalar { .. })
        })
        .collect()
}

#[test]
fn structural_call_clobbers_remove_every_aliasing_home_at_the_live_across_call_point() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let staged = staged_legality(target);
        let selected = staged.live_range_stage().liveness_stage().selected_stage();
        let calls = caller_calls(selected);
        assert_eq!(calls.len(), 3);
        // Each call carries the borrowed structural argument's pointer as one
        // fixed ABI operand beside the two scalar arguments and the result.
        for call in &calls {
            assert!(!call.clobbers.is_empty());
            assert_eq!(call.operands.len(), 4);
        }
        let call_clobbers = calls
            .iter()
            .map(|call| (call.id, &call.clobbers))
            .collect::<BTreeMap<_, _>>();

        let ranges = staged
            .live_range_stage()
            .ranges()
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        // Every clobbered architectural unit records its own action at the
        // same point; the recorded set is always the call's clobber list.
        let mut point_clobbers: BTreeMap<LiveRangePoint, &Vec<RegisterUnitId>> = BTreeMap::new();
        for row in &ranges.architectural_units {
            for action in &row.actions {
                if action.kind != ArchitecturalUnitActionKind::Clobber {
                    continue;
                }
                let Some(clobbers) = call_clobbers.get(&action.instruction) else {
                    continue;
                };
                if let Some(existing) = point_clobbers.get(&action.point) {
                    assert_eq!(*existing, *clobbers);
                } else {
                    point_clobbers.insert(action.point, *clobbers);
                }
            }
        }
        assert!(!point_clobbers.is_empty());

        // The first call's result leaves the call in the ABI result view and is
        // copied into the virtual register that stays live across the second
        // call to reach the third call's argument list.
        let result_def = calls[0].operands.last().unwrap().virtual_register;
        let live_result = caller_function(selected)
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| {
                let mut operands = instruction.operands.iter();
                let source = operands.next()?;
                let destination = operands.next()?;
                (instruction.operands.len() == 2
                    && source.virtual_register == result_def
                    && matches!(source.access, RegisterOperandAccess::Use)
                    && matches!(destination.access, RegisterOperandAccess::Def))
                .then_some(destination.virtual_register)
            })
            .expect("the first call result must be copied into a live register");
        let legality = staged
            .legality()
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        let live_across_call = legality
            .virtual_registers
            .iter()
            .filter(|register| {
                register
                    .points
                    .iter()
                    .any(|point| point_clobbers.contains_key(&point.point))
            })
            .map(|register| register.virtual_register)
            .collect::<BTreeSet<_>>();
        assert!(live_across_call.contains(&live_result));

        let model = selected.register_environment().physical().model();
        for register in &legality.virtual_registers {
            for point in register
                .points
                .iter()
                .filter(|point| point_clobbers.contains_key(&point.point))
            {
                assert!(!point.candidates.is_empty());
                let clobbers = point_clobbers[&point.point];
                for candidate in &point.candidates {
                    let view = &model.views[usize::from(candidate.0)];
                    assert!(
                        view.units
                            .iter()
                            .chain(&view.write_units)
                            .all(|unit| clobbers.binary_search(unit).is_err())
                    );
                }
            }
        }
    }
}

#[test]
fn structural_call_homes_preserve_the_live_result_and_every_fixed_call_operand() {
    for (target, convention_name) in [
        (NativeTarget::linux_x64(), "system-v-amd64"),
        (NativeTarget::windows_x64(), "microsoft-x64"),
        (NativeTarget::uefi_x64(), "microsoft-x64"),
        (NativeTarget::linux_arm64(), "aapcs64"),
        (NativeTarget::macos_arm64(), "darwin-aapcs64"),
    ] {
        let staged = staged_homes(target);
        let selected = staged
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let calls = caller_calls(selected);
        assert_eq!(calls.len(), 3);
        let homes = staged
            .homes()
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        let model = selected.register_environment().physical().model();
        let convention = model
            .conventions
            .iter()
            .find(|convention| convention.name == convention_name)
            .unwrap();
        let result_def = calls[0].operands.last().unwrap().virtual_register;
        let live_result = caller_function(selected)
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| {
                let mut operands = instruction.operands.iter();
                let source = operands.next()?;
                let destination = operands.next()?;
                (instruction.operands.len() == 2
                    && source.virtual_register == result_def
                    && matches!(source.access, RegisterOperandAccess::Use)
                    && matches!(destination.access, RegisterOperandAccess::Def))
                .then_some(destination.virtual_register)
            })
            .expect("the first call result must be copied into a live register");
        let result_home = homes
            .assignments
            .iter()
            .find(|assignment| assignment.virtual_register == live_result)
            .unwrap();
        let result_view = &model.views[usize::from(result_home.view.0)];
        assert!(
            result_view
                .units
                .iter()
                .chain(&result_view.write_units)
                .all(|unit| convention.callee_saved.binary_search(unit).is_ok())
        );

        for call in &calls {
            for operand in &call.operands {
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
    }
}
