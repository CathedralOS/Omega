// TOP-LEVEL-BOUNDARY-REQUIREMENTS end-to-end: `Token::consume(self)` is a
// public nongeneric boundary requirement whose only parameter is an owned
// `self` receiver. Calling it through a member receiver settles a
// receiver-keyed forwarding row, rewrites the call to the selected checked
// adapter, and both executable engines run the adapter under that one
// selected plan -- the interpreter exits 41 and the native artifact exits 41.
// The projected fixture calls it through `self.inner.token`, a receiver
// place deeper than one projection hop: settle registration re-derives each
// intermediate member's exact field symbol inside its owner, the rewrite
// reifies the whole place as the adapter's leading argument, and the checked
// interpreter executes the selected adapter in both value and statement
// position.
use crate::{compile_reviewed_repository_fixture, interpret, repo_root};
// Only the non-Windows leg produces and runs the native artifact.
#[cfg(not(windows))]
use crate::{Command, compile_rooted_canary_for_native_host, fs};
use compiler::CheckedCompileRequest;

fn member_call_fixture() -> std::path::PathBuf {
    repo_root().join("tests/fixtures/boundary-requirement-member-call")
}

fn projected_member_call_fixture() -> std::path::PathBuf {
    repo_root().join("tests/fixtures/boundary-requirement-member-call-projected")
}

#[test]
fn member_call_requirement_executes_in_the_checked_interpreter() {
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &member_call_fixture().join("main.omg"),
        None,
    ))
    .expect("member-call requirement fixture should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.error, None);
    assert_eq!(
        outcome.exit_code, 41,
        "the value-position member call must execute the selected adapter"
    );
}

#[test]
fn projected_member_call_requirement_executes_in_the_checked_interpreter() {
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &projected_member_call_fixture().join("main.omg"),
        None,
    ))
    .expect("projected member-call requirement fixture should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.error, None);
    assert_eq!(
        outcome.exit_code, 41,
        "the deeper projected member call must execute the selected adapter"
    );
}

#[cfg(not(windows))]
#[test]
fn member_call_requirement_executes_in_the_native_artifact() {
    let scratch = std::env::temp_dir().join(format!(
        "omega-member-call-requirement-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation =
        compile_rooted_canary_for_native_host(&member_call_fixture(), scratch.join("out"))
            .expect("member-call requirement fixture should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("member-call requirement fixture should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("member-call requirement native artifact should run");
    assert_eq!(
        output.status.code(),
        Some(41),
        "native dispatches the adapter through the forwarded receiver"
    );
    let _ = fs::remove_dir_all(&scratch);
}
