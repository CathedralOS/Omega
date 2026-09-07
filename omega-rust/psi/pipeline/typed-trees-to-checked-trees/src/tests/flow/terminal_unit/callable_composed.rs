//! Ordinary and composed Unit bodies share one complete call-target closure.

use super::*;

const CHAIN: &str = r#"
    data Root {}
    machine Root::enter(flag: bool) { Helper::outer(flag); }
    data Helper {}
    machine Helper::outer(flag: bool) {
        transition flag { true -> yes() false -> no() }
        state yes() { Helper::middle(true); }
        state no() { Helper::middle(false); }
    }
    machine Helper::middle(flag: bool) {
        transition flag { true -> yes() false -> no() }
        state yes() { Helper::relay(); }
        state no() { Helper::relay(); }
    }
    machine Helper::relay() { Helper::inner(true); }
    machine Helper::inner(flag: bool) {
        transition flag { true -> yes() false -> no() }
        state yes() { Helper::quiet(); }
        state no() { Helper::quiet(); }
    }
    machine Helper::quiet() {}
    machine Helper::unrelated() {}
"#;

fn assert_unique_catalogs(plans: &checked_trees::CheckedUnitEffectPlans) {
    let identities = plans
        .machines
        .iter()
        .map(|plan| plan.machine)
        .chain(plans.composed_machines.iter().map(|plan| plan.machine))
        .collect::<Vec<_>>();
    for identity in &identities {
        assert_eq!(
            identities
                .iter()
                .filter(|candidate| *candidate == identity)
                .count(),
            1,
            "one ordinary or composed body owns each callable identity"
        );
    }
}

#[test]
fn ordinary_caller_retains_supported_three_state_unit_callee() {
    let checked = checked(
        r#"
        data Helper {}
        machine Helper::choose(flag: bool) {
            transition flag { true -> yes() false -> no() }
            state yes() { Helper::quiet(); }
            state no() { Helper::quiet(); }
        }
        machine Helper::quiet() {}
        data Root {}
        machine Root::enter(flag: bool) { Helper::choose(flag); }
    "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let caller = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("ordinary caller retains its composed target");
    let callee = plans
        .composed_for_machine(machine_named(&checked, "choose"))
        .expect("callee keeps its original three-state body");
    assert_eq!(callee.states.len(), 3);
    assert!(matches!(&caller.operations[0],
        CheckedUnitEffectOperationPlan::CallUnit { target_machine, target_state, .. }
            if *target_machine == callee.machine && *target_state == callee.states[0].state));
    assert!(
        plans
            .for_machine(machine_named(&checked, "quiet"))
            .is_some()
    );
    assert_unique_catalogs(plans);
}

#[test]
fn callable_composed_targets_survive_direct_and_interleaved_transitive_calls() {
    let checked = checked(CHAIN);
    let plans = &checked.facts.flow.terminal_unit_effects;
    for name in ["enter", "relay", "quiet", "unrelated"] {
        assert!(
            plans.for_machine(machine_named(&checked, name)).is_some(),
            "{name}"
        );
    }
    for (caller, target) in [("outer", "middle"), ("middle", "relay"), ("inner", "quiet")] {
        let plan = plans
            .composed_for_machine(machine_named(&checked, caller))
            .expect("composed closure member");
        let target = machine_named(&checked, target);
        assert_eq!(plan.states.len(), 3);
        for leaf in &plan.states[1..] {
            assert!(matches!(leaf.operations.as_slice(),
                [CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. }]
                    if *target_machine == target));
        }
    }
    assert_unique_catalogs(plans);
}

#[test]
fn missing_transitive_body_prunes_both_catalogs_to_a_joint_fixed_point() {
    // A valid two-state source has neither an ordinary one-state body nor an
    // admitted composed body. Merely knowing its source symbol cannot admit it.
    let checked = checked(&CHAIN.replace(
        "machine Helper::quiet() {}",
        r#"
        machine Helper::quiet() {
            transition { _ -> done() }
            state done() {}
        }
    "#,
    ));
    let plans = &checked.facts.flow.terminal_unit_effects;
    for name in ["enter", "outer", "middle", "relay", "inner", "quiet"] {
        let symbol = machine_named(&checked, name);
        assert!(plans.for_machine(symbol).is_none(), "ordinary {name}");
        assert!(
            plans.composed_for_machine(symbol).is_none(),
            "composed {name}"
        );
    }
    assert!(
        plans
            .for_machine(machine_named(&checked, "unrelated"))
            .is_some()
    );
    assert_unique_catalogs(plans);
}

#[test]
fn unsupported_composed_leaf_prunes_upstream_without_relaxing_body_admission() {
    let checked = checked(&CHAIN.replace(
        "state yes() { Helper::quiet(); }",
        "state yes() { Helper::quiet(); Helper::quiet(); }",
    ));
    let plans = &checked.facts.flow.terminal_unit_effects;
    for name in ["enter", "outer", "middle", "relay", "inner"] {
        let symbol = machine_named(&checked, name);
        assert!(plans.for_machine(symbol).is_none(), "ordinary {name}");
        assert!(
            plans.composed_for_machine(symbol).is_none(),
            "composed {name}"
        );
    }
    for name in ["quiet", "unrelated"] {
        assert!(
            plans.for_machine(machine_named(&checked, name)).is_some(),
            "{name}"
        );
    }
    assert_unique_catalogs(plans);
}
