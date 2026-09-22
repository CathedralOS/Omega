// TOP-LEVEL-BOUNDARY-REQUIREMENTS end-to-end: `CellSource::read<'a>` is a
// public boundary requirement carrying an erased lifetime telescope. The
// telescope supplies no static call arguments, conformance validates the
// provider's own telescope as the requirement's positional identity, and
// settlement keys the same owner-keyed dispatch row a nongeneric
// requirement produces. Both executable engines run the selected adapter
// under that one plan -- the interpreter exits 41 and the native artifact
// exits 41.
use crate::{compile_reviewed_repository_fixture, interpret, pass_canary};
// Only the non-Windows leg produces and runs the native artifact.
#[cfg(test)]
use crate::compile_terminal_canary_without_output_for_target;
#[cfg(not(windows))]
use crate::{Command, compile_rooted_canary_for_native_host, fs};
use compiler::CheckedCompileRequest;

fn lifetime_call_canary() -> std::path::PathBuf {
    pass_canary("providers/lifetime_boundary_requirement_dispatch_exit")
}

#[test]
fn lifetime_call_requirement_executes_in_the_checked_interpreter() {
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &lifetime_call_canary().join("main.omg"),
        None,
    ))
    .expect("lifetime-call requirement fixture should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.error, None);
    assert_eq!(
        outcome.exit_code, 41,
        "the value-position lifetime call must execute the selected adapter"
    );
}

#[test]
fn lifetime_call_requirement_reaches_terminal_production() {
    compile_terminal_canary_without_output_for_target(&lifetime_call_canary(), "linux_x86_64")
        .expect("lifetime-call requirement fixture should produce a Terminal artifact");
}

#[cfg(not(windows))]
#[test]
fn lifetime_call_requirement_executes_in_the_native_artifact() {
    let scratch = std::env::temp_dir().join(format!(
        "omega-lifetime-call-requirement-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation =
        compile_rooted_canary_for_native_host(&lifetime_call_canary(), scratch.join("out"))
            .expect("lifetime-call requirement fixture should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("lifetime-call requirement fixture should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("lifetime-call requirement native artifact should run");
    assert_eq!(
        output.status.code(),
        Some(41),
        "native dispatches the adapter selected for the erased lifetime telescope"
    );
    let _ = fs::remove_dir_all(&scratch);
}
