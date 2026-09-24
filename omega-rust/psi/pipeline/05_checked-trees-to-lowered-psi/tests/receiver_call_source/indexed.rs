//! Fixed-array receiver paths retain their referent across the portable boundary.

use super::{
    CheckedUnitEffectOperationPlan, OperationKind, TerminalExecution, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalScalarValue, TerminalStructuralValue, unit_plan,
};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};
fn source(signature: &str, receiver: &str) -> String {
    format!(
        "data Record [copy] {{ value: u16; }}
         data Entry [copy] {{ record: Record; }}
         data Container {{ records: [Record; 2]; entries: [Entry; 2]; }}
         machine Record::replace(&write self) {{ self.value = 17; }}
         machine {signature} {{ {receiver}.replace(); }}"
    )
}

#[test]
fn indexed_write_only_receiver_reaches_canonical_terminal() {
    let checked = crate::front_end::checked_program(
        "data Record [copy] { value: u16; }
         machine Record::replace(&write self) { self.value = 17; }
         machine forward(records: &write [Record; 2]) { records[1].replace(); }",
    );
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("forward"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("indexed write-only receiver retains its exact portable subloan")
    .into_artifact();
}

#[test]
fn indexed_ieee_write_only_receiver_retains_runtime_and_literal_stores() {
    for primitive in ["f32", "f64"] {
        for replacement in ["value", "1.25"] {
            let checked = crate::front_end::checked_program(&format!(
                "data Record [copy] {{ value: {primitive}; }}
                 machine Record::replace(&write self, value: {primitive}) {{ self.value = {replacement}; }}
                 machine forward(records: &write [Record; 2], value: {primitive}) {{ records[1].replace(value); }}"
            ));
            let artifact = terminal_production::TerminalProductionRequest::new(
                &checked,
                TerminalMachineSelection::Name("forward"),
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .expect("indexed IEEE receiver preserves canonical store custody")
            .into_artifact();
            let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
            let caller = module
                .machines
                .iter()
                .find(|machine| machine.id == module.entry)
                .unwrap();
            let value = if primitive == "f32" {
                semantic_vocabulary::IeeeFloatValue::Binary32(0x7fc0_0042)
            } else {
                semantic_vocabulary::IeeeFloatValue::Binary64(0x8000_0000_0000_0000)
            };
            let mut execution = TerminalExecution::start_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[TerminalScalarValue::IeeeFloat(value)],
                TerminalStructuralInputs {
                    arguments: &[TerminalStructuralValue {
                        opaque_identity: 73,
                        structural_type: caller.structural_parameters[0].structural_type,
                        qualifications: Vec::new(),
                        path: Vec::new(),
                    }],
                    ..Default::default()
                },
            )
            .expect("IEEE field receiver execution starts");
            assert_eq!(
                execution
                    .resume(
                        &mut terminal_fuel::TerminalFuelMeter::with_allowance(100),
                        &mut AcceptTerminalEffects
                    )
                    .unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
        }
    }
}

#[test]
fn retained_write_only_alias_preserves_the_indexed_receiver() {
    let checked = crate::front_end::checked_program(
        "data Record [copy] { value: u16; }
         machine Record::replace(&write self) { self.value = 17; }
         machine forward(records: &write [Record; 2]) {
             let held: &write [Record; 2] = &write records;
             held[1].replace();
         }",
    );
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("forward"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("erased alias preserves the original receiver and write-only access")
    .into_artifact();
}

#[test]
fn fixed_indexed_receiver_paths_keep_fields_and_nested_arrays() {
    for (signature, receiver, caller) in [
        (
            "forward(records: &write [[Record; 2]; 2])",
            "records[1][0]",
            "forward",
        ),
        (
            "forward(container: &write Container)",
            "container.records[1]",
            "forward",
        ),
        (
            "forward(entries: &write [Entry; 2])",
            "entries[1].record",
            "forward",
        ),
        (
            "Container::forward(&write self)",
            "self.records[1]",
            "Container::forward",
        ),
        (
            "Container::forward(&write self)",
            "records[1]",
            "Container::forward",
        ),
        (
            "Container::forward(&write self)",
            "self.entries[1].record",
            "Container::forward",
        ),
        (
            "forward(records: &mut [Record; 2])",
            "records[1]",
            "forward",
        ),
    ] {
        let source = source(signature, receiver);
        let checked = crate::front_end::checked_program(&source);
        let _artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name(caller),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .unwrap_or_else(|error| panic!("{receiver} must retain its source path: {error:?}"))
        .into_artifact();
    }
}

#[test]
fn indexed_receiver_plan_cannot_substitute_another_in_bounds_element() {
    let mut checked = crate::front_end::checked_program(&source(
        "forward(records: &write [Record; 2])",
        "records[1]",
    ));
    let caller = unit_plan(&checked, "forward").machine;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == caller)
        .unwrap();
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &mut plan.operations[0]
    else {
        panic!("one indexed receiver call");
    };
    structural_arguments[0].path[0] =
        checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0);
    assert!(
        terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("forward")
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default()
        ))
        .is_err(),
        "valid geometry for another element is not the authored receiver"
    );
}

