/*

End-to-end CLI check that the build evaluator owes a Build entry's `requires`
clauses before it runs the Build (`omega --check`).

A machine's `requires` is owed by its caller. A Build entry has no caller in
source: the build evaluator starts it. The evaluator hands the entry a Build
it promises nothing about, so the only clauses it can owe are ones true of
the evaluator itself. The one programs write is the address width they were
written for, `requires addr::Bound == 18446744073709551616`.

We stage a project whose entry states one clause and run `omega --check`. A
clause that is true of the evaluator lets the Build run. A clause that is
false stops it before its first statement: the twin states the bound of a
32-bit address. A clause over the Build argument is refused by name, since
the evaluator establishes nothing about it.

*/

use std::process::Command;

fn check(name: &str, clause: &str) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-entry-precondition-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("staging directory");
    std::fs::write(root.join("main.omg"), "machine main() { }\n").expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        format!(
            "machine build(builder: &mut Build) requires {clause} {{
    Build::application(builder, \"conditioned\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
}}
"
        ),
    )
    .expect("staged Build");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("main.omg")
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    let _ = std::fs::remove_dir_all(&root);
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn a_build_entry_may_require_the_evaluators_address_width() {
    let (compiled, stderr) = check("width", "addr::Bound == 18446744073709551616");
    assert!(compiled, "{stderr}");
}

#[test]
fn a_build_entry_requirement_false_of_the_evaluator_stops_the_build() {
    for (name, clause) in [
        ("narrow", "addr::Bound == 4294967296"),
        ("false", "1u64 == 2u64"),
    ] {
        let (compiled, stderr) = check(name, clause);
        assert!(!compiled, "{clause}");
        assert!(
            stderr.contains("the build entry's `requires` does not hold for this build evaluator"),
            "{stderr}"
        );
    }
}

#[test]
fn a_build_entry_requirement_over_the_build_is_refused_by_name() {
    let (compiled, stderr) = check("argument", "builder.freestanding == false");
    assert!(!compiled);
    assert!(
        stderr.contains("a build entry `requires` clause over its arguments"),
        "{stderr}"
    );
}
