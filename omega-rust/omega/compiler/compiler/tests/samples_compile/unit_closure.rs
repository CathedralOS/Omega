//! The real entry's checked closure is separate from native provider admission.

use super::*;
use checked_trees::CheckedUnitEffectOperationPlan;

#[test]
fn cli_mvp_retains_checked_entry_and_console_call_closure() {
    let root = repo_root().join("samples/cli/basics/cli_mvp/main.omg");
    for target in [
        "macos_arm64",
        "linux_arm64",
        "linux_x86_64",
        "windows_x86_64",
    ] {
        let checked = compile_sample_to_checked(&root, Some(target)).unwrap();
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .unwrap();
        let plans = &checked.facts.flow.terminal_unit_effects;
        let plan = plans
            .for_machine(machine.symbol)
            .expect("complete authored entry plan");
        assert_eq!(plan.structural_parameters.len(), 1);
        assert!(plan.structural_parameters[0].is_self);
        assert_eq!(plan.operations.len(), 6);
        let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            result,
            discard_result_on_return,
            target_machine,
            ..
        } = &plan.operations[2]
        else {
            panic!("line read keeps its structural result");
        };
        assert_eq!(coordinate.statement_index, 2);
        assert!(!discard_result_on_return);
        assert!(plans.boundary_for_machine(*target_machine).is_some());
        assert!(
            matches!(&plan.operations[3], CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            coordinate: cleanup, affine_discards,
        } if cleanup == coordinate && affine_discards.len() == 1
            && affine_discards[0].type_identity == result.type_identity)
        );
        for name in [
            "ConsoleNativeProvider::write",
            "ConsoleNativeProvider::write_line",
            "console_write_bytes",
            "ConsoleNativeProvider::read_line",
        ] {
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == name)
                .unwrap();
            assert!(
                plans.for_machine(machine.symbol).is_some()
                    || plans.composed_for_machine(machine.symbol).is_some(),
                "{target}: missing {name}"
            );
        }
    }
}
