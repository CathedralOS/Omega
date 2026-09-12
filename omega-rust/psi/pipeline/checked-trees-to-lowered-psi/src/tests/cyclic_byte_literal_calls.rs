//! Published cyclic Unit calls preserve literal contents through resumable execution.

use super::checked_source;
mod mixed_and_custody;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalStructuralValue,
};

#[test]
fn cyclic_literal_calls_preserve_selected_bytes_at_every_fuel_pause() {
    let checked = checked_source(
        r#"
boundary trait Trace { machine write(bytes: &[u8]) reaches Trace; }
machine forward(first: &[u8], second: &[u8]) reaches Trace {
    Trace::write(first);
    Trace::write(second);
}
data Main { counter: u64 in Wrapping; }
machine Main::main(&mut self) reaches Trace {
    self.counter = 0;
    transition { _ -> head() }
    state head(&mut self) {
        transition self.counter < 3 { true -> choose() _ -> done() }
    }
    state choose(&mut self) {
        transition self.counter == 0 { true -> first() _ -> later() }
    }
    state first(&mut self) {
        forward("first", "");
        transition { _ -> advance() }
    }
    state later(&mut self) {
        forward("later", "λ");
        transition { _ -> advance() }
    }
    state advance(&mut self) {
        Trace::write("tick");
        self.counter = self.counter + 1;
        transition { _ -> head() }
    }
    state done(&mut self) { Trace::write("done"); }
}
"#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
        .produce_artifact()
        .expect("cyclic literal calls publish the portable Terminal artifact");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert!(
        entry.ranked_scc.is_none(),
        "no manufactured termination certificate"
    );
    let [receiver] = entry.structural_parameters.as_slice() else {
        panic!("persistent receiver survives publication")
    };
    assert_eq!(
        receiver.access,
        terminal_psi::StructuralAccess::MutableBorrow
    );
    let execute = || {
        TerminalExecution::start_artifact_with_structural_arguments(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            &[TerminalStructuralValue {
                opaque_identity: 1,
                structural_type: receiver.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
        )
        .expect("canonical artifact reloads and independently verifies")
    };
    let expected = [
        "first", "", "tick", "later", "λ", "tick", "later", "λ", "tick", "done",
    ]
    .map(|text| text.as_bytes().to_vec())
    .to_vec();
    let complete = TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit);
    let mut execution = execute();
    let mut meter = TerminalFuelMeter::with_allowance(1000);
    let mut trace = ByteTrace::default();
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut meter, &mut trace)
            .unwrap(),
        complete
    );
    assert_eq!(trace.0, expected);
    let total = meter.usage().total_units();
    for allowance in 0..total {
        let mut execution = execute();
        let mut meter = TerminalFuelMeter::with_allowance(allowance);
        let mut trace = ByteTrace::default();
        assert!(matches!(
            execution
                .resume_with_effect_handler(&mut meter, &mut trace)
                .unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        let prefix = trace.0.clone();
        let usage = meter.usage().clone();
        for _ in 0..3 {
            assert!(matches!(
                execution
                    .resume_with_effect_handler(&mut meter, &mut trace)
                    .unwrap(),
                TerminalExecutionStatus::SponsorExhausted(_)
            ));
            assert_eq!(
                trace.0, prefix,
                "zero-fuel resume repeats no effect at {allowance}"
            );
            assert_eq!(meter.usage(), &usage);
        }
        meter.replenish(total - allowance).unwrap();
        assert_eq!(
            execution
                .resume_with_effect_handler(&mut meter, &mut trace)
                .unwrap(),
            complete
        );
        assert_eq!(
            trace.0, expected,
            "exact selected contents/order at fuel split {allowance}"
        );
        assert_eq!(meter.usage().total_units(), total);
    }
}

#[derive(Default)]
struct ByteTrace(Vec<Vec<u8>>);

impl TerminalEffectHandler for ByteTrace {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            byte_sequence_arguments,
            ..
        } = effect
        else {
            panic!("only authored byte trace boundaries are observable")
        };
        assert!(arguments.is_empty());
        assert_eq!(structural_arguments.len(), 1);
        let [Some(bytes)] = byte_sequence_arguments.as_slice() else {
            panic!("boundary retains exact immutable byte contents")
        };
        self.0.push(bytes.clone());
        Ok(())
    }
}
