use super::{
    CHANGED_CONFORMANCE_DYNAMIC_UNIT_SOURCE, DIRECT_DYNAMIC_UNIT_SOURCE,
    FORWARDED_CHANGED_CONFORMANCE_DYNAMIC_UNIT_SOURCE, FORWARDED_DIRECT_DYNAMIC_UNIT_SOURCE,
    FORWARDED_REBOUND_DYNAMIC_INTEGER_SOURCE, FORWARDED_REBOUND_DYNAMIC_UNIT_SOURCE,
    JOINED_DYNAMIC_BOOLEAN_FORWARD_SOURCE, JOINED_DYNAMIC_UNIT_FORWARD_SOURCE,
    MULTI_HOP_DYNAMIC_INTEGER_CONTROL_SOURCE, MULTI_HOP_DYNAMIC_INTEGER_SOURCE,
    MULTI_HOP_DYNAMIC_UNIT_SOURCE, REBOUND_DYNAMIC_UNIT_SOURCE,
    assert_dynamic_unit_artifact_executes, unsupported_message,
};
use crate::terminal_identities::value_id;
use crate::tests::{checked_source, lower_machine};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_psi::{Operation, OperationKind, OperationResult, Terminator, ValueDeclaration};

#[test]
fn lowers_transparent_forwarding_chain_after_a_two_predecessor_join() {
    let mut checked = checked_source(JOINED_DYNAMIC_BOOLEAN_FORWARD_SOURCE);
    let checked_catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let [joined] = checked_catalog.joined_scalar_calls.as_slice() else {
        panic!("one checked forwarded dynamic join expected: {checked_catalog:#?}")
    };
    let [joined_transfer, forwarded_transfer] =
        joined.when_true.call.forwarding_transfers.as_slice()
    else {
        panic!("two shared post-join forwarding transfers expected: {joined:#?}")
    };
    assert_eq!(
        joined.when_false.call.forwarding_transfers,
        [joined_transfer.clone(), forwarded_transfer.clone()]
    );
    assert_eq!(joined_transfer.source_predecessor_count, 2);
    assert_eq!(forwarded_transfer.source_predecessor_count, 1);
    assert_eq!(joined_transfer.source_paths.len(), 2);
    assert_eq!(forwarded_transfer.source_paths.len(), 2);
    assert!(joined_transfer.has_complete_source_custody(&checked_catalog.transfers));
    assert!(forwarded_transfer.has_complete_source_custody(&checked_catalog.transfers));

    let lowered = lower_machine(&checked, "Main::run")
        .expect("the transparent forwarding chain after the join should lower");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the forwarded joined Terminal module should verify");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 2);
    assert_eq!(catalog.parameters.len(), 3);
    assert_eq!(catalog.arguments.len(), 4);
    assert_eq!(catalog.parameter_dispatches.len(), 1);
    assert_eq!(
        catalog.arguments[2].source,
        terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal: 0 }
    );
    let first_helper = catalog.parameters[0].owner;
    let second_helper = catalog.parameters[1].owner;
    let final_helper = catalog.parameters[2].owner;
    assert_eq!(catalog.arguments[2].owner, first_helper);
    assert_eq!(catalog.arguments[3].owner, second_helper);
    assert_eq!(catalog.parameter_dispatches[0].owner, final_helper);
    let first_helper_machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == first_helper)
        .expect("first forwarding helper");
    assert!(first_helper_machine.blocks.iter().any(|block| {
        matches!(
            block.operations.as_slice(),
            [Operation {
                kind: OperationKind::CallStructuralScalar { callee, .. },
                ..
            }] if *callee == second_helper
        )
    }));
    assert_eq!(lowered.source_call_occurrences.len(), 5);

    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::run")
        .produce_artifact()
        .expect("forwarded joined module should encode canonically");
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("forwarded joined module should decode"),
        lowered.semantic_module,
    );

    let [joined] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .joined_scalar_calls
        .as_mut_slice()
    else {
        unreachable!("checked above")
    };
    joined.when_true.call.forwarding_transfers[1]
        .source_paths
        .pop();
    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic call drifted from checked flow custody",
    );
}

