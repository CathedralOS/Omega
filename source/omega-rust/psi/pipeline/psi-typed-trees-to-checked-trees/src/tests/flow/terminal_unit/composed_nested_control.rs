//! Two conditional frontiers and three exact effect leaves.

use super::*;

#[test]
fn composes_nested_boolean_control_with_one_scalar_handoff() {
    let checked = checked(
        r#"
        boundary trait Host { machine exit(code: i32); }
        data Root {}
        machine Root::enter(first: bool, second: bool) {
            transition first {
                true -> dispatch(second)
                _ -> outer_no()
            }
            state dispatch(second: bool) {
                transition second {
                    true -> inner_yes()
                    _ -> inner_no()
                }
            }
            state inner_yes() { Host::exit(1); }
            state inner_no() { Host::exit(2); }
            state outer_no() { Host::exit(3); }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("nested conditional should retain all three effect leaves");
    let [entry, dispatch, inner_yes, inner_no, outer_no] = plan.states.as_slice() else {
        panic!("nested composed control retains exactly five states")
    };
    let psi_checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &entry.terminator
    else {
        panic!("entry is the outer conditional")
    };
    assert_eq!(when_true.target_state, dispatch.state);
    assert_eq!(when_false.target_state, outer_no.state);
    assert!(matches!(
        when_true.scalar_arguments.as_slice(),
        [argument]
            if argument.source_scalar_parameter_index == 1
                && argument.target_scalar_parameter_index == 0
    ));
    assert!(matches!(
        dispatch.terminator,
        psi_checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
    ));
    for leaf in [inner_yes, inner_no, outer_no] {
        assert!(matches!(
            leaf.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::BoundaryCall { .. }]
        ));
    }
}

#[test]
fn composes_three_frontiers_with_recursive_scalar_suffix_handoffs() {
    let checked = checked(
        r#"
        boundary trait Host { machine exit(code: i32); }
        data Root {}
        machine Root::enter(first: bool, second: bool, third: bool) {
            transition first { true -> middle(second, third) _ -> outer_no() }
            state middle(second: bool, third: bool) {
                transition second { true -> inner(third) _ -> middle_no() }
            }
            state inner(third: bool) {
                transition third { true -> yes() _ -> no() }
            }
            state yes() { Host::exit(1); }
            state no() { Host::exit(2); }
            state middle_no() { Host::exit(3); }
            state outer_no() { Host::exit(4); }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("three conditional frontiers should compose recursively");
    assert_eq!(plan.states.len(), 7);
    for (index, expected_arguments) in [2, 1, 0].into_iter().enumerate() {
        let control = &plan.states[index];
        assert_eq!(control.scalar_parameters.len(), 3 - index);
        let psi_checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
            when_true,
            ..
        } = &control.terminator
        else {
            panic!("every control state remains conditional")
        };
        assert_eq!(when_true.scalar_arguments.len(), expected_arguments);
        for (argument_index, argument) in when_true.scalar_arguments.iter().enumerate() {
            assert_eq!(
                argument.source_scalar_parameter_index,
                (argument_index + 1) as u32
            );
            assert_eq!(
                argument.target_scalar_parameter_index,
                argument_index as u32
            );
        }
    }
    for leaf in &plan.states[3..] {
        assert!(matches!(
            leaf.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::BoundaryCall { .. }]
        ));
    }
}
