/*

End-to-end CLI check that a Build may lend the host a value the host issued
(`omega --check`).

`BuildOutput::require` hands the Build a `RequiredOutput`: a marker for an
output file the Build has promised to write. The Build host knows that marker
only by the identity it gave out. The Build keeps the marker in a local of
its own and later passes it back, sometimes by value (`BuildOutput::fail`)
and sometimes on loan: `RequiredOutput::path(&obligation)` asks where the
promised file goes, and `BuildOutput::complete` lends the marker to the host's
readiness check before it gives it up.

The first program does both loans and must compile: it asks for the path,
writes the file there and completes the output. The second lends a
`RequiredOutput {}` it wrote itself. That has the right type and was never
issued, so the host must still refuse it: lending is no way around the check.

*/

use std::process::Command;

fn check(name: &str, body: &str) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-required-output-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("staging directory");
    std::fs::write(root.join("main.omg"), "machine main() { }\n").expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        format!(
            "machine build(builder: &mut Build) {{
    Build::application(builder, \"promised\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
{body}}}
machine settle(builder: &mut Build, settled: OutputCompletion) {{
    transition settled {{
        OutputCompletion::Sealed {{ receipt }} -> done()
        OutputCompletion::Retry {{ obligation, file }} -> failed(builder, obligation)
    }}
    machine done() {{}}
    machine failed(builder: &mut Build, obligation: RequiredOutput) {{
        BuildOutput::fail(&mut builder.output, obligation, \"output was not sealed\");
    }}
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
fn a_build_lends_the_host_the_required_output_it_was_issued() {
    let (compiled, stderr) = check(
        "issued",
        "    let obligation: RequiredOutput = BuildOutput::require(&mut builder.output, \"answer.dat\");
    let file: BuildPath = BuildOutput::resolve(&builder.output, RequiredOutput::path(&obligation));
    let writer: i32 = BuildOutput::create(&mut builder.output, file, 0);
    _ = BuildOutput::write(&mut builder.output, writer, \"forty-two\");
    _ = BuildOutput::close(&mut builder.output, writer);
    let settled: OutputCompletion = BuildOutput::complete(&mut builder.output, obligation, file);
    settle(builder, settled);
",
    );
    assert!(compiled, "{stderr}");
}

#[test]
fn a_required_output_the_host_never_issued_is_refused_on_loan() {
    let (compiled, stderr) = check(
        "forged",
        "    let forged: RequiredOutput = RequiredOutput {};
    let file: BuildPath = BuildOutput::resolve(&builder.output, RequiredOutput::path(&forged));
",
    );
    assert!(!compiled);
    assert!(
        stderr.contains("Build filesystem value was not issued by this activation"),
        "{stderr}"
    );
}