#[test]
fn joined_descriptor_helpers_reject_disagreeing_body_custody() {
    let mut checked = checked_source(JOINED_DYNAMIC_BOOLEAN_FORWARD_SOURCE);
    checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .joined_scalar_calls[0]
        .when_false
        .call
        .forwarding_helpers
        .pop();
    assert_eq!(
        unsupported_message(&checked),
        "joined source-call helper chain drifted from checked custody"
    );
}

#[test]
fn lowers_result_less_dynamic_join_through_the_shared_helper_chain() {
    let mut checked = checked_source(JOINED_DYNAMIC_UNIT_FORWARD_SOURCE);
    let checked_catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert!(checked_catalog.direct_unit_calls.is_empty());
    let [joined] = checked_catalog.joined_unit_calls.as_slice() else {
        panic!("one checked result-less join expected: {checked_catalog:#?}")
    };
    assert_eq!(joined.when_true.call.forwarding_transfers.len(), 2);
    assert_eq!(
        joined.when_true.call.forwarding_transfers,
        joined.when_false.call.forwarding_transfers,
    );

    let lowered =
        lower_machine(&checked, "Main::run").expect("the result-less descriptor join should lower");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the result-less joined Terminal module should verify");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 2);
    assert_eq!(catalog.parameters.len(), 3);
    assert_eq!(catalog.arguments.len(), 4);
    assert_eq!(catalog.parameter_dispatches.len(), 1);
    assert!(
        catalog.parameters[0]
            .requirements
            .iter()
            .all(|requirement| {
                requirement.result == terminal_psi::ClosedConformanceCallableResult::Unit
            })
    );
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("joined dynamic Unit caller");
    assert_eq!(caller.blocks.len(), 3);
    assert!(caller.blocks[1..].iter().all(|block| {
        matches!(
            block.operations.as_slice(),
            [Operation {
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit { .. },
                ..
            }]
        )
    }));
    assert_eq!(lowered.source_call_occurrences.len(), 5);

    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::run")
        .produce_artifact()
        .expect("result-less joined module should encode canonically");
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("result-less joined module should decode"),
        lowered.semantic_module,
    );
    let [self_parameter] = caller.structural_parameters.as_slice() else {
        panic!("joined dynamic Unit caller requires structural self")
    };
    for choose_first in [true, false] {
        let structural = terminal_interpreter::TerminalStructuralValue {
            opaque_identity: 1,
            structural_type: self_parameter.structural_type,
            qualifications: self_parameter.qualifications.clone(),
            path: Vec::new(),
        };
        let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[terminal_interpreter::TerminalScalarValue::Boolean(
                choose_first,
            )],
            TerminalStructuralInputs {
                arguments: &[structural],
                ..Default::default()
            },
        )
        .expect("result-less joined artifact should start");
        let mut meter = terminal_fuel::TerminalFuelMeter::unbounded();
        assert_eq!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .expect("result-less joined artifact should execute"),
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Unit,
            ),
        );
    }

    let [joined] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .joined_unit_calls
        .as_mut_slice()
    else {
        unreachable!("checked above")
    };
    joined.when_false.successor.target_state = joined.when_true.successor.target_state;
    assert_eq!(
        unsupported_message(&checked),
        "joined dynamic control plan drifted from checked custody",
    );
}

#[test]
fn lowers_parameter_sourced_dynamic_forwarding_as_two_explicit_helpers() {
    let mut checked = checked_source(MULTI_HOP_DYNAMIC_INTEGER_SOURCE);
    let checked_catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert_eq!(checked_catalog.transfers.len(), 2);
    let [plan] = checked_catalog.direct_scalar_calls.as_slice() else {
        panic!("one multi-hop checked plan expected: {checked_catalog:#?}")
    };
    assert_eq!(plan.forwarding_transfers.len(), 1);

    let lowered = lower_machine(&checked, "Main::run")
        .expect("the exact parameter-sourced forwarding path should lower");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the multi-hop dynamic module should verify");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 1);
    assert_eq!(catalog.parameters.len(), 2);
    assert_eq!(catalog.arguments.len(), 2);
    assert_eq!(catalog.parameter_dispatches.len(), 1);
    assert_eq!(
        catalog.arguments[0].source,
        terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 0 }
    );
    assert_eq!(
        catalog.arguments[1].source,
        terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal: 0 }
    );
    assert_eq!(catalog.arguments[1].owner, catalog.parameters[0].owner);
    assert_eq!(
        catalog.parameter_dispatches[0].owner,
        catalog.parameters[1].owner
    );
    assert_eq!(lowered.source_call_occurrences.len(), 3);

    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::run")
        .produce_artifact()
        .expect("multi-hop dynamic module should encode canonically");
    let decoded = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("multi-hop dynamic module should decode");
    assert_eq!(decoded.dynamic_dispatch.parameters.len(), 2);
    assert_eq!(
        decoded.dynamic_dispatch.arguments[1].source,
        terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal: 0 }
    );

    let [plan] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .direct_scalar_calls
        .as_mut_slice()
    else {
        unreachable!("checked above")
    };
    plan.forwarding_transfers[0].coordinate.statement_index = 1;
    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic call drifted from checked flow custody"
    );
}

