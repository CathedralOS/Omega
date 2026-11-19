/*

End-to-end CLI check that a Build can wait for a build dependency it
registered to be prepared (`omega --check`).

`suspend Build::prepare_dependency(builder, registered);` asks the build
evaluator to make one registered dependency ready before the Build goes on
(`wiki/spec/build/scoped_execution.md`, "Dependency declarations and
binding"). The Build parks at that call. The evaluator acquires the
dependency, runs its Build, prepares its source and resumes the Build where
it stopped; nothing before the call runs again.

`registered` is the value `Build::build_dependency` returned. The evaluator
issued it, so it knows which registration to prepare. A
`DependencyRegistration {}` written in source has the same type and names no
registration.

Each program below is the same project with one line of its Build changed.
The first prepares the dependency and compiles. The second prepares a
dependency whose own Build declares two project roles: the refusal names that
dependency, which shows the evaluator really ran its Build at the wait. The
third hands over a registration it wrote itself and is refused for that.

*/

use std::process::Command;

/// `omega --check` on a project whose Build registers the build dependency
/// in `dep/` as `kit`, then runs `after`. The dependency's Build is `dep`.
fn check(name: &str, after: &str, dep: &str) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-dependency-preparation-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("dep")).expect("staging directory");
    std::fs::write(root.join("main.omg"), "machine main() { }\n").expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        format!(
            "machine build(builder: &mut Build)
suspends
{{
    Build::application(builder, \"prepares\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    let mut request: DependencyDescription = dependency::from_source(Source::Path {{ location: \"dep\" }});
    dependency::with_alias(&mut request, \"kit\");
    let registered: DependencyRegistration = Build::build_dependency(builder, request);
{after}}}
"
        ),
    )
    .expect("staged Build");
    std::fs::write(root.join("dep/build.omg"), dep).expect("staged dependency Build");
    std::fs::write(
        root.join("dep/main.omg"),
        "public machine v() -> i32 { 7 }\n",
    )
    .expect("staged dependency source");
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

const PACKAGE: &str = "machine build(builder: &mut Build) {
    Build::package(builder, \"kit_package\");
}
";

#[test]
fn a_build_waits_for_its_registered_dependency_and_goes_on() {
    let (compiled, stderr) = check(
        "prepared",
        "    suspend Build::prepare_dependency(builder, registered);
    BuildLog::write_line(&mut builder.log, \"after preparation\");
",
        PACKAGE,
    );
    assert!(compiled, "{stderr}");
}

#[test]
fn the_wait_runs_the_build_of_the_dependency_it_names() {
    let (compiled, stderr) = check(
        "two-roles",
        "    suspend Build::prepare_dependency(builder, registered);
",
        "machine build(builder: &mut Build) {
    Build::package(builder, \"kit_package\");
    Build::package(builder, \"second_role\");
}
",
    );
    assert!(!compiled);
    assert!(stderr.contains("project role"), "{stderr}");
}

#[test]
fn a_registration_the_build_wrote_itself_names_no_dependency() {
    let (compiled, stderr) = check(
        "forged",
        "    let forged: DependencyRegistration = DependencyRegistration {};
    suspend Build::prepare_dependency(builder, forged);
",
        PACKAGE,
    );
    assert!(!compiled);
    assert!(
        stderr.contains("requires a registration this activation issued"),
        "{stderr}"
    );
}
