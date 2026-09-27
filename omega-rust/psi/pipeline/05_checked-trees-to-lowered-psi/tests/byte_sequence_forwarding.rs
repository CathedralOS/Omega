use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_psi::OperationKind;

const SOURCE: &str = r#"
    boundary trait Output {
        machine write(bytes: &[u8]) reaches Output;
    }
    data Helper {}
    machine Helper::write(bytes: &[u8]) reaches Output {
        Output::write(bytes);
    }
    data Root {}
    machine Root::enter() reaches Output {
        Helper::write("\x80A");
    }
"#;

fn lower(source: &str) -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(source);
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("borrowed bytes have an ordinary helper call plan")
}

fn assert_rejected(lowered: &lowered_psi::LoweredPsi) {
    assert!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "malformed byte transfer must reject"
    );
}

#[test]
fn missing_or_late_literal_establishment_rejects() {
    for missing in [true, false] {
        let mut lowered = lower(SOURCE);
        let root = lowered
            .semantic_module
            .machines
            .iter_mut()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        assert!(matches!(
            root.blocks[0].operations[0].kind,
            OperationKind::EstablishByteSequenceLiteral { .. }
        ));
        if missing {
            root.blocks[0].operations.remove(0);
        } else {
            root.blocks[0].operations.swap(0, 1);
        }
        assert_rejected(&lowered);
    }
}
