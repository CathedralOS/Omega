//! Ordinary Unit calls borrow original primitive locals across every scalar width.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

#[path = "primitive_local_unit_calls/crash_routes.rs"]
mod crash_routes;
#[path = "primitive_local_unit_calls/custody.rs"]
mod custody;
#[path = "primitive_local_unit_calls/reference_custody.rs"]
mod reference_custody;
#[path = "primitive_local_unit_calls/reference_initializer.rs"]
mod reference_initializer;

fn source(scalar: &str) -> String {
    format!(
        "machine replace(destination: &mut {scalar}, value: {scalar}) {{
            destination = value;
        }}
        machine observe(output: &mut {scalar}, initial: {scalar}, replacement: {scalar}) {{
            let mut scratch: {scalar} = initial;
            replace(&mut scratch, replacement);
            output = scratch;
        }}"
    )
}

fn artifact(source: &str) -> terminal_codec::CanonicalTerminalArtifact {
    let artifact = terminal_production::TerminalProductionRequest::new(
        &crate::front_end::checked_program(source),
        TerminalMachineSelection::Name("observe"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap_or_else(|error| panic!("{source}: {error:?}"))
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("reload module");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).expect("reload proof");
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independently verify reloaded artifact");
    artifact
}

fn integer(sign: IntegerSign, width: u16, value: IntegerValue) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(sign, width).unwrap(),
        value,
    }
}

fn execute(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    initial: TerminalScalarValue,
    replacement: TerminalScalarValue,
    expected: &[TerminalScalarValue],
) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameter = &caller.structural_parameters[0];
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[initial, replacement],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            primitive_values: &[TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: initial,
            }],
            ..Default::default()
        },
    )
    .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut observations = Vec::new();
    let mut complete = false;
    for _ in 0..64 {
        let status = execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap();
        observations.push(execution.structural_primitive_values()[0].value);
        match status {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Unit);
                complete = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            other => panic!("unexpected status {other:?}"),
        }
    }
    assert!(complete);
    observations.dedup();
    assert_eq!(observations, expected);
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        let usage = meter
            .usage()
            .at(terminal_fuel::FuelChargeSite::Operation(operation.id))
            .unwrap();
        assert_eq!(
            usage.executions(),
            1,
            "operation {:?} must not replay",
            operation.id
        );
        assert_eq!(usage.units(), 1);
    }
}
