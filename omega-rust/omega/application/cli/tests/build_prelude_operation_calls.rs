/*

End-to-end CLI check that a Build may call the operations the toolchain's
Build prelude declares as machines (`omega --check`).

`Build::exclude_crash(builder, cause)` and the Product queries
(`BuildProduct::entry(&builder.product, path, slot)`) are declared prelude
machines, so name resolution answers the author's call with that
declaration. Typing then spells the call of the Build protocol that carries
the request. The author's selection and the generated call name different
declarations, and both are right: the selection record keeps what the author
named, and the generated call is not that call.

We stage a project whose Build makes one such call and run `omega --check`.
What the project is stopped by afterwards belongs to the Product admissions
of those requests, which are separate work; these tests hold that the Build
itself is typed, judged and run.

*/

use std::process::Command;

fn check(name: &str, build: &str) -> String {
    let root = std::env::temp_dir().join(format!(
        "omega-build-prelude-operation-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("staging directory");
    std::fs::write(root.join("main.omg"), "machine main() { }\n").expect("staged program");
    std::fs::write(root.join("build.omg"), build).expect("staged Build");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("main.omg")
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    let _ = std::fs::remove_dir_all(&root);
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The Build ran: nothing was refused while its source was discovered, and
/// neither of the two refusals a replaced prelude operation used to meet is
/// present.
fn build_ran(stderr: &str) {
    for refusal in [
        "project source discovery failed",
        "rewritten call contradicts its original selection record",
        "call private machine of another package",
    ] {
        assert!(!stderr.contains(refusal), "{stderr}");
    }
}

#[test]
fn a_build_that_excludes_a_crash_cause_runs() {
    build_ran(&check(
        "exclude-crash",
        "machine build(builder: &mut Build) {
    Build::application(builder, \"excluding\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    Build::exclude_crash(builder, CrashCause::Trap);
}
",
    ));
}

/// The same request made by a helper machine the Build forwards its `Build`
/// to, with the cause passed as a value.
#[test]
fn a_helper_machine_that_excludes_a_crash_cause_runs() {
    build_ran(&check(
        "exclude-helper",
        "machine build(builder: &mut Build) {
    Build::application(builder, \"excluding\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    exclude(builder, CrashCause::Abort);
}
machine exclude(builder: &mut Build, cause: CrashCause) {
    Build::exclude_crash(builder, cause);
}
",
    ));
}

/// A Product query in a value position: the entry a root binding is given.
#[test]
fn a_build_that_queries_a_product_entry_is_not_refused_for_its_selection() {
    let stderr = check(
        "query-entry",
        "machine build(builder: &mut Build) {
    Build::application(builder, \"querying\");
    Build::bind(builder, linux_x86_64::ProgramEntry, entry(builder, \"linux_x86_64::ProgramEntry\"));
}
machine entry(builder: &mut Build, slot: &[u8]) -> ProductEntryRef {
    BuildProduct::entry(&builder.product, \"main\", slot)
}
",
    );
    assert!(
        !stderr.contains("rewritten call contradicts its original selection record"),
        "{stderr}"
    );
}
