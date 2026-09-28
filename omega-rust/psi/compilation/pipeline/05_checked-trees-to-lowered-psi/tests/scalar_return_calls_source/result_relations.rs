use super::checked_arms;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm};
use terminal_psi::OperationKind;

#[test]
fn embedded_calls_use_proved_parameter_relative_results() {
    for body in [
        "(n * 2) as i32",
        "transition { _ -> ((n * 2) as i32) }",
        "self.count = self.count + 1; (n * 2) as i32",
    ] {
        for input in [6, i32::MAX] {
            let source = format!(
                r#"
                data Calc {{ count: i32 in Wrapping; }}
                data Main {{ calc: Calc; base: i32 in Wrapping; }}
                machine Calc::double_val(&mut self, n: i32 in Wrapping) -> i32 {{ {body} }}
                machine Main::value(&mut self) -> i32 {{
                    let result: i32 in Wrapping = self.base + self.calc.double_val({input}) * 3;
                    result as i32
                }}
            "#
            );
            let checked = checked_arms(&source, false);
            let lowered = checked_trees_to_lowered_psi::lower_machine(
                &checked,
                TerminalMachineSelection::Name("Main::value"),
            )
            .unwrap_or_else(|error| panic!("input={input}, body={body}: {error:?}"));
            terminal_verifier::verify_module(
                &lowered.semantic_module,
                &lowered.proof_bundle,
                &proof_admission::AdmissionProfile::default(),
            )
            .expect("parameter-relative result facts must independently verify");
            let callee = lowered
                .semantic_module
                .machines
                .iter()
                .find(|machine| {
                    machine.contract.ensures.iter().any(|clause| {
                        matches!(
                            clause.proposition,
                            Proposition::Equal(_, ScalarTerm::WrappingIntegerMultiply { .. })
                        )
                    })
                })
                .expect("callee retains exact wrapping result meaning");
            let guarantee = callee.contract.ensures.last().unwrap().obligation;
            let mut missing = lowered.proof_bundle.clone();
            missing
                .evidence
                .retain(|proof| proof.obligation != guarantee);
            assert!(
                terminal_verifier::verify_module(
                    &lowered.semantic_module,
                    &missing,
                    &proof_admission::AdmissionProfile::default(),
                )
                .is_err(),
                "a generated contract is not evidence for itself"
            );
            let mut changed = lowered.semantic_module.clone();
            let changed_callee = changed
                .machines
                .iter_mut()
                .find(|machine| machine.id == callee.id)
                .unwrap();
            let literal = changed_callee
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| {
                    if let OperationKind::IntegerConstant { value } = &mut operation.kind
                        && *value == IntegerValue::Signed(2)
                    {
                        Some(value)
                    } else {
                        None
                    }
                })
                .unwrap();
            *literal = IntegerValue::Signed(3);
            assert!(
                terminal_verifier::verify_module(
                    &changed,
                    &lowered.proof_bundle,
                    &proof_admission::AdmissionProfile::default(),
                )
                .is_err(),
                "changed arithmetic cannot retain the original guarantee proof"
            );
        }
    }
}
