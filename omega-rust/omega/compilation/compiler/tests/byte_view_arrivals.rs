//! Indexed reads and length observations share the exact stored or reborrowed
//! byte descriptor after the published program is reopened.

use std::path::{Path, PathBuf};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInstantiation, TerminalProgram,
};
use terminal_work::WorkMeter;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .join("tests/fixtures")
}

#[test]
fn reopened_byte_views_read_the_original_backing_after_arrival() {
    for fixture in [
        "mutable_byte_view_arrival",
        "mutable_byte_view_short_circuit",
    ] {
        let root = fixtures().join("run/views").join(fixture).join("main.omg");
        let checked = orchestration::check_source(
            &mut compiler::CompilationSession::default(),
            compiler::CheckedCompileRequest::new(&root, Some("macos_arm64")),
        )
        .result
        .unwrap_or_else(|failure| panic!("{fixture}: {:?}", failure.into_diagnostics()));
        let closure = checked.terminal_publication().accepted().unwrap().closure();
        let published = closure.publish().unwrap();
        let reopened = CanonicalTerminalArtifact::from_bytes(&published.artifact.to_bytes())
            .expect("reopen the checked program through the artifact reader");
        let program = TerminalProgram::borrow(reopened.view());
        let mut execution = program
            .instantiate(TerminalInstantiation::default())
            .unwrap();
        assert_eq!(
            execution.invoke(&mut WorkMeter::unbounded()).unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit),
            "{fixture} must reach the successful byte-equality branch"
        );
    }
}

#[test]
fn arrival_index_equal_to_exact_view_length_refuses() {
    let root = fixtures().join("fail/views/mutable_byte_view_arrival_bounds/main.omg");
    let diagnostics = orchestration::check_source(
        &mut compiler::CompilationSession::default(),
        compiler::CheckedCompileRequest::new(&root, Some("macos_arm64")),
    )
    .result
    .expect_err("index two exceeds the same two-byte descriptor")
    .into_diagnostics();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove operation safety in machine `inspect`")
            && diagnostic
                .message
                .contains("the bounds do not decide the comparison")),
        "{diagnostics:?}"
    );
}