#[test]
fn retains_multi_hop_forwarded_scalar_result_control() {
    let mut checked = checked_source(MULTI_HOP_DYNAMIC_INTEGER_CONTROL_SOURCE);
    let checked_catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert_eq!(checked_catalog.transfers.len(), 2);
    let [plan] = checked_catalog.direct_scalar_calls.as_slice() else {
        panic!("one multi-hop checked continuation plan expected: {checked_catalog:#?}")
    };
    assert_eq!(plan.forwarding_transfers.len(), 1);
    assert!(plan.unit_continuation.is_some());

    let lowered = lower_machine(&checked, "Main::run")
        .expect("the multi-hop scalar result continuation should lower");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the multi-hop scalar result continuation should verify");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 1);
    assert_eq!(catalog.parameters.len(), 2);
    assert_eq!(catalog.arguments.len(), 2);
    assert_eq!(catalog.parameter_dispatches.len(), 1);
    assert_eq!(
        catalog.arguments[0].source,
        terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 0 }
    );
    assert_eq!(
        catalog.arguments[1].source,
        terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal: 0 }
    );
    assert_eq!(catalog.arguments[1].owner, catalog.parameters[0].owner);
    assert_eq!(
        catalog.parameter_dispatches[0].owner,
        catalog.parameters[1].owner
    );
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("caller machine");
    assert_eq!(caller.blocks.len(), 3);
    assert!(matches!(
        caller.blocks[0].terminator,
        Terminator::Conditional { .. }
    ));
    assert_eq!(lowered.source_call_occurrences.len(), 5);

    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::run")
        .produce_artifact()
        .expect("multi-hop scalar result control should encode canonically");
    let decoded = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("multi-hop scalar result control should decode");
    assert_eq!(decoded, lowered.semantic_module);

    let [plan] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .direct_scalar_calls
        .as_mut_slice()
    else {
        unreachable!("checked above")
    };
    plan.forwarding_transfers[0].coordinate.statement_index = 1;
    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic call drifted from checked flow custody"
    );
}

#[test]
fn forwarded_descriptor_helper_preserves_scalar_computation_and_branch() {
    let source = MULTI_HOP_DYNAMIC_INTEGER_SOURCE.replace(
        "let result: i32 = finish(erased);\n        transition { _ -> result }",
        "let before: i32 = 3;\n        let result: i32 = finish(erased);\n        let combined: i32 = result ^ before;\n        transition combined == 0 { true -> 7 _ -> combined }",
    );
    assert_ne!(source, MULTI_HOP_DYNAMIC_INTEGER_SOURCE);
    let checked = checked_source(&source);
    let lowered = lower_machine(&checked, "Main::run")
        .expect("descriptor forwarding retains the helper's actual scalar body");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("composed forwarding independently verifies");
    let parameter = &lowered.semantic_module.dynamic_dispatch.parameters[0];
    let helper = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == parameter.owner)
        .expect("first forwarding helper");
    assert!(
        helper
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, Terminator::Conditional { .. }))
    );
}

fn composed_helper_source() -> String {
    MULTI_HOP_DYNAMIC_INTEGER_CONTROL_SOURCE
        .replace("transition result == 0", "transition result == 7")
        .replace(
            "let result: i32 = finish(erased);\n        transition { _ -> result }",
            "let before: i32 = 3;\n        let result: i32 = finish(erased);\n        let combined: i32 = result ^ before;\n        transition combined == 0 { true -> 7 _ -> combined }",
        )
}