#[test]
fn indexed_receiver_executes_once_across_every_fuel_boundary() {
    let checked = crate::front_end::checked_program(&source(
        "forward(records: &write [[Record; 2]; 2])",
        "records[1][0]",
    ));
    assert_indexed_receiver_fuel(&checked);
}

#[test]
fn erased_indexed_alias_executes_once_across_every_fuel_boundary() {
    let checked = crate::front_end::checked_program(
        "data Record [copy] { value: u16; }
         machine Record::replace(&write self) { self.value = 17; }
         machine forward(records: &mut [[Record; 2]; 2]) {
             let held: &write [[Record; 2]; 2] = &write records;
             held[1][0].replace();
         }",
    );
    assert_indexed_receiver_fuel(&checked);
}

#[test]
fn nested_indexed_alias_executes_once_across_every_fuel_boundary() {
    let checked = crate::front_end::checked_program(
        "data Record [copy] { value: u16; }
         machine Record::replace(&write self) { self.value = 17; }
         machine forward(records: &write [[Record; 2]; 2]) {
             let held: &write [[Record; 2]; 2] = &write records;
             let child: &write [[Record; 2]; 2] = &write held;
             child[1][0].replace();
         }",
    );
    assert_indexed_receiver_fuel(&checked);
}

#[test]
fn projected_alias_capture_executes_once_across_every_fuel_boundary() {
    for body in [
        "let held: &write [Record; 2] = &write records[1];
         held[0].replace();",
        "let held: &write [Record; 2] = &write records[1];
         let child: &write Record = &write held[0];
         child.replace();",
    ] {
        let checked = crate::front_end::checked_program(&format!(
            "data Record [copy] {{ value: u16; }}
             machine Record::replace(&write self) {{ self.value = 17; }}
             machine forward(records: &write [[Record; 2]; 2]) {{ {body} }}"
        ));
        assert_indexed_receiver_fuel(&checked);
    }
}

fn assert_indexed_receiver_fuel(checked: &checked_trees::CheckedTrees) {
    let artifact = terminal_production::TerminalProductionRequest::new(
        checked,
        TerminalMachineSelection::Name("forward"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        artifact.semantic_bytes()
    );
    let profile = proof_admission::AdmissionProfile::default();
    let verified = terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
    let certificate =
        terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, module.entry).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &caller.blocks[0].operations[0].kind
    else {
        panic!("receiver call");
    };
    assert_eq!(
        structural_arguments[0].path,
        vec![
            terminal_psi::StructuralPathSegment::FixedIndex(1),
            terminal_psi::StructuralPathSegment::FixedIndex(0)
        ]
    );
    for incremental in [false, true] {
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            &[],
            TerminalStructuralInputs {
                arguments: &[TerminalStructuralValue {
                    opaque_identity: 73,
                    structural_type: caller.structural_parameters[0].structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(if incremental {
            0
        } else {
            certificate.ceiling_units()
        });
        let mut complete = false;
        for _ in 0..=certificate.ceiling_units() {
            match execution
                .resume(&mut fuel, &mut AcceptTerminalEffects)
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(_) => {
                    assert!(incremental);
                    fuel.replenish(1).unwrap();
                }
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, TerminalExecutionResult::Unit);
                    complete = true;
                    break;
                }
                status => panic!("unexpected receiver execution status: {status:?}"),
            }
        }
        assert!(complete);
        assert_eq!(fuel.usage().total_units(), certificate.ceiling_units());
        for operation in module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
        {
            assert_eq!(
                fuel.usage()
                    .at(terminal_fuel::FuelChargeSite::Operation(operation.id))
                    .unwrap()
                    .executions(),
                1
            );
        }
    }
}

#[test]
fn indexed_receiver_keeps_a_scalar_parameter_separate_from_its_loan() {
    let checked = crate::front_end::checked_program(
        "data Record [copy] { value: u16; }
         machine Record::replace(&write self, replacement: u16) { self.value = replacement; }
         machine forward(replacement: u16, records: &mut [Record; 2]) { records[1].replace(replacement); }",
    );
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("forward"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
}

#[test]
fn dynamic_indexed_receiver_remains_checked_without_static_terminal_geometry() {
    let checked = crate::front_end::checked_program(&source(
        "forward(records: &write [Record; 2], index: u64 [0..=1])",
        "records[index]",
    ));
    assert!(
        terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("forward")
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default()
        ))
        .is_err()
    );
}

#[test]
fn unused_projected_receiver_keeps_existing_self_erasure() {
    for receiver in ["records[1]", "entries[1].record"] {
        let source =
            source("Container::forward(&write self)", receiver).replace("self.value = 17;", "");
        let checked = crate::front_end::checked_program(&source);
        let _artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("Container::forward"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .unwrap_or_else(|error| {
            panic!("an unused receiver remains erasable: {receiver}: {error:?}")
        })
        .into_artifact();
    }
}
