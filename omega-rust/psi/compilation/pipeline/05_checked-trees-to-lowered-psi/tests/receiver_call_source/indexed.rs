//! Fixed-array receiver paths retain their referent across the portable boundary.

use super::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralValue,
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
