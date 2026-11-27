/*

End-to-end CLI check that a Build may lend the host a value the host issued
(`omega --check`).

`BuildOutput::require` hands the Build a `RequiredOutput`: a marker for an
output file the Build has promised to write. The Build host knows that marker
only by the identity it gave out. The Build keeps the marker in a local of
its own and later passes it back, sometimes by value (`BuildOutput::fail`)
and sometimes on loan: `RequiredOutput::path(@obligation)` asks where the
promised file goes, and `BuildOutput::complete` lends the marker to the host's
readiness check before it gives it up.

The first program does both loans and must compile: it asks for the path,
writes the file there and completes the output. The second lends a
`RequiredOutput {}` it wrote itself. That has the right type and was never
issued, so the host must still refuse it: lending is no way around the check.

*/

#[path = "support/project.rs"]
mod project;

fn check(name: &str, body: &str) -> (bool, String) {
    let build = format!(
        "use main.omg as app;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: \"promised\" }};
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
{body}}}
machine settle(builder: @mutable BuildPlan, settled: OutputCompletion) {{
    transition settled {{
        OutputCompletion::Sealed {{ receipt }} -> done(),
        OutputCompletion::Retry {{ obligation, file }} -> failed(builder, obligation)
    }}
    machine done() {{}}
    machine failed(builder: @mutable BuildPlan, obligation: RequiredOutput) {{
        BuildOutput::fail(@mutable builder.output, obligation, \"output was not sealed\");
    }}
}}
"
    );
    project::check(
        name,
        &[("main.omg", "machine main() { }\n"), ("build.omg", &build)],
        &["--check", "main.omg", "--target", "linux_x86_64"],
    )
}

#[test]
fn a_build_lends_the_host_the_required_output_it_was_issued() {
    let (compiled, stderr) = check(
        "issued",
        "    let obligation: RequiredOutput = BuildOutput::require(@mutable builder.output, \"answer.dat\");
    let file: BuildPath = BuildOutput::resolve(@builder.output, RequiredOutput::path(@obligation));
    let writer: i32 = BuildOutput::create(@mutable builder.output, file, 0);
    _ = BuildOutput::write(@mutable builder.output, writer, \"forty-two\");
    _ = BuildOutput::close(@mutable builder.output, writer);
    let settled: OutputCompletion = BuildOutput::complete(@mutable builder.output, obligation, file);
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
    let file: BuildPath = BuildOutput::resolve(@builder.output, RequiredOutput::path(@forged));
",
    );
    assert!(!compiled);
    assert!(
        stderr.contains("Build filesystem value was not issued by this activation")
            && stderr.contains("build.omg:6:")
            && !stderr.contains("ExecutionSite")
            && !stderr.contains("EffectRejected"),
        "{stderr}"
    );
}
