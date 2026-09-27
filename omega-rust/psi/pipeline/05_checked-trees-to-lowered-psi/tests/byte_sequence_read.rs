use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalScalarValue, interpret_terminal_artifact_measured,
};

const SOURCE: &str = r#"
    boundary trait Output {
        machine flag(value: bool) reaches Output;
    }
    data Helper {}
    machine Helper::inspect(bytes: &[u8]) reaches Output {
        Output::flag(bytes.len > 0 && bytes[0] == 128);
    }
    data Root {}
    machine Root::enter() reaches Output {
        Helper::inspect("\x80A");
        Helper::inspect("");
        Helper::inspect("\x00\xFF\x7F");
    }
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    check(source).expect("check")
}

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<String>> {
    crate::front_end::checked_program_result(source).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect()
    })
}

#[derive(Default)]
struct Flags(Vec<bool>);

impl TerminalEffectHandler for Flags {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("expected flag boundary");
        };
        let [TerminalScalarValue::Boolean(value)] = arguments.as_slice() else {
            panic!("expected Boolean flag");
        };
        self.0.push(*value);
        Ok(())
    }
}

fn execute(source: &str) -> Vec<bool> {
    let checked = checked(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("guarded read has an executable helper closure");
    let expected = execute_lowered(&lowered);
    let selections = optimization::PsiOptimizationSelections::new([
        optimization::PsiOptimization::DeadPureScalarElimination,
    ])
    .unwrap();
    let optimized = lowered_psi_to_lowered_psi::run_psi_optimization(lowered, selections)
        .expect("dead scalar elimination preserves read operands and bounds evidence");
    assert_eq!(execute_lowered(optimized.lowered()), expected);
    expected
}

fn execute_lowered(lowered: &lowered_psi::LoweredPsi) -> Vec<bool> {
    let mut flags = Flags::default();
    let result = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut flags,
    )
    .unwrap();
    assert_eq!(result.value(), TerminalExecutionResult::Unit);
    flags.0
}

#[test]
fn sibling_branches_keep_their_own_length_observations() {
    assert_eq!(
        execute(&SOURCE.replace(
            "bytes.len > 0 && bytes[0] == 128",
            "(bytes.len > 0 && bytes[0] == 128) || (bytes.len > 0 && bytes[0] == 0)",
        )),
        [true, false, true],
    );
}