#[test]
fn forwarded_descriptor_calculations_execute_through_verified_artifact() {
    use terminal_interpreter::{
        TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalScalarValue,
    };
    #[derive(Default)]
    struct Observe(Vec<TerminalScalarValue>);
    impl TerminalEffectHandler for Observe {
        fn handle_effect(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<(), TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("Console effect")
            };
            self.0.extend(arguments.iter().copied());
            Ok(())
        }
    }
    let integer = |value| TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Signed,
            32,
        )
        .unwrap(),
        value: semantic_vocabulary::IntegerValue::Signed(value),
    };
    // The second body also computes after the actual parameter dispatch,
    // exercising both helper roles through the same evaluator.
    for final_computation in [false, true] {
        let source = if final_computation {
            composed_helper_source().replace(
                "let result: i32 = erased.measure();\n        transition { _ -> result }",
                "let result: i32 = erased.measure();\n        let mask: i32 = 1;\n        let combined: i32 = result ^ mask;\n        transition { _ -> combined }",
            )
        } else {
            composed_helper_source()
        };
        let checked = checked_source(&source);
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::run")
            .produce_artifact()
            .expect("composed helper source publishes canonical verified artifact");
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let parameter = &entry.structural_parameters[0];
        let field = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                OperationKind::IntegerStructuralField { field, .. } => Some(field),
                _ => None,
            })
            .expect("original receiver integer field");
        for (input, expected) in [(if final_computation { 2 } else { 3 }, 70), (6, 71)] {
            let argument = terminal_interpreter::TerminalStructuralValue {
                opaque_identity: 1,
                structural_type: parameter.structural_type,
                qualifications: parameter.qualifications.clone(),
                path: Vec::new(),
            };
            let field_value = terminal_interpreter::TerminalStructuralScalarFieldValue {
                argument_index: 0,
                path: module.dynamic_dispatch.selections[0].source.path.clone(),
                field,
                value: integer(input),
            };
            let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[],
                TerminalStructuralInputs {
                    arguments: &[argument],
                    scalar_fields: &[field_value],
                    ..Default::default()
                },
            )
            .expect("source-rooted helper artifact starts");
            let mut observe = Observe::default();
            assert_eq!(
                execution
                    .resume(
                        &mut terminal_fuel::TerminalFuelMeter::unbounded(),
                        &mut observe
                    )
                    .unwrap(),
                terminal_interpreter::TerminalExecutionStatus::Complete(
                    terminal_interpreter::TerminalExecutionResult::Unit
                )
            );
            assert_eq!(
                observe.0,
                [integer(expected)],
                "final computation {final_computation}, input {input}"
            );
        }
    }
}

