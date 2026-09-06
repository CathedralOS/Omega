//! A Unit closure consumes the scalar boundary wrapper's real checked body.

use super::*;

const SOURCE: &str = r#"
    boundary trait Host {
        machine measure(value: i32) -> i32;
        machine send(value: i32);
    }
    data Scalar {}
    machine Scalar::measure() -> i32
    reaches Host
    { let result: i32 = Host::measure(70); result }
    data Root {}
    machine Root::run()
    reaches Host
    { let value: i32 = Scalar::measure(); Host::send(value); }
"#;

#[test]
fn unit_scalar_call_retains_registered_boundary_return_target() {
    let mut checked = checked(SOURCE);
    let root = machine_named(&checked, "run");
    let target = machine_named(&checked, "Scalar::measure");
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(target)
            .is_none()
    );
    let wrapper = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .machines
        .iter()
        .find(|plan| plan.machine == target)
        .unwrap()
        .clone();
    assert!(wrapper.structural_parameters.is_empty());
    assert!(wrapper.entry_claims.is_empty());
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(root)
        .expect("Unit caller retains scalar wrapper");
    let CheckedUnitEffectOperationPlan::ScalarCall {
        target_machine,
        target_state,
        result,
        target_contract_commitment,
        ..
    } = &plan.operations[0]
    else {
        panic!("wrapper stays an ordinary scalar call");
    };
    assert_eq!(
        (*target_machine, *target_state, result.primitive_type),
        (target, wrapper.state, wrapper.result_type)
    );
    assert_eq!(
        *target_contract_commitment,
        checked
            .facts
            .contract_plans
            .for_machine(target)
            .unwrap()
            .commitment
    );
    crate::rebuild_checked_terminal_plans_with_selected_execution(&mut checked, &[], &[]).unwrap();
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(root)
            .is_some()
    );
    crate::rebuild_checked_unit_effect_plans_with_selected_execution(&mut checked, &[], &[]);
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(root)
            .is_some()
    );
}

#[test]
fn unit_scalar_call_rejects_missing_or_drifted_boundary_return_registration() {
    let original = checked(SOURCE);
    let root = machine_named(&original, "run");
    let target = machine_named(&original, "Scalar::measure");
    for mutation in 0..3 {
        let mut changed = original.clone();
        let plans = &mut changed.facts.flow.terminal_boundary_scalar_returns.machines;
        if mutation == 0 {
            plans.retain(|plan| plan.machine != target);
        } else {
            let plan = plans
                .iter_mut()
                .find(|plan| plan.machine == target)
                .unwrap();
            if mutation == 1 {
                plan.result_type = PrimitiveType::Bool;
            } else {
                plan.state = arena::Handle::invalid();
            }
        }
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&changed.typed, &changed.facts, &[], &[]);
        assert!(
            rebuilt.for_machine(root).is_none(),
            "wrapper registration mutation {mutation}"
        );
    }
}

#[test]
fn scalar_boundary_wrapper_does_not_discard_structural_call_custody() {
    let source = SOURCE
        .replace(
            "boundary trait Host",
            "data Metrics { current: i32; }\n boundary trait Host",
        )
        .replace(
            "machine measure(value: i32)",
            "machine measure(metrics: Metrics, value: i32)",
        )
        .replace(
            "Scalar::measure() ->",
            "Scalar::measure(metrics: Metrics) ->",
        )
        .replace("Host::measure(70)", "Host::measure(metrics, 70)")
        .replace("Root::run()", "Root::run(metrics: Metrics)")
        .replace("Scalar::measure();", "Scalar::measure(metrics);");
    let checked = checked(&source);
    let root = machine_named(&checked, "run");
    let target = machine_named(&checked, "Scalar::measure");
    let wrapper = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .machines
        .iter()
        .find(|plan| plan.machine == target)
        .expect("structural wrapper keeps its existing body plan");
    assert_eq!(wrapper.structural_parameters.len(), 1);
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(root)
            .is_none(),
        "ordinary ScalarCall cannot silently erase an owned structural argument"
    );
}
