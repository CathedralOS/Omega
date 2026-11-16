//! Product queries pause the real Terminal Build before a description exists.

use compiler::{CheckedCompileRequest, CompilationSession};

#[test]
fn schema_and_named_provider_queries_resume_the_original_build() {
    use build_settings::host::bindings::BuildProductDescription;
    use source_compiler::TerminalBuildQueryOperation;
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap();
    let root = repository.join("tests/fixtures/pass/build/product_description_paths_exit/main.omg");
    let mut session = CompilationSession::default();
    let mut invocation = session
        .begin_package_build(CheckedCompileRequest::new(&root, None))
        .unwrap_or_else(|failure| panic!("{:?}", failure.into_diagnostics()));
    let mut queries = invocation.product_queries();
    let host = invocation.execution().unwrap().host().unwrap();
    let mut activation = host.start().unwrap();
    let mut queried = Vec::new();
    loop {
        let step = activation.advance(1024).unwrap();
        if step.returned {
            break;
        }
        assert!(step.awaiting_provider.is_none());
        let Some(request) = step.awaiting_query else {
            continue;
        };
        assert!(request.slot.is_none());
        // Selecting a locator never coerces a different declaration kind.
        let mut wrong_role = request.as_ref().clone();
        wrong_role.path = match request.operation {
            TerminalBuildQueryOperation::Schema => b"payload::Provider".to_vec(),
            TerminalBuildQueryOperation::Provider => b"payload::Packet".to_vec(),
            _ => panic!("unexpected query"),
        };
        let rejected = queries.describe(&wrong_role, Vec::new()).unwrap_err();
        assert!(
            rejected
                .iter()
                .any(|diagnostic| diagnostic.message.contains("names no")),
            "{rejected:?}"
        );
        let answer = queries.describe(&request, Vec::new()).unwrap();
        let repeated = queries.describe(&request, Vec::new()).unwrap();
        match (&answer, repeated) {
            (
                BuildProductDescription::Schema(schema),
                BuildProductDescription::Schema(repeated),
            ) => {
                assert_eq!(request.operation, TerminalBuildQueryOperation::Schema);
                assert_eq!(schema.canonical_path, "payload::Packet");
                assert_eq!(
                    *schema, repeated,
                    "query context retains the exact declaration"
                );
            }
            (
                BuildProductDescription::Provider(provider),
                BuildProductDescription::Provider(repeated),
            ) => {
                assert_eq!(request.operation, TerminalBuildQueryOperation::Provider);
                assert_eq!(provider.canonical_path, "payload::Provider");
                assert_eq!(
                    *provider, repeated,
                    "query context retains the named implementation"
                );
            }
            _ => panic!("unexpected query role: {answer:?}"),
        }
        queried.push(request.operation);
        activation.answer_query(&request, answer, &[]).unwrap();
    }
    assert_eq!(
        queried,
        [
            TerminalBuildQueryOperation::Schema,
            TerminalBuildQueryOperation::Provider
        ]
    );
    // The fixture performs each path inspection twice, plus two ordinary path
    // calls that increment a counter. Return proves its exact values and count;
    // only the two declaration queries suspend for source work.
    activation.complete().unwrap();
}

#[test]
fn admitted_build_helpers_keep_their_lexical_product_scope() {
    let repository = fixture_package_inputs::repo_root();
    for fixture in [
        "product_entry_query_foreign_helper_private_entry",
        "product_entry_query_build_scope_alias",
    ] {
        let root = repository.join("tests/fixtures/fail/build").join(fixture);
        // The fixture suspends while its Build dependency is prepared. The
        // ordinary scheduler serves that pause and resumes the same activation
        // before checking the query's original helper/package visibility.
        let Err(diagnostics) =
            fixture_package_inputs::discover_fixture(&root.join("main.omg"), Default::default())
        else {
            panic!("{fixture}: the Product query must be refused");
        };
        let expected = std::fs::read_to_string(root.join("expected.txt")).unwrap();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected.trim())),
            "{fixture}: {diagnostics:?}"
        );
    }
}

#[test]
fn authored_product_query_retains_lexical_occurrence_and_pauses() {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate belongs to the repository");
    let root =
        repository.join("tests/fixtures/pass/build/described_runner_entry_left_exit/main.omg");
    let mut session = CompilationSession::default();
    let mut invocation = session
        .begin_package_build(CheckedCompileRequest::new(&root, None))
        .expect("admit authored Build without completing Product sources");
    let execution = invocation.execution().expect("authored Build execution");
    let host = execution.host().expect("Terminal Build host");
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
        source_compiler::TerminalBuildQueryOperation::Entry
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

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

#[test]
fn typed_product_descriptions_use_the_named_capture_and_reject_another_role() {
    let repository = fixture_package_inputs::repo_root();
    let positive =
        repository.join("tests/fixtures/run/reflection/build_parameter_description/main.omg");
    // Discovery runs the actual Build and its text/position oracle under the
    // acquired package's lineage. No fixture package key substitutes for it.
    fixture_package_inputs::discover_fixture(&positive, Default::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    let negative =
        repository.join("tests/fixtures/fail/reflection/build_parameter_wrong_kind/main.omg");
    let Err(diagnostics) = fixture_package_inputs::discover_fixture(&negative, Default::default())
    else {
        panic!("a field cannot satisfy a formal query");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("names no formal in")),
        "{diagnostics:?}"
    );
}
