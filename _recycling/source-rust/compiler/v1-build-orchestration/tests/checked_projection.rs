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
        .expect("orchestration crate is under omega-rust/omega/build-system/orchestration")
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

use products::BuildConfigExt as _;

#[test]
fn the_product_publishes_its_accepted_module_and_selected_entry() {
    let checked = accepted_fixture();
    let publication = checked.terminal_publication();
    let accepted = publication.accepted().expect("actual accepted source");
    let program = publication
        .require_published(publication.optimizations())
        .expect("actual published product");
    let artifact = std::sync::Arc::clone(program.artifact());
    let module = artifact.semantic_module();

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
    assert_eq!(evidence.module, artifact.manifest().semantic());

    // The executed Build selects the entry. Its receipt joins that source
    // signature to this artifact, independently of diagnostic spelling.
    let source_owner = std::sync::Arc::downgrade(checked.source_inspection().program());
    let build_owner = checked
        .build()
        .accepted_build
        .as_ref()
        .map(|accepted| std::sync::Arc::downgrade(accepted.source()));
    let source_program = std::sync::Arc::downgrade(&program);
    let published = checked.into_published().expect("closed executable input");
    drop(program);
    assert!(
        source_program.upgrade().is_none(),
        "executable input retains no source publication object"
    );
    assert!(std::sync::Arc::ptr_eq(
        published.program().artifact(),
        &artifact
    ));
    assert!(
        source_owner.upgrade().is_none(),
        "executable input retains no Product source"
    );
    assert!(
        build_owner.is_none_or(|owner| owner.upgrade().is_none()),
        "executable input retains no Build source"
    );
    let (signature, application) =
        compiler::prepare_program_entry(&published).expect("the selected entry's producer");
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
        artifact.manifest().semantic()
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
    // An independently callable application checker must join the receipt to
    // the same artifact occurrence as its roots, even for identical code in
    // a fresh invocation namespace.
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::for_invocation();
    for identity in module.identities.entries() {
        identities.intern(identity);
    }
    let mut invocation_module = module.clone();
    invocation_module.identities = identities.build();
    let invocation = terminal_codec::InvocationTerminalCode::new(invocation_module)
        .expect("the same code can be held by a fresh invocation");
    let custody = terminal_installation::application::TerminalApplicationCustody::prepare(
        Ok(selected),
        Some(signature.identity().bytes()),
        Some(terminal_installation::application::TerminalCompositionRoots::default()),
    );
    let refusal = custody
        .attach(
            invocation.view(),
            Ok(&artifact),
            published.program().machine_provenance(),
            &mut terminal_installation::application::UnmeasuredEntryReceipt,
        )
        .expect_err("roots from another invocation cannot join this receipt");
    assert_eq!(
        refusal[0].message,
        "application roots and receipt belong to different artifacts or invocations"
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
            &artifact,
        )
        .is_err(),
        "the same entry leaf from another owner cannot join this artifact",
    );
}
