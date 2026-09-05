//! One-call acyclic target closure beneath composed Unit leaves.

use super::*;

fn checked_transitive_internal_calls() -> checked_trees::CheckedTrees {
    checked_source(
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
    )
}

fn checked_depth_two_internal_calls() -> checked_trees::CheckedTrees {
    checked_source(
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
    )
}

fn source_machine(checked: &CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|selection| selection.name == name)
        .map(|selection| selection.machine)
        .expect("named checked terminal machine")
}

#[test]
fn lowers_and_deduplicates_one_call_internal_target_closure() {
    let checked = checked_transitive_internal_calls();
    let lowered = lower_machine(&checked, "Root::enter").expect("transitive internal closure");
    assert_eq!(lowered.semantic_module.machines.len(), 3);
    let root = &lowered.semantic_module.machines[0];
    let OperationKind::CallUnit {
        callee: relay_id, ..
    } = root.blocks[1].operations[0].kind
    else {
        panic!("composed leaf should call the relay")
    };
    let relay = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == relay_id)
        .expect("relay target is present");
    for leaf in &root.blocks[1..] {
        assert!(matches!(
            leaf.operations.as_slice(),
            [Operation {
                kind: OperationKind::CallUnit { callee, .. },
                ..
            }] if *callee == relay_id
        ));
    }
    let [
        Operation {
            kind: OperationKind::CallUnit {
                callee: quiet_id, ..
            },
            ..
        },
    ] = relay.blocks[0].operations.as_slice()
    else {
        panic!("relay target should contain one call")
    };
    let quiet = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == *quiet_id)
        .expect("empty transitive target is present");
    assert!(quiet.blocks[0].operations.is_empty());
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("transitive internal-call module verifies");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn lowers_a_depth_two_internal_target_chain_once() {
    let checked = checked_depth_two_internal_calls();
    let lowered = lower_machine(&checked, "Root::enter").expect("depth-two internal closure");
    assert_eq!(lowered.semantic_module.machines.len(), 4);
    let root = &lowered.semantic_module.machines[0];
    let mut next = match root.blocks[1].operations[0].kind {
        OperationKind::CallUnit { callee, .. } => callee,
        _ => panic!("composed leaf should call the first link"),
    };
    for _ in 0..2 {
        let machine = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == next)
            .expect("chain target is present exactly once");
        let [
            Operation {
                kind: OperationKind::CallUnit { callee, .. },
                ..
            },
        ] = machine.blocks[0].operations.as_slice()
        else {
            panic!("nonfinal chain target should contain one call")
        };
        next = *callee;
    }
    let quiet = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == next)
        .expect("final empty target is present exactly once");
    assert!(quiet.blocks[0].operations.is_empty());
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("depth-two internal-call module verifies");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn transitive_internal_target_rejects_nested_identity_and_plan_corruption() {
    let baseline = checked_transitive_internal_calls();
    let rejects = |checked: &CheckedTrees| {
        assert!(matches!(
            lower_machine(checked, "Root::enter"),
            Err(LoweringError::Unsupported(_))
        ));
    };

    let relay = source_machine(&baseline, "Root::relay");
    let quiet = source_machine(&baseline, "Root::quiet");
    let mut state = baseline.clone();
    let wrong_state = state.facts.flow.terminal_unit_effects.composed_machines[0].states[0].state;
    let relay_plan = state
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == relay)
        .expect("relay plan");
    let CheckedUnitEffectOperationPlan::CallUnit { target_state, .. } =
        &mut relay_plan.operations[0]
    else {
        unreachable!()
    };
    *target_state = wrong_state;
    rejects(&state);

    let mut missing = baseline;
    missing
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .retain(|plan| plan.machine != quiet);
    rejects(&missing);
}