#[test]
fn forwarded_descriptor_helper_rejects_substituted_body_custody() {
    let checked = checked_source(&composed_helper_source());
    for mutation in 0..6 {
        let mut changed = checked.clone();
        let helper = &mut changed
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .direct_scalar_calls[0]
            .forwarding_helpers[0];
        match mutation {
            0 => {
                helper.scalar_locals.remove(0);
            }
            1 => helper.scalar_locals[0].0.binding_ordinal += 1,
            2 => helper.scalar_locals[0].1 = helper.scalar_locals[1].1.clone(),
            3 => helper.call_result.statement_index = 0,
            4 => {
                helper.scalar_control.terminator =
                    checked_trees::CheckedScalarStateTerminator::Return {
                        statement_ordinal: 3,
                    }
            }
            5 => {
                let checked_trees::CheckedScalarStateTerminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } = &mut helper.scalar_control.terminator
                else {
                    panic!("conditional helper")
                };
                std::mem::swap(when_true, when_false);
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, "Main::run").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn lowers_parameter_sourced_dynamic_unit_forwarding_as_two_explicit_helpers() {
    let mut checked = checked_source(MULTI_HOP_DYNAMIC_UNIT_SOURCE);
    let checked_catalog = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    assert_eq!(checked_catalog.transfers.len(), 2);
    let [plan] = checked_catalog.direct_unit_calls.as_slice() else {
        panic!("one multi-hop checked Unit plan expected: {checked_catalog:#?}")
    };
    assert_eq!(plan.forwarding_transfers.len(), 1);

    let lowered = lower_machine(&checked, "Main::run")
        .expect("the exact parameter-sourced Unit forwarding path should lower");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the multi-hop dynamic Unit module should verify");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 1);
    assert_eq!(catalog.parameters.len(), 2);
    assert_eq!(catalog.arguments.len(), 2);
    assert_eq!(catalog.parameter_dispatches.len(), 1);
    assert_eq!(
        catalog.arguments[1].source,
        terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal: 0 }
    );
    assert_eq!(catalog.arguments[1].owner, catalog.parameters[0].owner);
    assert_eq!(
        catalog.parameter_dispatches[0].owner,
        catalog.parameters[1].owner
    );
    assert!(lowered.semantic_module.machines.iter().all(|machine| {
        machine.result == terminal_psi::TerminalMachineResult::Unit
            && machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .all(|operation| operation.result == OperationResult::Unit)
    }));
    assert_eq!(lowered.source_call_occurrences.len(), 3);

    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::run")
        .produce_artifact()
        .expect("multi-hop dynamic Unit module should encode canonically");
    let decoded = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("multi-hop dynamic Unit module should decode");
    assert_eq!(decoded.dynamic_dispatch.parameters.len(), 2);
    assert_eq!(
        decoded.dynamic_dispatch.arguments[1].source,
        terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal: 0 }
    );
    assert_dynamic_unit_artifact_executes(&artifact);

    let [plan] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .direct_unit_calls
        .as_mut_slice()
    else {
        unreachable!("checked above")
    };
    plan.forwarding_transfers[0].coordinate.statement_index = 1;
    assert_eq!(
        unsupported_message(&checked),
        "dynamic Unit call drifted from checked flow custody"
    );
}

#[test]
fn lowers_direct_dynamic_unit_without_allocating_a_scalar_result() {
    let checked = checked_source(DIRECT_DYNAMIC_UNIT_SOURCE);
    let lowered = lower_machine(&checked, "Main::run").expect("direct dynamic Unit call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("direct dynamic Unit module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 1);
    assert_eq!(catalog.direct_dispatches.len(), 1);
    assert!(catalog.rebound_descriptors.is_empty());
    assert!(catalog.indirect_dispatches.is_empty());
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("dynamic Unit caller");
    let [operation] = caller.blocks[0].operations.as_slice() else {
        panic!("one direct Unit operation expected")
    };
    assert_eq!(operation.result, OperationResult::Unit);
    assert!(matches!(operation.kind, OperationKind::CallUnit { .. }));
    let [application] = lowered
        .semantic_module
        .closed_conformance_applications
        .as_slice()
    else {
        panic!("one exact dynamic Unit application expected")
    };
    let [callable] = application.realization_callables.as_slice() else {
        panic!("one Unit realization expected")
    };
    assert_eq!(
        callable.result,
        terminal_psi::ClosedConformanceCallableResult::Unit
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::run")
        .produce_artifact()
        .expect("direct dynamic Unit module encodes");
    let decoded = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("direct dynamic Unit module decodes");
    assert_eq!(decoded, lowered.semantic_module);
    assert_dynamic_unit_artifact_executes(&artifact);
}

#[test]
fn lowers_rebound_dynamic_unit_to_a_resultless_indirect_dispatch() {
    let checked = checked_source(REBOUND_DYNAMIC_UNIT_SOURCE);
    let mut lowered =
        lower_machine(&checked, "Main::run").expect("rebound dynamic Unit call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("rebound dynamic Unit module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 2);
    assert_eq!(catalog.rebound_descriptors.len(), 1);
    assert_eq!(catalog.indirect_dispatches.len(), 1);
    let caller = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("dynamic Unit caller");
    let [operation] = caller.blocks[0].operations.as_mut_slice() else {
        panic!("one indirect Unit operation expected")
    };
    assert_eq!(operation.result, OperationResult::Unit);
    assert!(matches!(
        operation.kind,
        OperationKind::CallDynamicUnit {
            descriptor_ordinal: 0,
            ..
        }
    ));
    operation.result = OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(999),
        scalar_type: semantic_vocabulary::ScalarType::Boolean,
    });
    assert!(terminal_verifier::validate_module(&lowered.semantic_module).is_err());
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::run")
        .produce_artifact()
        .expect("rebound dynamic Unit module encodes");
    assert_dynamic_unit_artifact_executes(&artifact);
}

