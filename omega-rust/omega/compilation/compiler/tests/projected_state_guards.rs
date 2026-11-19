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

#[test]
fn a_callee_clause_reads_the_state_field_its_caller_did_not_name() {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            // The row state names only `grid.rows`; the column loop it enters
            // also requires `x <= grid.columns`. The arrival needs the value
            // the row state's formal holds at each entry, not a constant.
            let checked = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(
                    &fixture("run", "unnamed_state_field_clause"),
                    Some("macos_arm64"),
                ),
            )
            .result
            .unwrap_or_else(|failure| panic!("{:?}", failure.into_diagnostics()));
            let accepted = checked.terminal_publication().accepted().unwrap();
            let published = accepted.closure().publish().unwrap();
            let reopened = CanonicalTerminalArtifact::from_bytes(&published.artifact.to_bytes())
                .expect("reopen the nested loop as Psi");
            let program = TerminalProgram::borrow(reopened.view());
            let mut execution = program
                .instantiate(TerminalInstantiation::default())
                .unwrap();
            assert_eq!(
                execution.invoke(&mut WorkMeter::unbounded()).unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );

            // Entering with x = 1 needs `1 <= grid.columns`. The snapshot
            // names the field's entry value without knowing it is 3.
            let diagnostics = orchestration::check_source(
                &mut compiler::CompilationSession::default(),
                compiler::CheckedCompileRequest::new(
                    &fixture("fail", "unnamed_state_field_clause_exceeded"),
                    Some("macos_arm64"),
                ),
            )
            .result
            .expect_err("the row state knows no lower bound on grid.columns")
            .into_diagnostics();
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("cannot prove requires contract for call column from row")
                    && diagnostic
                        .message
                        .contains("the bounds do not decide the comparison")),
                "{diagnostics:?}"
            );
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
}
