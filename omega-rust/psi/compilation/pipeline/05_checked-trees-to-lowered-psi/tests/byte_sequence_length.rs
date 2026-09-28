use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalScalarValue, interpret_terminal_artifact_measured,
};

const SOURCE: &str = r#"
    boundary trait Output {
        machine size(length: u64) reaches Output;
    }
    data Helper {}
    machine Helper::measure(bytes: &[u8]) reaches Output {
        Output::size(bytes.len);
    }
    data Root {}
    machine Root::enter() reaches Output {
        Helper::measure("\x80A");
        Helper::measure("");
        Helper::measure("\x00\xFF\x7F");
    }
"#;

fn lower(source: &str) -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(source);
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("byte length has an executable Unit helper closure")
}

#[derive(Default)]
struct LengthEffects(Vec<u128>);

impl TerminalEffectHandler for LengthEffects {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("expected length boundary");
        };
        let [
            TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(length),
            },
        ] = arguments.as_slice()
        else {
            panic!("expected unsigned byte length");
        };
        assert_eq!(
            *scalar_type,
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
        );
        self.0.push(*length);
        Ok(())
    }
}

fn execute(source: &str) -> Vec<u128> {
    let lowered = lower(source);
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let evidence = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
    assert_eq!(
        decode_proof_bundle(&evidence).unwrap(),
        lowered.proof_bundle
    );
    let mut effects = LengthEffects::default();
    let result = interpret_terminal_artifact_measured(
        &semantic,
        &evidence,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut effects,
    )
    .unwrap();
    assert_eq!(result.value(), TerminalExecutionResult::Unit);
    effects.0
}

#[test]
fn length_participates_in_exact_arithmetic_without_becoming_a_constant() {
    assert_eq!(
        execute(&SOURCE.replace("Output::size(bytes.len);", "Output::size(bytes.len + 0);")),
        vec![2, 0, 3]
    );
}