#[test]
fn retains_changed_conformance_unit_applications_without_a_scalar_result() {
    let lowered = lower_machine(
        &checked_source(CHANGED_CONFORMANCE_DYNAMIC_UNIT_SOURCE),
        "Main::run",
    )
    .expect("changed-conformance dynamic Unit call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("changed-conformance dynamic Unit module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 2);
    assert_ne!(
        catalog.selections[0].conformance_application_commitment,
        catalog.selections[1].conformance_application_commitment
    );
    assert_eq!(
        lowered
            .semantic_module
            .closed_conformance_applications
            .len(),
        2
    );
    assert!(matches!(
        lowered.semantic_module.machines[0].blocks[0].operations[0],
        Operation {
            result: OperationResult::Unit,
            kind: OperationKind::CallDynamicUnit { .. },
            ..
        }
    ));
}

#[test]
fn forwards_changed_conformance_unit_custody_without_a_scalar_result() {
    let lowered = lower_machine(
        &checked_source(FORWARDED_CHANGED_CONFORMANCE_DYNAMIC_UNIT_SOURCE),
        "Main::run",
    )
    .expect("forwarded changed-conformance dynamic Unit call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("forwarded changed-conformance dynamic Unit module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 2);
    assert_ne!(
        catalog.selections[0].conformance_application_commitment,
        catalog.selections[1].conformance_application_commitment
    );
    assert_eq!(catalog.arguments.len(), 1);
    assert_eq!(catalog.parameter_dispatches.len(), 1);
    assert!(matches!(
        catalog.arguments[0].source,
        terminal_psi::TerminalDynamicDescriptorSource::ReboundDescriptor { .. }
    ));
    assert!(lowered.semantic_module.machines.iter().all(|machine| {
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .all(|operation| operation.result == OperationResult::Unit)
    }));
}

#[test]
fn preserves_forwarded_dynamic_unit_parameter_abi_without_a_result_value() {
    let checked = checked_source(FORWARDED_REBOUND_DYNAMIC_UNIT_SOURCE);
    let lowered = lower_machine(&checked, "Main::run").expect("forwarded dynamic Unit call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("forwarded dynamic Unit module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    let [parameter] = catalog.parameters.as_slice() else {
        panic!("one dynamic Unit parameter expected, got {catalog:#?}")
    };
    let [argument] = catalog.arguments.as_slice() else {
        panic!("one dynamic Unit argument expected, got {catalog:#?}")
    };
    let [dispatch] = catalog.parameter_dispatches.as_slice() else {
        panic!("one dynamic Unit parameter dispatch expected, got {catalog:#?}")
    };
    assert_eq!(parameter.owner, dispatch.owner);
    assert_eq!(
        parameter.requirements[0].result,
        terminal_psi::ClosedConformanceCallableResult::Unit
    );
    assert_eq!(argument.parameter_ordinal, parameter.ordinal);
    assert_eq!(
        argument.source,
        terminal_psi::TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal: 0 }
    );
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == argument.owner)
        .expect("forwarded Unit caller");
    let outer = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == argument.operation)
        .expect("outer Unit call");
    assert_eq!(outer.result, OperationResult::Unit);
    assert!(matches!(outer.kind, OperationKind::CallUnit { .. }));
    let helper = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == parameter.owner)
        .expect("forwarded Unit helper");
    let inner = helper
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == dispatch.operation)
        .expect("inner Unit parameter dispatch");
    assert_eq!(inner.result, OperationResult::Unit);
    assert!(matches!(
        inner.kind,
        OperationKind::CallDynamicParameterUnit {
            parameter_ordinal: 0,
            requirement_slot: 0,
            ..
        }
    ));
    assert_eq!(lowered.source_call_occurrences.len(), 2);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::run")
        .produce_artifact()
        .expect("forwarded dynamic Unit module encodes");
    terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("forwarded dynamic Unit module decodes");
    assert_dynamic_unit_artifact_executes(&artifact);
}

