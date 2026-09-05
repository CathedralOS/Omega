//! Acyclic composed Unit control with an unconditional scalar prefix edge.

use super::*;

#[test]
fn composes_scalar_prefix_before_boundary_call_conditional() {
    let checked = checked(
        r#"
        boundary trait Host { machine exit(code: i32); }
        data Root {}
        machine Root::enter(flag: bool) {
            transition { _ -> dispatch(flag) }
            state dispatch(flag: bool) {
                transition flag { true -> yes() _ -> no() }
            }
            state yes() { Host::exit(1); }
            state no() { Host::exit(2); }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("the scalar prefix should compose with both effect leaves");
    let [entry, dispatch, when_true, when_false] = plan.states.as_slice() else {
        panic!("prefixed composed control retains exactly four states")
    };
    assert!(matches!(
        &entry.terminator,
        checked_trees::CheckedComposedUnitControlTerminatorPlan::Jump { successor }
            if successor.target_state == dispatch.state
                && matches!(successor.scalar_arguments.as_slice(), [argument]
                    if argument.source_scalar_parameter_index == 0
                        && argument.target_scalar_parameter_index == 0)
    ));
    assert!(matches!(
        dispatch.terminator,
        checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
    ));
    for leaf in [when_true, when_false] {
        assert!(matches!(
            leaf.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::BoundaryCall { .. }]
        ));
    }
}

#[test]
fn composes_two_scalar_prefixes_without_a_depth_specific_route() {
    let checked = checked(
        r#"
        boundary trait Host { machine exit(code: i32); }
        data Root {}
        machine Root::enter(flag: bool) {
            transition { _ -> relay(flag) }
            state relay(flag: bool) { transition { _ -> dispatch(flag) } }
            state dispatch(flag: bool) {
                transition flag { true -> yes() _ -> no() }
            }
            state yes() { Host::exit(1); }
            state no() { Host::exit(2); }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("the scalar prefix chain should compose with both effect leaves");
    let [entry, relay, dispatch, when_true, when_false] = plan.states.as_slice() else {
        panic!("two prefixes retain exactly five checked states")
    };
    for (state, target) in [(entry, relay), (relay, dispatch)] {
        assert!(matches!(
            &state.terminator,
            checked_trees::CheckedComposedUnitControlTerminatorPlan::Jump { successor }
                if successor.target_state == target.state
                    && matches!(successor.scalar_arguments.as_slice(), [argument]
                        if argument.source_scalar_parameter_index == 0
                            && argument.target_scalar_parameter_index == 0)
        ));
    }
    assert!(matches!(
        dispatch.terminator,
        checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
    ));
    for leaf in [when_true, when_false] {
        assert!(matches!(
            leaf.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::BoundaryCall { .. }]
        ));
    }
}
