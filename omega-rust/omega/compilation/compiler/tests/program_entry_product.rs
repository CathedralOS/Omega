//! The selected Build entry joins the actual accepted publication, rather than
//! selecting an ambient `main` or publishing a second judgment.
#[test]
fn selected_entry_receipt_keeps_the_accepted_artifact_and_rejects_other_owners() {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(4)
                .unwrap();
            let main =
                repository.join("tests/fixtures/run/calls/scalar_contract_route_exit/main.omg");
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .unwrap_or_else(|failure| {
                panic!("default scalar check: {:?}", failure.into_diagnostics())
            });
            let accepted = checked.terminal_publication().accepted().unwrap();
            let program = checked.terminal_publication().published().unwrap();
            let module = program.artifact().semantic_module();
            assert_eq!(module, accepted.module());
            assert!(std::sync::Arc::ptr_eq(
                accepted.source(),
                checked.source_inspection().program(),
            ));
            let evidence = program.evidence().expect("actual discharged evidence");
            assert!(!evidence.seals.is_empty());
            assert_eq!(evidence.module, program.artifact().manifest().semantic());

            let (signature, application) = compiler::prepare_program_entry(&checked).unwrap();
            let receipt = application.receipt.unwrap().expect("actual entry receipt");
            assert_eq!(
                receipt.source_signature_identity(),
                signature.identity().bytes()
            );
            assert_eq!(
                receipt.terminal_psi_identity(),
                program.artifact().manifest().semantic()
            );
            assert_eq!(application.roots.len(), 1);
            assert_eq!(
                application.roots[0].as_ref().unwrap().machine,
                receipt.terminal_entry()
            );
            assert_eq!(
                signature.target_slot().owner,
                target::TargetProfile::from_canonical_target_name("linux_x86_64").unwrap(),
            );
            let root = terminal_psi::PreparedNominalRoot::new(
                signature.declaration(),
                "diagnostic name is not authority",
                None,
            )
            .unwrap();
            assert_eq!(root.resolve(module).unwrap().id, receipt.terminal_entry());
            let wrong_owner =
                terminal_psi::PreparedNominalRoot::new("other-package::main", "main", None)
                    .unwrap();
            assert!(
                terminal_codec::entry_receipt::prepare_program_entry_receipt(
                    &wrong_owner,
                    signature.identity().bytes(),
                    program.artifact(),
                )
                .is_err()
            );

            // A portable body with the same entry ID is insufficient when its
            // declaration or arrival signature is absent.
            let mut malformed = module.clone();
            malformed
                .machines
                .iter_mut()
                .find(|machine| machine.id == receipt.terminal_entry())
                .unwrap()
                .declaration = None;
            assert!(
                program_entry_plan::SelectedProgramEntrySignature::from_module_entry(
                    signature.target_slot(),
                    &malformed,
                    receipt.terminal_entry(),
                    "main",
                )
                .unwrap_err()
                .contains("source declaration")
            );
        })
        .unwrap()
        .join()
        .unwrap();
    });
}