#[test]
fn forwards_a_direct_dynamic_unit_selection_without_fabricating_a_rebound_descriptor() {
    let checked = checked_source(FORWARDED_DIRECT_DYNAMIC_UNIT_SOURCE);
    let mut lowered =
        lower_machine(&checked, "Main::run").expect("direct forwarded dynamic Unit call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("direct forwarded dynamic Unit module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 1);
    assert!(catalog.rebound_descriptors.is_empty());
    let [parameter] = catalog.parameters.as_slice() else {
        panic!("one direct dynamic Unit parameter expected, got {catalog:#?}")
    };
    let [argument] = catalog.arguments.as_slice() else {
        panic!("one direct dynamic Unit argument expected, got {catalog:#?}")
    };
    let [dispatch] = catalog.parameter_dispatches.as_slice() else {
        panic!("one direct dynamic Unit parameter dispatch expected, got {catalog:#?}")
    };
    assert_eq!(parameter.owner, dispatch.owner);
    assert_eq!(
        argument.source,
        terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 0 }
    );
    assert_eq!(
        parameter.requirements[0].result,
        terminal_psi::ClosedConformanceCallableResult::Unit
    );
    assert_eq!(lowered.source_call_occurrences.len(), 2);

    lowered.semantic_module.dynamic_dispatch.arguments[0].source =
        terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 1 };
    assert!(terminal_verifier::validate_module(&lowered.semantic_module).is_err());

    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::run")
        .produce_artifact()
        .expect("direct forwarded dynamic Unit module encodes");
    let decoded = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("direct forwarded dynamic Unit module decodes");
    assert_eq!(
        decoded.dynamic_dispatch.arguments[0].source,
        terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 0 }
    );
    assert_dynamic_unit_artifact_executes(&artifact);
}

#[test]
fn preserves_forwarded_dynamic_parameter_abi_from_checked_source() {
    let checked = checked_source(FORWARDED_REBOUND_DYNAMIC_INTEGER_SOURCE);
    let lowered =
        lower_machine(&checked, "Main::run").expect("forwarded dynamic parameter source lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("forwarded dynamic parameter module verifies");

    let catalog = &lowered.semantic_module.dynamic_dispatch;
    let [parameter] = catalog.parameters.as_slice() else {
        panic!("one forwarded dynamic parameter expected, got {catalog:#?}")
    };
    let [argument] = catalog.arguments.as_slice() else {
        panic!("one forwarded dynamic argument expected, got {catalog:#?}")
    };
    let [dispatch] = catalog.parameter_dispatches.as_slice() else {
        panic!("one forwarded parameter dispatch expected, got {catalog:#?}")
    };
    assert_eq!(parameter.owner, dispatch.owner);
    assert_eq!(parameter.ordinal, 0);
    assert_eq!(parameter.source_position, 0);
    assert_eq!(parameter.requirements.len(), 1);
    assert_eq!(argument.parameter_ordinal, parameter.ordinal);
    assert_eq!(
        argument.source,
        terminal_psi::TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal: 0 }
    );
    assert!(catalog.indirect_dispatches.is_empty());

    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == argument.owner)
        .expect("forwarded caller machine");
    let caller_operation = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == argument.operation)
        .expect("forwarded caller operation");
    assert!(matches!(
        caller_operation.kind,
        OperationKind::CallStructuralScalar { callee, .. } if callee == parameter.owner
    ));
    let helper = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == parameter.owner)
        .expect("forwarded helper machine");
    let helper_operation = helper
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == dispatch.operation)
        .expect("forwarded helper dispatch operation");
    assert!(matches!(
        helper_operation.kind,
        OperationKind::CallDynamicParameterScalar {
            parameter_ordinal: 0,
            requirement_slot: 0,
            ..
        }
    ));
    assert_eq!(lowered.source_call_occurrences.len(), 2);

    let produced = terminal_production::TerminalProductionRequest::new(&checked, "Main::run")
        .produce_artifact()
        .expect("source-produced dynamic parameter module encodes");
    let decoded = terminal_codec::decode_module(produced.semantic_bytes())
        .expect("source-produced dynamic parameter module decodes");
    assert_eq!(decoded, lowered.semantic_module);
}
