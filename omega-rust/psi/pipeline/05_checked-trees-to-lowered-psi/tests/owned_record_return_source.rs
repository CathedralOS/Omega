//! Whole parameter returns use the same ordered body as preceding calls and stores.

use checked_trees::{CheckedTrees, CheckedUnitEffectOperationPlan};
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalStructuralValue,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

fn fixture(property: &str, carrier: &str, parameters: &str, prefix: &str) -> CheckedTrees {
    let source = format!(
        "data Record {property} {{ first: u64; second: u64; third: u64; }}
         data Entry {{ value: u64; }}
         data Buffer<T> {{ entries: [T; 3]; }}
         machine identity(value: u64) -> u64 {{ value }}
         machine stamp(output: &mut u64, value: u64) -> u64 {{ output = value; value }}
         machine retain(mask: u64, {parameters} record: {carrier}) -> {carrier} {{ {prefix} record }}"
    );
    crate::front_end::checked_program(&source)
}

#[test]
fn owned_record_call_replay_rejects_same_type_argument_and_access_substitution() {
    for property in ["", "[copy]"] {
        let checked = crate::front_end::checked_program(&format!(
            "data Record {property} {{ first: u64; second: u64; third: u64; }}
             machine retain(record: Record) -> Record {{ record }}
             machine relay(left: Record, right: Record) -> Record {{ retain(left) }}"
        ));
        let _ = terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("relay"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .unwrap()
        .into_artifact();
        for mutation in 0..3 {
            let mut invalid = checked.clone();
            let operation = invalid
                .facts
                .flow
                .terminal_unit_effects
                .composed_machines
                .iter_mut()
                .flat_map(|machine| &mut machine.states)
                .flat_map(|state| &mut state.operations)
                .find(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::StructuralCall { .. }
                    )
                })
                .unwrap();
            let CheckedUnitEffectOperationPlan::StructuralCall {
                structural_arguments,
                coordinate,
                ..
            } = operation
            else {
                unreachable!()
            };
            match mutation {
                0 => {
                    structural_arguments[0].source =
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                            parameter_index: 1,
                        }
                }
                1 => {
                    structural_arguments[0].access =
                        checked_trees::CheckedStructuralAccess::SharedBorrow
                }
                _ => coordinate.call_ordinal += 1,
            }
            assert!(
                terminal_production::TerminalProductionRequest::new(
                    &invalid,
                    TerminalMachineSelection::Name("relay")
                )
                .produce(TerminalProductionCustody::artifact_only(
                    &mut TerminalProductionTimings::default()
                ))
                .is_err(),
                "call mutation {mutation}"
            );
        }
    }
}

#[test]
fn source_replay_rejects_return_parameter_and_carrier_substitution() {
    for (property, carrier, wrong_carrier) in [
        ("[copy]", "Record", "Entry"),
        ("", "[Entry; 3]", "[Entry; 2]"),
        ("", "Buffer<Entry>", "Buffer<u64>"),
    ] {
        let original = fixture(
            property,
            carrier,
            &format!("other: {carrier},"),
            "_ = identity(mask);",
        );
        let _ = terminal_production::TerminalProductionRequest::new(
            &original,
            TerminalMachineSelection::Name("retain"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .unwrap_or_else(|error| panic!("{carrier} before mutation: {error:?}"))
        .into_artifact();
        let source = original
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "retain")
            .unwrap()
            .symbol;
        for mutation in ["parameter", "carrier"] {
            let mut changed = original.clone();
            let effects = &mut changed.facts.flow.terminal_unit_effects;
            // Structural-result bodies are recorded either in the ordinary
            // catalog or in the composed state graph, depending on the
            // carrier; substitute in whichever catalog retained the plan.
            if let Some(plan) = effects
                .machines
                .iter_mut()
                .find(|plan| plan.machine == source)
            {
                let result = plan.structural_result.as_mut().unwrap();
                match mutation {
                    "parameter" => {
                        result.source =
                            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                parameter_index: 0,
                            };
                    }
                    _ => {
                        // Keep the plan's parameter and result consistent with each other,
                        // but substitute a carrier different from the source declaration.
                        result.type_identity = wrong_carrier.into();
                        plan.structural_parameters[1].type_identity = wrong_carrier.into();
                    }
                }
            } else {
                let graph = effects
                    .composed_machines
                    .iter_mut()
                    .find(|graph| graph.machine == source)
                    .expect("composed catalog retains the ordered body");
                match mutation {
                    "parameter" => {
                        for state in &mut graph.states {
                            if let checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result } =
                                &mut state.terminator
                            {
                                result.source =
                                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                        parameter_index: 0,
                                    };
                            }
                        }
                    }
                    _ => {
                        // Keep the plan's parameter and result consistent with each other,
                        // but substitute a carrier different from the source declaration.
                        if let checked_trees::CheckedControlResultPlan::Structural(result) =
                            &mut graph.result
                        {
                            result.type_identity = wrong_carrier.into();
                        }
                        for state in &mut graph.states {
                            state.structural_parameters[1].type_identity = wrong_carrier.into();
                            if let checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result } =
                                &mut state.terminator
                            {
                                result.type_identity = wrong_carrier.into();
                            }
                        }
                    }
                }
            }
            assert!(
                terminal_production::TerminalProductionRequest::new(
                    &changed,
                    TerminalMachineSelection::Name("retain")
                )
                .produce(TerminalProductionCustody::artifact_only(
                    &mut TerminalProductionTimings::default()
                ))
                .is_err(),
                "{carrier}: substituted return {mutation}"
            );
        }
    }
}

#[test]
fn owned_array_call_results_return_without_fabricated_claims() {
    let checked = crate::front_end::checked_program(
        "data Entry { value: u64; }
         machine forward(values: [Entry; 3]) -> [Entry; 3] { values }
         machine relay(values: [Entry; 3]) -> [Entry; 3] {
             let first: [Entry; 3] = forward(values);
             forward(first)
         }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("relay"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("a call-produced owned array uses its exact live owner at return")
    .into_artifact();
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    drop(checked);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let entry_position = module
        .machines
        .iter()
        .position(|machine| machine.id == module.entry)
        .unwrap();
    let entry = &module.machines[entry_position];
    let input = TerminalStructuralValue {
        opaque_identity: 73,
        structural_type: entry.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: std::slice::from_ref(&input),
            ..Default::default()
        },
    )
    .unwrap();
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(64);
    let TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(result)) = execution
        .resume(&mut fuel, &mut AcceptTerminalEffects)
        .unwrap()
    else {
        panic!("whole owned array call chain completes");
    };
    assert_eq!(result.value, input);
    assert!(result.claims.is_empty());
    for mutation in 0..3 {
        let mut changed = module.clone();
        let caller = &mut changed.machines[entry_position];
        let consumed_input = caller.structural_parameters[0].place;
        let returned = caller
            .blocks
            .iter_mut()
            .find_map(|block| match &mut block.terminator {
                terminal_psi::Terminator::ReturnStructural {
                    source,
                    returned_claims,
                    trivial_affine_discards,
                    ..
                } => Some((source, returned_claims, trivial_affine_discards)),
                _ => None,
            })
            .unwrap();
        match mutation {
            0 => *returned.0 = consumed_input,
            1 => returned
                .1
                .push(semantic_vocabulary::ClaimId::new(1).unwrap()),
            _ => returned.2.push(*returned.0),
        }
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &proof,
                &proof_admission::AdmissionProfile::default(),
            )
            .is_err(),
            "return must reject consumed owners, fabricated claims and double disposal: {mutation}"
        );
    }
}
