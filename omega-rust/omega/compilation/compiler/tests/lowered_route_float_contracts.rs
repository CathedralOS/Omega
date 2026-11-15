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

#[test]
fn ieee_self_equality_needs_an_explicit_premise() {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let project = platform_custody::temporary_directory::TemporaryDirectory::create(
                "ieee-self-equality",
                false,
            )
            .unwrap();
            let build = repository().join("tests/fixtures/run/float/range_delivery/build.omg");
            std::fs::copy(build, project.path().join("build.omg")).unwrap();
            let main = project.path().join("main.omg");
            let check = |premise| {
                std::fs::write(
                    &main,
                    format!(
                        "machine need(value: f32) requires value == value {{ }}\n\
                         public machine probe(value: f32) requires {premise} {{ need(value); }}\n\
                         machine main() {{ probe(0.25f32); }}\n"
                    ),
                )
                .unwrap();
                orchestration::check_source(
                    &mut compiler::CompilationSession::default(),
                    compiler::CheckedCompileRequest::new(&main, Some("macos_arm64")),
                )
                .result
            };
            let checked = check("value == value")
                .unwrap_or_else(|failure| panic!("{:?}", failure.into_diagnostics()));
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

            let diagnostics = check("true")
                .expect_err("an unconstrained f32 may be NaN")
                .into_diagnostics();
            let [diagnostic] = diagnostics.as_slice() else {
                panic!("expected one refusal at the call: {diagnostics:?}");
            };
            assert!(
                diagnostic.message.starts_with(
                    "cannot prove requires contract for call need from probe: Float::equal(value, value)"
                ),
                "{diagnostic:?}"
            );
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
}
