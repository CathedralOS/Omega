//! Fixtures shared by the boundary wrapper tests: sources, artifacts, the
//! observing handler and the guarantee sources.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
#[path = "boundary_wrappers/ordered_boolean_guarantees.rs"]
mod ordered_boolean_guarantees;
#[path = "boundary_wrappers/scalar_guarantees_and_boundary_requirements.rs"]
mod scalar_guarantees_and_boundary_requirements;
#[path = "boundary_wrappers/scalar_wrappers_and_boundary_returns.rs"]
mod scalar_wrappers_and_boundary_returns;
#[path = "boundary_wrappers/wrapper_composition_and_custody.rs"]
mod wrapper_composition_and_custody;

use crate::unit_scalar_result_source::{SOURCE, checked_from_source};
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalExecution, TerminalExecutionStatus, TerminalScalarValue, TerminalStructuralValue,
};

fn source() -> String {
    SOURCE
        .replace("Host::measure(70)", "Scalar::measure()")
        .replace(
            "data Main {}",
            r#"
        data Scalar {}
        machine Scalar::measure() -> i32 reaches Host {
            let result: i32 = Host::measure(70);
            result
        }
        data Main {}
        "#,
        )
}

fn artifact(checked: &checked_trees::CheckedTrees) -> (Vec<u8>, Vec<u8>) {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect("Unit closure retains scalar boundary body");
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let evidence = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    let module = decode_module(&semantic).unwrap();
    let proof = decode_proof_bundle(&evidence).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    let published = terminal_production::TerminalProductionRequest::new(
        checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("source-owned shared closure publishes")
    .into_artifact();
    assert_eq!(decode_module(published.semantic_bytes()).unwrap(), module);
    (semantic, evidence)
}

fn integer(value: i128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Signed,
            32,
        )
        .unwrap(),
        value: semantic_vocabulary::IntegerValue::Signed(value),
    }
}

#[derive(Default)]
struct Observe {
    arguments: Vec<Vec<TerminalScalarValue>>,
}

impl TerminalEffectHandler for Observe {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        panic!("scalar boundaries require the result-bearing handler")
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            result,
            structural_arguments,
            ..
        } = effect
        else {
            panic!("expected boundary call")
        };
        assert!(structural_arguments.is_empty());
        self.arguments.push(arguments.clone());
        Ok(match result {
            terminal_psi::BoundaryMachineResult::Scalar(_) => {
                TerminalEffectResult::Scalar(arguments[0])
            }
            terminal_psi::BoundaryMachineResult::Unit => TerminalEffectResult::Unit,
            _ => panic!("no structural return in this scalar call lane"),
        })
    }
}

fn execute(artifact: &(Vec<u8>, Vec<u8>)) -> (TerminalExecutionStatus, Observe) {
    let module = decode_module(&artifact.0).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameters = entry
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 100 + index as u64,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &parameters,
            ..Default::default()
        },
    )
    .unwrap();
    let mut observer = Observe::default();
    let status = execution
        .resume(&mut TerminalFuelMeter::unbounded(), &mut observer)
        .unwrap();
    (status, observer)
}

fn normal_guarantee_source(body: &str) -> String {
    format!("machine identity(value: i32) -> i32 ensures result == value {{ value }}\n{}", source()
        .replace("Scalar::measure() -> i32 reaches Host", "Scalar::measure(value: i32) -> i32\nrequires value >= 1\nensures result == value\nreaches Host")
        .replace("let result: i32 = Host::measure(70);\n            result", body)
        .replace("Scalar::measure();", "Scalar::measure(70);"))
}

fn boolean_guarantee_source(body: &str, input: bool) -> String {
    // Retype the fixture declarations before inserting the customer's body;
    // an authored i32 literal inside that body must keep its integer suffix.
    normal_guarantee_source("BODY_PLACEHOLDER")
        .replace("i32", "bool")
        .replace("requires value >= 1", "requires value == value")
        .replace("Scalar::measure(70)", &format!("Scalar::measure({input})"))
        .replace("BODY_PLACEHOLDER", body)
}

fn nested_boolean_guarantee_source(expression: &str, before: &str, inputs: [bool; 4]) -> String {
    let [value, spare, other, last] = inputs;
    boolean_guarantee_source(&format!("{before} {expression}"), value)
        .replace(
            "Scalar::measure(value: bool)",
            "Scalar::measure(marker: u16, spare: bool, value: bool, other: bool, last: bool)",
        )
        .replace(
            &format!("Scalar::measure({value})"),
            &format!("Scalar::measure(9u16, {spare}, {value}, {other}, {last})"),
        )
        .replace(
            "ensures result == value\nreaches Host",
            &format!("ensures result == ({expression})\nreaches Host"),
        )
}

fn assert_exact_normal_return_evidence(source: &str) {
    let artifact = artifact(&checked_from_source(source));
    let module = decode_module(&artifact.0).unwrap();
    let proof = decode_proof_bundle(&artifact.1).unwrap();
    let wrapper = module
        .machines
        .iter()
        .find(|machine| {
            !machine.contract.ensures.is_empty() && !machine.contract.requires.is_empty()
        })
        .unwrap();
    let obligation = wrapper.contract.ensures[0].obligation;
    let mut missing = proof.clone();
    missing
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert_eq!(missing.evidence.len() + 1, proof.evidence.len());
    assert!(
        terminal_verifier::verify_module(&module, &missing, &AdmissionProfile::default()).is_err()
    );

    let mut changed = module.clone();
    let wrapper = changed
        .machines
        .iter_mut()
        .find(|machine| machine.id == wrapper.id)
        .unwrap();
    let block = wrapper
        .blocks
        .iter_mut()
        .find(|block| matches!(block.terminator, terminal_psi::Terminator::Return { .. }))
        .unwrap();
    let opaque_result = block
        .operations
        .iter()
        .find_map(|operation| match (&operation.kind, &operation.result) {
            (
                terminal_psi::OperationKind::BoundaryCall { .. },
                terminal_psi::OperationResult::Scalar(result),
            ) => Some(result.id),
            _ => None,
        })
        .unwrap();
    let terminal_psi::Terminator::Return { value, .. } = &mut block.terminator else {
        unreachable!()
    };
    assert_ne!(*value, opaque_result);
    *value = opaque_result;
    // The test handler returns its input, but an opaque boundary promises no
    // such relation. Neither its carrier nor a previous proof grants equality.
    assert!(
        terminal_verifier::verify_module(&changed, &proof, &AdmissionProfile::default()).is_err()
    );
}

fn ordered_contract_source() -> String {
    source()
        .replace("Scalar::measure() -> i32 reaches Host", "Scalar::measure(value: i32 [1..=100], other: i32) -> i32\nrequires value >= 1\nrequires other >= 2\nrequires value >= 1\nreaches Host")
        .replace("let result: i32 = Host::measure(70);\n            result", "Host::finish(other); Host::measure(value)")
        .replace("Scalar::measure();", "Scalar::measure(70, 11);")
}
