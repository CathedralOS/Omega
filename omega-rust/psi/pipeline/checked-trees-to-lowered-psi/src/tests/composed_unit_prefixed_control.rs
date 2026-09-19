//! Scalar prefixes compose with ordinary graph edges and effect sequencing.

use super::{CheckedTrees, LoweringError, checked_source, lower_machine};
use checked_trees::CheckedComposedUnitControlTerminatorPlan;
use terminal_psi::{Operation, OperationKind, Terminator};

#[test]
fn interleaved_states_preserve_mixed_handoffs_and_effect_order() {
    use terminal_interpreter::{
        TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
        TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
        TerminalStructuralInputs,
    };

    let checked = checked_source(
        r#"
            boundary trait Host { machine emit(value: i32); }
            data Root {}
            machine Root::enter(flag: bool, value: i32) reaches Host {
                Host::emit(1);
                transition { _ -> dispatch(flag, value) }
                state yes(value: i32) {
                    Host::emit(value);
                    transition { _ -> done() }
                }
                state dispatch(flag: bool, value: i32) {
                    Host::emit(2);
                    transition flag { true -> yes(value) _ -> no() }
                }
                state done() { Host::emit(3); }
                state no() {
                    Host::emit(4);
                    transition { _ -> done() }
                }
            }
        "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Root::enter")
        .produce_artifact()
        .expect("mixed signatures and interleaved state declarations publish one graph");

    #[derive(Default)]
    struct Trace(Vec<i128>);
    impl TerminalEffectHandler for Trace {
        fn handle_effect(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<(), TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("only authored boundary calls are observable");
            };
            let [
                TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Signed(value),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("each boundary receives its one signed integer");
            };
            self.0.push(*value);
            Ok(())
        }
    }
    for (flag, expected) in [(true, vec![1, 2, 37, 3]), (false, vec![1, 2, 4, 3])] {
        let arguments = [
            TerminalScalarValue::Boolean(flag),
            TerminalScalarValue::Integer {
                scalar_type: semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Signed,
                    32,
                )
                .expect("i32 carrier"),
                value: semantic_vocabulary::IntegerValue::Signed(37),
            },
        ];
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &arguments,
            TerminalStructuralInputs::default(),
        )
        .expect("serialized graph independently checks");
        let mut trace = Trace::default();
        assert_eq!(
            execution
                .resume(
                    &mut terminal_fuel::TerminalFuelMeter::with_allowance(100),
                    &mut trace
                )
                .expect("acyclic graph executes"),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit),
        );
        assert_eq!(trace.0, expected);
    }
    let mut corrupted = checked;
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
        &mut corrupted.facts.flow.terminal_unit_effects.composed_machines[0].states[0].terminator
    else {
        panic!("entry forwards to the dispatch state");
    };
    successor.scalar_arguments.swap(0, 1);
    assert!(
        lower_machine(&corrupted, "Root::enter").is_err(),
        "mixed lanes cannot be swapped"
    );
}

fn checked_prefixed_control() -> CheckedTrees {
    checked_source(
        r#"
            boundary trait Host { machine exit(code: i32); }
            data Root {}
            machine Root::enter(flag: bool) reaches Host {
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

fn checked_multi_prefixed_control() -> CheckedTrees {
    checked_source(
        r#"
            boundary trait Host { machine exit(code: i32); }
            data Root {}
            machine Root::enter(flag: bool) reaches Host {
                transition { _ -> relay(flag) }
                state relay(flag: bool) { transition { _ -> dispatch(flag) } }
                state dispatch(flag: bool) {
                    transition flag { true -> yes() _ -> no() }
                }
                state yes() { Host::exit(1); }
                state no() { Host::exit(2); }
            }
        "#,
    )
}

fn checked_multi_prefixed_internal_control() -> CheckedTrees {
    checked_source(
        r#"
            data Root {}
            machine Root::quiet() {}
            machine Root::enter(flag: bool) {
                transition { _ -> relay(flag) }
                state relay(flag: bool) { transition { _ -> dispatch(flag) } }
                state dispatch(flag: bool) {
                    transition flag { true -> yes() _ -> no() }
                }
                state yes() { Root::quiet(); }
                state no() { Root::quiet(); }
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
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("prefixed composed module verifies");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("decode"),
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

#[test]
fn lowers_and_independently_replays_two_scalar_prefixes() {
    let checked = checked_multi_prefixed_control();
    let lowered = lower_machine(&checked, "Root::enter")
        .expect("the exact five-state effect graph should lower");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("multi-prefix boundary graph emits one machine")
    };
    let [entry_parameter] = machine.parameters.as_slice() else {
        panic!("multi-prefix graph retains its Boolean machine input")
    };
    let [entry, relay, dispatch, when_true, when_false] = machine.blocks.as_slice() else {
        panic!("two-prefix graph emits exactly five blocks")
    };
    let [relay_parameter] = relay.parameters.as_slice() else {
        panic!("relay receives one Boolean block argument")
    };
    let [dispatch_parameter] = dispatch.parameters.as_slice() else {
        panic!("dispatch receives one Boolean block argument")
    };
    for (block, target, argument) in [
        (entry, relay, entry_parameter.id),
        (relay, dispatch, relay_parameter.id),
    ] {
        assert!(matches!(
            &block.terminator,
            Terminator::Jump {
                target: actual_target,
                arguments,
                trivial_affine_discards,
                ..
            } if *actual_target == target.id
                && arguments.as_slice() == [argument]
                && trivial_affine_discards.is_empty()
        ));
    }
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
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("multi-prefix composed module verifies");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn multi_prefixed_control_rejects_second_edge_corruption() {
    let mut checked = checked_multi_prefixed_control();
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
        &mut checked.facts.flow.terminal_unit_effects.composed_machines[0].states[1].terminator
    else {
        unreachable!()
    };
    successor.scalar_arguments[0].source =
        checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter { index: 1 };
    assert!(matches!(
        lower_machine(&checked, "Root::enter"),
        Err(LoweringError::Unsupported(_))
    ));
}

#[test]
fn multi_prefixed_control_retains_its_internal_target_with_disjoint_root_blocks() {
    let checked = checked_multi_prefixed_internal_control();
    let lowered = lower_machine(&checked, "Root::enter")
        .expect("multi-prefix internal-call graph should lower");
    let [root, target] = lowered.semantic_module.machines.as_slice() else {
        panic!("multi-prefix root and one deduplicated target form the closure")
    };
    assert_eq!(root.blocks.len(), 5);
    assert_eq!(target.blocks.len(), 1);
    let mut block_ids = root
        .blocks
        .iter()
        .chain(&target.blocks)
        .map(|block| block.id.get())
        .collect::<Vec<_>>();
    block_ids.sort_unstable();
    assert_eq!(block_ids, vec![1, 2, 3, 4, 5, 6]);
    assert_eq!(root.entry, root.blocks[0].id);
    for leaf in &root.blocks[3..] {
        assert!(matches!(
            leaf.operations.as_slice(),
            [Operation {
                kind: OperationKind::CallUnit { callee, .. },
                ..
            }] if *callee == target.id
        ));
    }
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("multi-prefix internal-call module verifies");
}
