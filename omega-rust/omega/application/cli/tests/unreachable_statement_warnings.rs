/*

End-to-end CLI check for the advisory warning about statements written after a
call that never returns (`omega --check`).

`wiki/spec/language/state_contracts.md`, "Statements after a call that never
returns": the program is accepted, and the checker issues "one advisory
warning at the first unreachable executable statement, with a note locating
the call that prevents continuation, not one per following statement". We run
`omega --check` on the kept program, whose `main` calls a machine that crashes
on its only path and then holds two more statements, and read `stderr`: the
check succeeds, there is one warning, and the note after it is placed at the
call.

*/

use std::path::PathBuf;
use std::process::Command;

#[test]
fn statements_after_a_call_that_never_returns_warn_once_and_compile() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../../../tests/fixtures/pass/contracts/statement_after_never_returning_call_exit",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("main.omg")
        .current_dir(&directory)
        .output()
        .expect("omega runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "the program compiles: {stderr}");
    let lines: Vec<&str> = stderr.lines().collect();
    let warnings: Vec<usize> = (0..lines.len())
        .filter(|line| lines[*line].starts_with("warning: "))
        .collect();
    let [warning] = warnings[..] else {
        panic!("one warning for two unreachable statements: {stderr}");
    };
    assert!(
        lines[warning].starts_with(
            "warning: unreachable statement in machine `main` state `entry`: the call before it never returns"
        ),
        "{stderr}"
    );
    // The first unreachable statement is on line 9 and the call on line 8.
    assert!(lines[warning + 1].contains("main.omg:9:"), "{stderr}");
    assert_eq!(lines[warning + 2], "note: this call never returns");
    assert!(lines[warning + 3].ends_with("main.omg:8:5"), "{stderr}");
}
