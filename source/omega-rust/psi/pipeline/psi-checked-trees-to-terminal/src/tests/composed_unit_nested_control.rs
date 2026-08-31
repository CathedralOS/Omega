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

fn checked_depth_three_nested_control() -> CheckedTrees {
    checked_source(
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
    )
}

fn checked_balanced_nested_control() -> CheckedTrees {
    checked_source(
        r#"
            boundary trait Host { machine exit(code: i32); }
            data Root {}
            machine Root::enter(first: bool, left_guard: bool, right_guard: bool) {
                transition first {
                    true -> left(left_guard)
                    _ -> right(right_guard)
                }
                state left(flag: bool) {
                    transition flag { true -> yes() _ -> shared() }
                }
                state right(flag: bool) {
                    transition flag { true -> shared() _ -> no() }
                }
                state yes() { Host::exit(1); }
                state shared() { Host::exit(2); }
                state no() { Host::exit(3); }
            }
        "#,
    )
}

fn checked_four_state_nested_control() -> CheckedTrees {
    checked_source(
        r#"
            boundary trait Host { machine exit(code: i32); }
            data Root {}
            machine Root::enter(first: bool, second: bool) {
                transition first { true -> dispatch(second) _ -> no() }
                state dispatch(flag: bool) {
                    transition flag { true -> yes() _ -> no() }
                }
                state yes() { Host::exit(1); }
                state no() { Host::exit(2); }
            }
        "#,
    )
}

fn checked_call_prefixed_nested_control() -> CheckedTrees {
    checked_source(
        r#"
            boundary trait Host { machine exit(code: i32); }
            data Root {}
            machine Root::quiet() {}
            machine Root::enter(first: bool, second: bool) {
                Root::quiet();
                transition first { true -> dispatch(second) _ -> no() }
                state dispatch(flag: bool) {
                    transition flag { true -> yes() _ -> no() }
                }
                state yes() { Host::exit(1); }
                state no() { Host::exit(2); }
            }
        "#,
    )
}

fn checked_multi_call_prefixed_nested_control() -> CheckedTrees {
    checked_source(
        r#"
            boundary trait Host { machine exit(code: i32); }
            data Root {}
            machine Root::quiet_a() {}
            machine Root::quiet_b() {}
            machine Root::enter(first: bool, second: bool) {
                Root::quiet_a();
                Root::quiet_b();
                transition first { true -> dispatch(second) _ -> no() }
                state dispatch(flag: bool) {
                    transition flag { true -> yes() _ -> no() }
                }
                state yes() { Host::exit(1); }
                state no() { Host::exit(2); }
            }
        "#,
    )
}

