//! Exact internal Unit-call leaves in composed control.

use super::*;

#[test]
fn composes_two_internal_unit_call_leaves() {
    let checked = checked(
        r#"
        data Root {}
        machine Root::quiet() {}
        machine Root::enter(flag: bool) {
            transition flag {
                true -> yes()
                _ -> no()
            }
            state yes() { Root::quiet(); }
            state no() { Root::quiet(); }
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let quiet = machine_named(&checked, "quiet");
    assert!(
        plans.for_machine(quiet).is_some(),
        "the internal target retains its ordinary Unit plan"
    );
    let composed = plans
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("the conditional should retain both internal Unit leaves");
    let [entry, when_true, when_false] = composed.states.as_slice() else {
        panic!("internal-call composed control retains exactly three states")
    };
    assert!(entry.operations.is_empty());
    for leaf in [when_true, when_false] {
        assert!(matches!(
            leaf.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::CallUnit {
                target_machine,
                structural_arguments,
                claim_transfers,
                ..
            }] if *target_machine == quiet
                && structural_arguments.is_empty()
                && claim_transfers.is_empty()
        ));
    }
}
