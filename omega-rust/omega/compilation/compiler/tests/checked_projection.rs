//! Product checking retains the declaration environment for name resolution
//! and judges the subject and its semantic closure, rather than pruning typed
//! trees (`wiki/spec/build/validation_subjects.md`). The accepted module and
//! evidence are the product's checking result; publication and the Build's
//! selected entry must retain that same result
//! (`wiki/spec/terminal-psi/product.md`, "Optimization ordering").
fn accepted_fixture() -> compiler::CheckedCompilation {
    let main = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate is under omega-rust/omega/compilation/compiler")
        .join("tests/fixtures/run/calls/scalar_contract_route_exit/main.omg");
    // Discovery and checking recurse deeply; use the compiler worker's stack.
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            )
            .result
            .unwrap_or_else(|failure| {
                panic!(
                    "the scalar fixture must be accepted: {:?}",
                    failure.into_diagnostics()
                )
            })
        })
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}

#[test]
fn the_product_publishes_its_accepted_module_and_selected_entry() {
    let checked = accepted_fixture();
    let publication = checked.terminal_publication();
    let accepted = publication.accepted().expect("actual accepted source");
    let program = publication.published().expect("actual published product");
    let module = program.artifact().semantic_module();

    // The retained source and request own this judgment. The source catalog's
    // unused declarations do not stand for checked executable dependencies.
    assert!(std::sync::Arc::ptr_eq(
        accepted.source(),
        checked.source_inspection().program(),
    ));
    let roster = publication.roster().expect("the judged request");
    assert_eq!(roster.entries, checked.build().config.program_roots());
    assert_eq!(
        roster.subject,
        checked
            .sources()
            .package_subject()
            .map(|subject| subject.root())
    );
    assert_eq!(module, accepted.module());
    assert!(
        accepted
            .boundary_requirements_declared()
            .expect("accepted boundary correspondence")
            .is_empty(),
        "this scalar program reaches no boundary requirement",
    );
    let evidence = program.evidence().expect("actual discharged evidence");
    assert!(
        !evidence.seals.is_empty(),
        "the fixture owes checked arithmetic"
    );
    assert_eq!(evidence.module, program.artifact().manifest().semantic());

    // The executed Build selects the entry. Its receipt joins that source
    // signature to this artifact, independently of diagnostic spelling.
    let (signature, application) =
        compiler::prepare_program_entry(&checked).expect("the selected entry's producer");
    let receipt = application
        .receipt
        .expect("entry preparation")
        .expect("the executable entry's receipt");
    assert_eq!(
        receipt.source_signature_identity(),
        signature.identity().bytes()
    );
    assert_eq!(
        receipt.terminal_psi_identity(),
        program.artifact().manifest().semantic()
    );
    let [root] = application.roots.as_slice() else {
        panic!("the Build selects one entry for this target");
    };
    assert_eq!(
        root.as_ref().expect("selected root").machine,
        receipt.terminal_entry()
    );
    let selected = terminal_psi::PreparedNominalRoot::new(
        signature.declaration(),
        "diagnostic text is not entry authority",
        None,
    )
    .expect("the selected entry's exact source owner");
    assert_eq!(
        selected.resolve(module).unwrap().id,
        receipt.terminal_entry()
    );
    let wrong_owner = terminal_psi::PreparedNominalRoot::new(
        &format!("other-package::{}", signature.declaration()),
        "diagnostic text is not entry authority",
        None,
    )
    .unwrap();
    assert!(
        terminal_codec::entry_receipt::prepare_program_entry_receipt(
            &wrong_owner,
            signature.identity().bytes(),
            program.artifact(),
        )
        .is_err(),
        "the same entry leaf from another owner cannot join this artifact",
    );
    assert!(
        publication.for_other_roots().accepted().is_err(),
        "another root selection needs its own judgment",
    );
}
