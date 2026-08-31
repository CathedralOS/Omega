//! Independent replay of two conditional frontiers and three effect leaves.

use super::*;

fn checked_nested_control() -> CheckedTrees {
    checked_source(
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
    )
}

#[test]
fn lowers_two_conditional_frontiers_with_one_scalar_handoff() {
    let checked = checked_nested_control();
    let lowered = lower_machine(&checked, "Root::enter").expect("nested composed control lowers");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("nested boundary graph emits one machine")
    };
    let [first, second] = machine.parameters.as_slice() else {
        panic!("outer conditional retains both Boolean inputs")
    };
    let [entry, dispatch, inner_yes, inner_no, outer_no] = machine.blocks.as_slice() else {
        panic!("nested control emits exactly five blocks")
    };
    let [dispatch_parameter] = dispatch.parameters.as_slice() else {
        panic!("inner conditional receives one Boolean argument")
    };
    let Terminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &entry.terminator
    else {
        panic!("entry lowers as a conditional")
    };
    assert_eq!(*condition, first.id);
    assert_eq!(when_true.target, dispatch.id);
    assert_eq!(when_true.arguments.as_slice(), [second.id]);
    assert_eq!(when_false.target, outer_no.id);
    assert!(when_false.arguments.is_empty());
    assert!(matches!(
        dispatch.terminator,
        Terminator::Conditional { condition, .. } if condition == dispatch_parameter.id
    ));
    for leaf in [inner_yes, inner_no, outer_no] {
        assert!(matches!(
            leaf.operations.last(),
            Some(Operation {
                kind: OperationKind::BoundaryCall { .. },
                ..
            })
        ));
    }
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("nested composed module verifies");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn nested_control_rejects_outer_handoff_and_inner_topology_corruption() {
    let baseline = checked_nested_control();
    let rejects = |checked: &CheckedTrees| {
        assert!(matches!(
            lower_machine(checked, "Root::enter"),
            Err(LoweringError::Unsupported(_))
        ));
    };

    let mut handoff = baseline.clone();
    let CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
        &mut handoff.facts.flow.terminal_unit_effects.composed_machines[0].states[0].terminator
    else {
        unreachable!()
    };
    when_true.scalar_arguments[0].source_scalar_parameter_index = 0;
    rejects(&handoff);

    let mut topology = baseline;
    let wrong = topology.facts.flow.terminal_unit_effects.composed_machines[0].states[4].state;
    let CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
        &mut topology.facts.flow.terminal_unit_effects.composed_machines[0].states[1].terminator
    else {
        unreachable!()
    };
    when_true.target_state = wrong;
    rejects(&topology);
}

#[test]
fn nested_control_deduplicates_one_internal_target_after_five_root_blocks() {
    let checked = checked_source(
        r#"
            data Root {}
            machine Root::quiet() {}
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
                state inner_yes() { Root::quiet(); }
                state inner_no() { Root::quiet(); }
                state outer_no() { Root::quiet(); }
            }
        "#,
    );
    let lowered = lower_machine(&checked, "Root::enter").expect("nested internal closure lowers");
    let [root, target] = lowered.semantic_module.machines.as_slice() else {
        panic!("nested root and one deduplicated target form the closure")
    };
    assert_eq!(root.blocks.len(), 5);
    assert_eq!(target.blocks[0].id.get(), 6);
    for leaf in &root.blocks[2..] {
        assert!(matches!(
            leaf.operations.as_slice(),
            [Operation {
                kind: OperationKind::CallUnit { callee, .. },
                ..
            }] if *callee == target.id
        ));
    }
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("nested internal-call module verifies");
}
