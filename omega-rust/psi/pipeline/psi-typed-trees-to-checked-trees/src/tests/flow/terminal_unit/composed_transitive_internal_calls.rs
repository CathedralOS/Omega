//! Exact transitive internal Unit-call closure from composed control.

use super::*;

#[test]
fn composes_one_call_internal_target_and_retains_its_empty_callee() {
    let checked = checked(
        r#"
        data Root {}
        machine Root::quiet() {}
        machine Root::relay() { Root::quiet(); }
        machine Root::enter(flag: bool) {
            transition flag {
                true -> yes()
                _ -> no()
            }
            state yes() { Root::relay(); }
            state no() { Root::relay(); }
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let quiet = machine_named(&checked, "quiet");
    let relay = machine_named(&checked, "relay");
    let relay_plan = plans
        .for_machine(relay)
        .expect("the one-call target retains its ordinary Unit plan");
    assert!(matches!(
        relay_plan.operations.as_slice(),
        [CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. }]
            if *target_machine == quiet
    ));
    assert!(
        plans.for_machine(quiet).is_some(),
        "the transitive empty target survives ordinary-plan pruning"
    );
    let composed = plans
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("the conditional should retain both relay leaves");
    for leaf in &composed.states[1..] {
        assert!(matches!(
            leaf.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. }]
                if *target_machine == relay
        ));
    }
}

#[test]
fn retains_a_depth_two_internal_target_chain() {
    let checked = checked(
        r#"
        data Root {}
        machine Root::quiet() {}
        machine Root::second() { Root::quiet(); }
        machine Root::first() { Root::second(); }
        machine Root::enter(flag: bool) {
            transition flag {
                true -> yes()
                _ -> no()
            }
            state yes() { Root::first(); }
            state no() { Root::first(); }
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let quiet = machine_named(&checked, "quiet");
    let second = machine_named(&checked, "second");
    let first = machine_named(&checked, "first");
    for (caller, callee) in [(first, second), (second, quiet)] {
        let plan = plans.for_machine(caller).expect("chain member plan");
        assert!(matches!(
            plan.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. },
                CheckedUnitEffectOperationPlan::ReturnUnit { .. }]
                if *target_machine == callee
        ));
    }
    let composed = plans
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("the conditional retains both first-link leaves");
    for leaf in &composed.states[1..] {
        assert!(matches!(
            leaf.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. }]
                if *target_machine == first
        ));
    }
}
