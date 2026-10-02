use crate::TerminalMachineSelection;
use crate::machine_lowering::lower_machine;

#[test]
fn scalar_return_preserves_erased_arguments_around_borrowed_input() {
    let checked = crate::front_end::checked_program(
        r#"
        data Proof { tag: i32; }
        data Inner { value: u8; }
        data Main { inner: Inner; result: u8; }
        machine first(values: &Inner) -> u8 { values.value }
        machine Main::relay(witness [erased]: Proof, values: &Inner, trailing [erased]: Proof) -> u8 {
            first(values)
        }
        machine Main::caller(main: &mut Main) {
            main.inner.value = 70;
            main.result = Main::relay(Proof { tag: 1 }, &main.inner, Proof { tag: 2 });
        }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::caller"))
        .expect("lower erased scalar return closure");
    let callee = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.contract.erased_proof_formals.len() == 2)
        .expect("relay retains both proof declarations");
    assert_eq!(callee.structural_parameters.len(), 1);
    assert!(callee.parameters.is_empty());

    let mut missing = lowered.semantic_module.clone();
    missing
        .machines
        .iter_mut()
        .find(|machine| machine.id == callee.id)
        .expect("same relay declaration")
        .contract
        .erased_proof_formals
        .clear();
    let error = terminal_verifier::validate_module(&missing)
        .expect_err("independent validation rejects a dropped erased declaration");
    assert!(format!("{error:?}").contains("ErasedProofArgumentArityMismatch"));
}

#[test]
fn direct_computation_and_effect_prefixes_preserve_both_erased_lanes() {
    for body in [
        "values.value",
        "first(witness, values, trailing)",
        "observe(values); first(witness, values, trailing)",
    ] {
        let source = format!(
            r#"
            data Proof {{ tag: i32; }}
            data Inner {{ value: u8; }}
            data Main {{ inner: Inner; result: u8; }}
            machine first(witness [erased]: Proof, values: &Inner, trailing [erased]: Proof) -> u8 {{ values.value }}
            machine observe(values: &Inner) {{}}
            machine Main::relay(
                witness [erased]: Proof,
                bound [erased]: u64,
                values: &Inner,
                trailing [erased]: Proof
            ) -> u8 {{ {body} }}
            machine Main::caller(main: &mut Main) {{
                main.inner.value = 70;
                main.result = Main::relay(Proof {{ tag: 1 }}, 7, &main.inner, Proof {{ tag: 2 }});
            }}
            "#,
        );
        let checked = crate::front_end::checked_program(&source);
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::caller"))
            .unwrap_or_else(|error| panic!("erased roster for {body}: {error:?}"));
        let callee = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| {
                machine.contract.erased_proof_formals.len() == 2
                    && machine.contract.erased_scalar_formals.len() == 1
            })
            .expect("relay retains proof declarations through every prefix");
        assert_eq!(callee.contract.erased_scalar_formals.len(), 1);
        assert_eq!(callee.structural_parameters.len(), 1);
        assert!(callee.parameters.is_empty());
    }
}
