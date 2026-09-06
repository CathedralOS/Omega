//! Fixed-array receiver paths retain their referent across the portable boundary.

use super::*;

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
    let checked = checked_from_source(
        "data Record [copy] { value: u16; }
         machine Record::replace(&write self) { self.value = 17; }
         machine forward(records: &write [Record; 2]) { records[1].replace(); }",
    );
    let _artifact = terminal_production::produce_terminal_artifact(&checked, "forward")
        .expect("indexed write-only receiver retains its exact portable subloan");
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
        let checked = checked_from_source(&source);
        let _artifact = terminal_production::produce_terminal_artifact(&checked, caller)
            .unwrap_or_else(|error| panic!("{receiver} must retain its source path: {error:?}"));
    }
}

#[test]
fn indexed_receiver_plan_cannot_substitute_another_in_bounds_element() {
    let mut checked = checked_from_source(&source(
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
        terminal_production::produce_terminal_artifact(&checked, "forward").is_err(),
        "valid geometry for another element is not the authored receiver"
    );
}

#[test]
fn indexed_receiver_executes_once_across_every_fuel_boundary() {
    let checked = checked_from_source(&source(
        "forward(records: &write [[Record; 2]; 2])",
        "records[1][0]",
    ));
    let artifact = terminal_production::produce_terminal_artifact(&checked, "forward").unwrap();
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
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            &[],
            &[TerminalStructuralValue {
                opaque_identity: 73,
                structural_type: caller.structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
        )
        .unwrap();
        let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(if incremental {
            0
        } else {
            certificate.ceiling_units()
        });
        let mut complete = false;
        for _ in 0..=certificate.ceiling_units() {
            match execution.resume(&mut fuel).unwrap() {
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
    let checked = checked_from_source(
        "data Record [copy] { value: u16; }
         machine Record::replace(&write self, replacement: u16) { self.value = replacement; }
         machine forward(replacement: u16, records: &mut [Record; 2]) { records[1].replace(replacement); }",
    );
    let _artifact = terminal_production::produce_terminal_artifact(&checked, "forward").unwrap();
}

#[test]
fn dynamic_indexed_receiver_remains_checked_without_static_terminal_geometry() {
    let checked = checked_from_source(&source(
        "forward(records: &write [Record; 2], index: u64 [0..=1])",
        "records[index]",
    ));
    assert!(terminal_production::produce_terminal_artifact(&checked, "forward").is_err());
}

#[test]
fn unused_projected_receiver_keeps_existing_self_erasure() {
    for receiver in ["records[1]", "entries[1].record"] {
        let source =
            source("Container::forward(&write self)", receiver).replace("self.value = 17;", "");
        let checked = checked_from_source(&source);
        let _artifact =
            terminal_production::produce_terminal_artifact(&checked, "Container::forward")
                .unwrap_or_else(|error| {
                    panic!("an unused receiver remains erasable: {receiver}: {error:?}")
                });
    }
}
