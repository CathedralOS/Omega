//! A complete input contract transports the stored float range through a local.
//! The negative removes only its upper bound, so private precondition inference
//! cannot supply the missing fact from the concrete callers.

fn repository() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .to_owned()
}

#[test]
fn float_range_delivery_uses_the_stored_bound_and_refuses_its_missing_premise() {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            for source in [
                "samples/cli/facts/float_range_delivery/local/main.omg",
                "tests/fixtures/run/float/range_delivery/main.omg",
            ] {
                let main = repository().join(source);
                let checked = orchestration::check_source(
                    &mut compiler::CompilationSession::default(),
                    compiler::CheckedCompileRequest::new(&main, Some("macos_arm64")),
                )
                .result
                .unwrap_or_else(|failure| panic!("{source}: {:?}", failure.into_diagnostics()));
                let publication = checked
                    .terminal_publication()
                    .accepted()
                    .unwrap()
                    .closure()
                    .publish()
                    .unwrap();
                terminal_codec::ClosedTerminalModule::from_semantic_bytes(
                    publication.artifact.semantic_bytes(),
                )
                .unwrap();
            }
            let main = repository().join("tests/fixtures/fail/float/range_delivery/main.omg");
            let diagnostics = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(&main, Some("macos_arm64")),
            )
            .result
            .expect_err("the complete input contract lacks the upper bound")
            .into_diagnostics();
            let diagnostic = format!("{diagnostics:?}");
            assert!(diagnostic.contains("cannot prove requires"), "{diagnostic}");
            assert!(diagnostic.contains("0.5"), "{diagnostic}");
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
}
