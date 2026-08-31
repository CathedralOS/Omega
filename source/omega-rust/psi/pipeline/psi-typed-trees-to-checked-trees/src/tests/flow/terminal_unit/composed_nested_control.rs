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
