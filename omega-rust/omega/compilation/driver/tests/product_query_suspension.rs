//! Product queries pause the real Terminal Build before a description exists.

use compiler::{CheckedCompileRequest, CompilationSession};

#[test]
fn admitted_build_helpers_keep_their_lexical_product_scope() {
    use package_compilation::{
        BuildDeclarationKind, PackageCompilationInputs, PackageDependencyBinding,
    };
    use semantic_vocabulary::PackageKeyIdentity;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap();
    for fixture in [
        "product_entry_query_foreign_helper_private_entry",
        "product_entry_query_build_scope_alias",
    ] {
        let root = repository.join("tests/omega/fail/build").join(fixture);
        let owner = PackageKeyIdentity::from_digest([91; 32]).unwrap();
        let helper = PackageKeyIdentity::from_digest([92; 32]).unwrap();
        // Explicit pre-admitted inputs exercise lexical visibility independently
        // of the separate discovery task for pre-execution helper imports.
        let inputs = PackageCompilationInputs::new(
            owner,
            BuildDeclarationKind::Application,
            vec![
                captured_package(owner, fixture, &root),
                captured_package(helper, "query_helper", &root.join("helper")),
            ],
            vec![PackageDependencyBinding::for_purpose(
                owner,
                "support",
                helper,
                build_declarations::DependencyPurpose::Build,
            )],
        )
        .unwrap();
        let mut request = CheckedCompileRequest::new(&root.join("main.omg"), None);
        request.package_inputs = Some(inputs);
        let mut invocation = CompilationSession::default()
            .begin_package_build(request)
            .unwrap_or_else(|failure| panic!("{fixture}: {:?}", failure.into_diagnostics()));
        let mut queries = invocation.product_queries();
        let mut host = invocation.execution().unwrap().host().unwrap();
        let mut activation = host.start().unwrap();
        let pending = loop {
            let step = activation.advance(1024).unwrap();
            assert!(!step.returned, "{fixture}: query must execute");
            if let Some(query) = step.awaiting_query {
                break query;
            }
        };
        let diagnostics = queries.describe(&pending, Vec::new()).unwrap_err();
        let expected = std::fs::read_to_string(root.join("expected.txt")).unwrap();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected.trim())),
            "{fixture}: {diagnostics:?}"
        );
    }
}

fn captured_package(
    identity: semantic_vocabulary::PackageKeyIdentity,
    name: &str,
    root: &std::path::Path,
) -> package_compilation::PackageSourceBinding {
    let capture = package_source::local::operations::capture_local_source_input(
        root,
        package_source::LocalSourceLimits::default(),
    )
    .unwrap();
    package_compilation::PackageSourceBinding::new(
        identity,
        name,
        capture.canonical_live_root().to_path_buf(),
    )
    .with_captured_source(capture.tree().clone())
    .unwrap()
}

#[test]
fn authored_product_query_retains_lexical_occurrence_and_pauses() {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate belongs to the repository");
    let root = repository.join("tests/omega/pass/build/described_runner_entry_left_exit/main.omg");
    let mut session = CompilationSession::default();
    let mut invocation = session
        .begin_package_build(CheckedCompileRequest::new(&root, None))
        .expect("admit authored Build without completing Product sources");
    let execution = invocation.execution().expect("authored Build execution");
    let mut host = execution.host().expect("Terminal Build host");
    let mut activation = host.start().expect("one Build activation");
    let request = loop {
        let step = activation.advance(1024).expect("execute Build to query");
        assert!(!step.returned, "query must suspend before Build returns");
        assert!(step.awaiting_provider.is_none());
        if let Some(request) = step.awaiting_query {
            break request;
        }
    };
    assert_eq!(request.path, b"left::run");
    assert_eq!(
        request.slot.as_deref(),
        Some(b"macos_arm64::ProgramEntry".as_slice())
    );
    assert_eq!(
        request.operation,
        psi_compiler::TerminalBuildQueryOperation::Entry
    );
    let source = std::fs::read_to_string(root.with_file_name("build.omg")).unwrap();
    let occurrence = request.occurrence.span;
    let authored = &source[occurrence.start as usize..occurrence.end as usize];
    assert!(
        authored.contains("entry"),
        "query occurrence {:?} names {authored:?}",
        request.occurrence
    );
    let issued = activation.requests().len();
    for _ in 0..3 {
        let step = activation
            .advance(1024)
            .expect("unanswered query stays paused");
        assert!(!step.returned);
        let repeated = step.awaiting_query.expect("same pending query");
        assert!(std::sync::Arc::ptr_eq(&request, &repeated));
        assert_eq!(activation.requests().len(), issued, "no Build replay");
    }
    assert!(
        activation.complete().is_err(),
        "an unanswered query is not a normal return"
    );
}