fn checked_boundary_prefixed_nested_control() -> CheckedTrees {
    checked_source(
        r#"
            boundary trait Host {
                machine tick();
                machine exit(code: i32);
            }
            data Root {}
            machine Root::enter(first: bool, second: bool) {
                Host::tick();
                transition first { true -> dispatch(second) _ -> no() }
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
fn lowers_the_smallest_two_frontier_convergent_graph() {
    let checked = checked_four_state_nested_control();
    let lowered = lower_machine(&checked, "Root::enter").expect("four-state graph lowers");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("four-state boundary graph emits one machine")
    };
    let [entry, dispatch, yes, no] = machine.blocks.as_slice() else {
        panic!("four-state graph emits two controls and two leaves")
    };
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &entry.terminator
    else {
        panic!("entry remains conditional")
    };
    assert_eq!(when_true.target, dispatch.id);
    assert_eq!(when_false.target, no.id);
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &dispatch.terminator
    else {
        panic!("dispatch remains conditional")
    };
    assert_eq!(when_true.target, yes.id);
    assert_eq!(when_false.target, no.id);
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("four-state convergent module verifies");
}

#[test]
fn lowers_an_internal_unit_call_before_a_conditional() {
    let checked = checked_call_prefixed_nested_control();
    let lowered = lower_machine(&checked, "Root::enter").expect("call-prefixed conditional lowers");
    let [root, quiet] = lowered.semantic_module.machines.as_slice() else {
        panic!("root and one deduplicated internal target form the module")
    };
    let [entry, dispatch, _, _] = root.blocks.as_slice() else {
        panic!("call-prefixed graph retains four root blocks")
    };
    assert!(matches!(
        entry.operations.as_slice(),
        [Operation {
            kind: OperationKind::CallUnit { callee, .. },
            ..
        }] if *callee == quiet.id
    ));
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &entry.terminator
    else {
        panic!("the call is followed by the source conditional")
    };
    assert_eq!(when_true.target, dispatch.id);
    assert!(when_false.arguments.is_empty());
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("call-prefixed conditional module verifies");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn call_prefixed_control_rejects_coordinate_drift() {
    let mut checked = checked_call_prefixed_nested_control();
    let CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. } =
        &mut checked.facts.flow.terminal_unit_effects.composed_machines[0].states[0].operations[0]
    else {
        unreachable!()
    };
    coordinate.statement_index = 1;
    assert!(matches!(
        lower_machine(&checked, "Root::enter"),
        Err(LoweringError::Unsupported(_))
    ));
}

#[test]
fn lowers_a_finite_internal_call_prefix_in_source_order() {
    let checked = checked_multi_call_prefixed_nested_control();
    let lowered = lower_machine(&checked, "Root::enter").expect("multi-call prefix lowers");
    let [root, first_target, second_target] = lowered.semantic_module.machines.as_slice() else {
        panic!("root and two internal targets form the module")
    };
    let entry = &root.blocks[0];
    assert!(matches!(
        entry.operations.as_slice(),
        [
            Operation { kind: OperationKind::CallUnit { callee: first, .. }, .. },
            Operation { kind: OperationKind::CallUnit { callee: second, .. }, .. },
        ] if *first == first_target.id && *second == second_target.id
    ));
    assert!(matches!(entry.terminator, Terminator::Conditional { .. }));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("multi-call prefix module verifies");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn finite_call_prefix_rejects_operation_reordering() {
    let mut checked = checked_multi_call_prefixed_nested_control();
    checked.facts.flow.terminal_unit_effects.composed_machines[0].states[0]
        .operations
        .swap(0, 1);
    assert!(matches!(
        lower_machine(&checked, "Root::enter"),
        Err(LoweringError::Unsupported(_))
    ));
}

#[test]
fn lowers_a_parameterless_boundary_call_before_a_conditional() {
    let checked = checked_boundary_prefixed_nested_control();
    let source_entry =
        checked.facts.flow.terminal_unit_effects.composed_machines[0].states[0].state;
    let lowered = lower_machine(&checked, "Root::enter").expect("boundary prefix lowers");
    let [root] = lowered.semantic_module.machines.as_slice() else {
        panic!("boundary-prefixed graph emits one root machine")
    };
    let entry = &root.blocks[0];
    assert!(matches!(
        entry.operations.as_slice(),
        [Operation {
            kind: OperationKind::BoundaryCall { arguments, .. },
            ..
        }] if arguments.is_empty()
    ));
    assert!(matches!(entry.terminator, Terminator::Conditional { .. }));
    assert!(lowered.source_call_occurrences.iter().any(|occurrence| {
        occurrence.source_state == source_entry
            && occurrence.statement_index == 0
            && occurrence.call_ordinal == 0
            && occurrence.terminal_operation == entry.operations[0].id
    }));
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("boundary-prefixed conditional module verifies");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn boundary_call_prefix_rejects_coordinate_drift() {
    let mut checked = checked_boundary_prefixed_nested_control();
    let CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. } =
        &mut checked.facts.flow.terminal_unit_effects.composed_machines[0].states[0].operations[0]
    else {
        unreachable!()
    };
    coordinate.statement_index = 1;
    assert!(matches!(
        lower_machine(&checked, "Root::enter"),
        Err(LoweringError::Unsupported(_))
    ));
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

#[test]
fn lowers_three_frontiers_with_recursive_scalar_suffix_handoffs() {
    let checked = checked_depth_three_nested_control();
    let lowered = lower_machine(&checked, "Root::enter").expect("depth-three nested control");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("depth-three boundary graph emits one machine")
    };
    assert_eq!(machine.parameters.len(), 3);
    assert_eq!(machine.blocks.len(), 7);
    assert_eq!(machine.blocks[1].parameters.len(), 2);
    assert_eq!(machine.blocks[2].parameters.len(), 1);
    for index in 0..3 {
        let Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } = &machine.blocks[index].terminator
        else {
            panic!("each control block is conditional")
        };
        let parameters = if index == 0 {
            &machine.parameters
        } else {
            &machine.blocks[index].parameters
        };
        assert_eq!(*condition, parameters[0].id);
        assert_eq!(
            when_true.arguments,
            parameters[1..]
                .iter()
                .map(|parameter| parameter.id)
                .collect::<Vec<_>>()
        );
        assert!(when_false.arguments.is_empty());
    }
    for leaf in &machine.blocks[3..] {
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
    .expect("depth-three nested module verifies");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn depth_three_nested_control_rejects_suffix_reordering() {
    let mut checked = checked_depth_three_nested_control();
    let CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
        &mut checked.facts.flow.terminal_unit_effects.composed_machines[0].states[0].terminator
    else {
        unreachable!()
    };
    when_true.scalar_arguments.swap(0, 1);
    assert!(matches!(
        lower_machine(&checked, "Root::enter"),
        Err(LoweringError::Unsupported(_))
    ));
}

#[test]
fn lowers_balanced_control_and_emits_one_convergent_leaf() {
    let checked = checked_balanced_nested_control();
    let lowered = lower_machine(&checked, "Root::enter").expect("balanced control graph lowers");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("balanced boundary graph emits one machine")
    };
    let [first, left_guard, right_guard] = machine.parameters.as_slice() else {
        panic!("entry retains all three Boolean inputs")
    };
    let [entry, left, right, yes, shared, no] = machine.blocks.as_slice() else {
        panic!("balanced graph emits each control and leaf exactly once")
    };
    let [left_parameter] = left.parameters.as_slice() else {
        panic!("left frontier receives its Boolean")
    };
    let [right_parameter] = right.parameters.as_slice() else {
        panic!("right frontier receives its Boolean")
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
    assert_eq!(when_true.target, left.id);
    assert_eq!(when_true.arguments.as_slice(), [left_guard.id]);
    assert_eq!(when_false.target, right.id);
    assert_eq!(when_false.arguments.as_slice(), [right_guard.id]);
    let Terminator::Conditional {
        condition,
        when_false,
        ..
    } = &left.terminator
    else {
        panic!("left frontier lowers as a conditional")
    };
    assert_eq!(*condition, left_parameter.id);
    assert_eq!(when_false.target, shared.id);
    let Terminator::Conditional {
        condition,
        when_true,
        ..
    } = &right.terminator
    else {
        panic!("right frontier lowers as a conditional")
    };
    assert_eq!(*condition, right_parameter.id);
    assert_eq!(when_true.target, shared.id);
    for leaf in [yes, shared, no] {
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
    .expect("balanced convergent module verifies");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn balanced_control_rejects_convergent_edge_drift() {
    let mut checked = checked_balanced_nested_control();
    let no = checked.facts.flow.terminal_unit_effects.composed_machines[0].states[5].state;
    let CheckedComposedUnitControlTerminatorPlan::Conditional { when_false, .. } =
        &mut checked.facts.flow.terminal_unit_effects.composed_machines[0].states[1].terminator
    else {
        unreachable!()
    };
    when_false.target_state = no;
    assert!(matches!(
        lower_machine(&checked, "Root::enter"),
        Err(LoweringError::Unsupported(_))
    ));
}

#[test]
fn balanced_control_rejects_guard_drift() {
    let mut checked = checked_balanced_nested_control();
    let CheckedComposedUnitControlTerminatorPlan::Conditional { guard, .. } =
        &mut checked.facts.flow.terminal_unit_effects.composed_machines[0].states[0].terminator
    else {
        unreachable!()
    };
    *guard = CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Parameter {
        position: 1,
    }));
    assert!(matches!(
        lower_machine(&checked, "Root::enter"),
        Err(LoweringError::Unsupported(_))
    ));
}
