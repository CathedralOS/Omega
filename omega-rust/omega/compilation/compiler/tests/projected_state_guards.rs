use std::path::{Path, PathBuf};

use terminal_codec::CanonicalTerminalArtifact;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInstantiation, TerminalProgram,
};
use terminal_work::WorkMeter;

fn fixture(kind: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .join("tests/fixtures")
        .join(kind)
        .join("contracts")
        .join(name)
        .join("main.omg")
}

#[test]
fn a_state_guard_binds_the_incoming_field_until_that_field_is_written() {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            // The state changes a sibling before indexing with the guarded
            // field, then overwrites the field and indexes with a fresh read.
            // The frame contents show that both indices reached their stores.
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(
                    &fixture("run", "projected_state_guard"),
                    Some("macos_arm64"),
                ),
            )
            .result
            .unwrap_or_else(|failure| panic!("{:?}", failure.into_diagnostics()));
            let accepted = checked.terminal_publication().accepted().unwrap();
            let published = accepted.closure().publish().unwrap();
            let reopened = CanonicalTerminalArtifact::from_bytes(&published.artifact.to_bytes())
                .expect("reopen the guarded loop as Psi");
            let program = TerminalProgram::borrow(reopened.view());
            let mut execution = program
                .instantiate(TerminalInstantiation::default())
                .unwrap();
            assert_eq!(
                execution.invoke(&mut WorkMeter::unbounded()).unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );

            let diagnostics = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(
                    &fixture("fail", "projected_state_guard_overwritten"),
                    Some("macos_arm64"),
                ),
            )
            .result
            .expect_err("writing the guarded field invalidates the index bound")
            .into_diagnostics();
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("cannot prove index is within length 12")),
                "{diagnostics:?}"
            );
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
}
