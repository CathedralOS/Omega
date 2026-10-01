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
fn qualified_whole_byte_parameter_lends_a_shared_boundary_view() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8; 128]::Utf8 requires valid_utf8(self);
        boundary trait Output { machine write(bytes: &[u8]) reaches Output; }
        data Formatter {}
        data Renderer { formatter: Formatter; }
        machine Formatter::replace(&mut self, line: &mut [u8; 128] in Utf8)
        ensures line in Utf8 { line = "room"; }
        machine Renderer::render(&mut self, line: &mut [u8; 128] in Utf8) reaches Output
        ensures line in Utf8 {
            self.formatter.replace(line);
            Output::write(line);
            self.formatter.replace(line);
            Output::write(line);
        }
        "#,
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Renderer::render"),
    )
    .expect("qualified whole storage lends its current live bytes to the boundary");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("shared observations preserve the owner's predicate across replacements");
}

#[test]
fn whole_byte_boundary_views_keep_the_sources_read_authority() {
    for source_access in ["", "&", "&mut "] {
        let checked = crate::front_end::checked_program(&format!(
            r#"
            domain [u8; 128]::Utf8 requires valid_utf8(self);
            boundary trait Output {{ machine write(bytes: &[u8]) reaches Output; }}
            machine render(line: {source_access}[u8; 128] in Utf8) reaches Output {{
                Output::write(line);
            }}
            "#,
        ));
        let lowered = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("render"),
        )
        .expect("each readable whole owner can lend a shared live view");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("the read-only boundary does not weaken the owner's invariant");

        // Check semantic rejection directly, not an obsolete proof digest.
        let mut unreadable = lowered.semantic_module.clone();
        let entry = unreadable.entry;
        unreadable
            .machines
            .iter_mut()
            .find(|machine| machine.id == entry)
            .unwrap()
            .structural_parameters[0]
            .access = terminal_psi::StructuralAccess::WriteOnlyBorrow;
        assert!(
            matches!(
                terminal_verifier::validate_module(&unreadable),
                Err(terminal_verifier::ModuleError::StructuralArgumentAccessExceedsSource { .. })
            ),
            "write-only authority cannot supply boundary content"
        );

        let mut exclusive = lowered.semantic_module.clone();
        let entry = exclusive.entry;
        let caller = exclusive
            .machines
            .iter_mut()
            .find(|machine| machine.id == entry)
            .unwrap();
        let argument = caller
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                OperationKind::BoundaryCall {
                    structural_arguments,
                    ..
                } => structural_arguments.first_mut(),
                _ => None,
            })
            .unwrap();
        argument.access = terminal_psi::StructuralAccess::MutableBorrow;
        assert!(
            terminal_verifier::validate_module(&exclusive).is_err(),
            "a read-only presentation cannot be substituted with a mutable loan"
        );
    }
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
