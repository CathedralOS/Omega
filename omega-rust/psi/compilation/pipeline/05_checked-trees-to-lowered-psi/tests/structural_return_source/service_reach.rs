use super::{
    AdmissionProfile, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalFuelMeter, TerminalStructuralResult, TerminalStructuralValue, decode_module,
};
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};
const NOMINAL_CALLBACK: &str = r#"
    data ByteUnit {}
    data CountedQuantity<Unit> { magnitude: u64; }
    trait Content<A> { machine project(subject: &Self) -> A; }
    data Region [linear] { length: u64; }
    domain Region::Owned;
    machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
    satisfies Content<CountedQuantity<ByteUnit>>::project
    { CountedQuantity { magnitude: region.length } }
    trait RegionForward {
        machine call(region: Region in Region::Owned) -> Region in Region::Owned;
    }
    data Main {}
    machine Main::forward(region: Region in Owned) -> Region in Owned
    satisfies RegionForward::call
    { region }
    machine Main::through_call<machine Selected>(region: Region in Owned) -> Region in Owned
    where machine Selected satisfies RegionForward::call;
    { Selected(region) }
    machine Main::demand(region: Region in Owned) -> Region in Owned {
        Main::through_call<Main::forward>(region)
    }
"#;

fn mixed_callback_source(body: &str) -> String {
    NOMINAL_CALLBACK.replace("{ Selected(region) }", &format!("{{ {body} }}"))
        + "machine Main::with_markers(before: u64, region: Region in Owned, after: u64) -> Region in Owned { region }\n machine Main::marker(value: u64) -> u64 { value }"
}

