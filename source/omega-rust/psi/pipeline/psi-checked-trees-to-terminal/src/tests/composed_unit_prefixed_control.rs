//! Four-state acyclic composed Unit control and scalar-edge replay.

use super::*;

fn checked_prefixed_control() -> CheckedTrees {
    checked_source(
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
    )
}

#[test]
fn lowers_scalar_prefix_before_conditional_effect_leaves() {
    let checked = checked_prefixed_control();
    let lowered = lower_machine(&checked, "Root::enter")
        .expect("the exact four-state effect graph should lower");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("prefixed boundary graph emits one machine")
    };
    let [entry_parameter] = machine.parameters.as_slice() else {
        panic!("prefixed graph retains its Boolean machine input")
    };
    let [entry, dispatch, when_true, when_false] = machine.blocks.as_slice() else {
        panic!("prefixed graph emits exactly four blocks")
    };
    let [dispatch_parameter] = dispatch.parameters.as_slice() else {
        panic!("dispatch receives one Boolean block argument")
    };
    assert!(matches!(
        &entry.terminator,
        Terminator::Jump {
            target,
            arguments,
            trivial_affine_discards,
            ..
        } if *target == dispatch.id
            && arguments.as_slice() == [entry_parameter.id]
            && trivial_affine_discards.is_empty()
    ));
    assert!(matches!(
        dispatch.terminator,
        Terminator::Conditional { condition, .. }
            if condition == dispatch_parameter.id
    ));
    for leaf in [when_true, when_false] {
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
    .expect("prefixed composed module verifies");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn prefixed_control_rejects_scalar_edge_and_topology_corruption() {
    let baseline = checked_prefixed_control();
    let rejects = |checked: &CheckedTrees| {
        assert!(matches!(
            lower_machine(checked, "Root::enter"),
            Err(LoweringError::Unsupported(_))
        ));
    };

    let mut scalar = baseline.clone();
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
        &mut scalar.facts.flow.terminal_unit_effects.composed_machines[0].states[0].terminator
    else {
        unreachable!()
    };
    successor.scalar_arguments[0].target_scalar_parameter_index = 1;
    rejects(&scalar);

    let mut target = baseline.clone();
    let wrong = target.facts.flow.terminal_unit_effects.composed_machines[0].states[2].state;
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
        &mut target.facts.flow.terminal_unit_effects.composed_machines[0].states[0].terminator
    else {
        unreachable!()
    };
    successor.target_state = wrong;
    rejects(&target);
}