#[test]
fn mixed_linear_call_rejects_changed_source_operand_positions() {
    for body in [
        "let forwarded: Region in Owned = Selected(region); Main::with_markers(7, forwarded, 9)",
        "let forwarded: Region in Owned = Selected(region); Main::with_markers(Main::marker(7), forwarded, Main::marker(9))",
    ] {
        let checked = crate::front_end::checked_program(&mixed_callback_source(body));
        let _artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("Main::demand"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("unchanged mixed call publishes")
        .into_artifact();
        for mutation in ["scalar_order", "scalar_arity", "claim_position"] {
            let mut invalid = checked.clone();
            let operation = invalid.facts.flow.terminal_unit_effects.composed_machines
            .iter_mut()
            .flat_map(|machine| &mut machine.states)
            .flat_map(|state| &mut state.operations)
            .find(|operation| matches!(operation,
                checked_trees::CheckedUnitEffectOperationPlan::StructuralCall { scalar_arguments, .. }
                if scalar_arguments.len() == 2))
            .expect("mixed call in the shared graph");
            let checked_trees::CheckedUnitEffectOperationPlan::StructuralCall {
                scalar_arguments,
                custody,
                ..
            } = operation
            else {
                unreachable!()
            };
            match mutation {
                "scalar_order" => scalar_arguments.swap(0, 1),
                "scalar_arity" => {
                    scalar_arguments.pop();
                }
                "claim_position" => custody.claim_transfers[0].argument_index = 1,
                _ => unreachable!(),
            }
            assert!(
                terminal_production::TerminalProductionRequest::new(
                    &invalid,
                    TerminalMachineSelection::Name("Main::demand")
                )
                .produce(TerminalProductionCustody::artifact_only(
                    &mut TerminalProductionTimings::default()
                ))
                .is_err(),
                "changed {mutation} must reject at source replay"
            );
        }
    }
}

#[test]
fn mixed_linear_call_replay_rejects_stale_values_and_claims() {
    let checked = crate::front_end::checked_program(&mixed_callback_source(
        "let first: Region in Owned = Main::with_markers(7, region, 9); Main::with_markers(11, first, 13)",
    ));
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Main::demand"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("publish mixed producer and consumer")
    .into_artifact();
    drop(checked);
    execute_identity(&artifact, 32);
    let module = decode_module(artifact.semantic_bytes()).expect("reload without source");
    for mutation in [
        "scalar_arity",
        "future_scalar",
        "stale_input",
        "own_result",
        "returned_claim",
        "qualification",
        "multiplicity",
        "content_guarantee",
        "scalar_type",
    ] {
        let mut invalid = module.clone();
        if mutation == "content_guarantee" || mutation == "scalar_type" {
            let leaf = invalid
                .machines
                .iter_mut()
                .find(|machine| {
                    machine.parameters.len() == 2 && !machine.content_identity_reshuffles.is_empty()
                })
                .expect("mixed callee's checked content guarantee");
            if mutation == "content_guarantee" {
                leaf.content_identity_reshuffles.clear();
            } else {
                leaf.parameters[0].scalar_type = semantic_vocabulary::ScalarType::Boolean;
            }
        } else {
            let caller = invalid
                .machines
                .iter_mut()
                .find(|machine| {
                    machine.blocks.iter().flat_map(|block| &block.operations)
                    .filter(|operation| matches!(operation.kind,
                        terminal_psi::OperationKind::CallStructuralWithScalarArguments { .. }))
                    .count() == 2
                })
                .expect("two mixed calls in authored order");
            let calls = caller
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .filter(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::CallStructuralWithScalarArguments { .. }
                    )
                })
                .collect::<Vec<_>>();
            let [producer, consumer]: [&mut terminal_psi::Operation; 2] = calls.try_into().unwrap();
            let terminal_psi::OperationKind::CallStructuralWithScalarArguments {
                arguments: produced_arguments,
                ..
            } = &mut producer.kind
            else {
                unreachable!()
            };
            let terminal_psi::OperationKind::CallStructuralWithScalarArguments {
                arguments,
                structural_arguments,
                returned_claim_transfers,
                ..
            } = &mut consumer.kind
            else {
                unreachable!()
            };
            match mutation {
                "scalar_arity" => {
                    arguments.pop();
                }
                "future_scalar" => produced_arguments[0] = arguments[0],
                "stale_input" => {
                    structural_arguments[0].place = caller.structural_parameters[0].place
                }
                "own_result" => {
                    structural_arguments[0].place = consumer.result.structural().unwrap().place
                }
                "returned_claim" => returned_claim_transfers.clear(),
                "qualification" | "multiplicity" => {
                    let terminal_psi::OperationResult::Structural(result) = &mut consumer.result
                    else {
                        unreachable!()
                    };
                    if mutation == "qualification" {
                        result.qualifications.clear();
                    } else {
                        result.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
                    }
                }
                _ => unreachable!(),
            }
        }
        assert!(
            terminal_verifier::validate_module(&invalid).is_err(),
            "source-free {mutation} must reject independently of an old proof fingerprint"
        );
    }
}

#[test]
fn nominal_linear_callback_result_cannot_be_moved_twice() {
    for consumed in ["region", "forwarded"] {
        let source = NOMINAL_CALLBACK.replace(
            "{ Selected(region) }",
            &format!("{{ let forwarded: Region in Owned = Selected(region); let returned: Region in Owned = Main::forward(forwarded); Main::forward({consumed}) }}"),
        );
        assert!(
            crate::front_end::checked_program_result(&source).is_err(),
            "moving {consumed} twice must reject"
        );
    }
}

fn execute_identity(artifact: &terminal_codec::CanonicalTerminalArtifact, fuel: u64) {
    let module = decode_module(artifact.semantic_bytes()).expect("reload semantics");
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameter = &caller.structural_parameters[0];
    let argument = TerminalStructuralValue {
        opaque_identity: 0x5eed,
        structural_type: parameter.structural_type,
        qualifications: parameter.qualifications.clone(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: std::slice::from_ref(&argument),
            ..Default::default()
        },
    )
    .expect("verify and execute without source custody");
    let mut meter = TerminalFuelMeter::with_allowance(fuel);
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            TerminalStructuralResult {
                value: argument,
                claims: vec![caller.entry_claims[0].claim],
            }
        ))
    );
}
