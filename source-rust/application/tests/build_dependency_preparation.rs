//! A Build can wait for a dependency before it finishes defining its Product.
//! The evaluator runs the selected package's Build and resumes the parked
//! caller at the wait. Preparing an unissued registration must fail there.

#[path = "support/project.rs"]
mod project;

/// Run a Build whose application entry is bound only after the wait.
fn check(name: &str, after: &str, dep: &str) -> (bool, String) {
    let build = format!(
        "use main.omg as app;
machine build(plan: @mutable BuildPlan) suspends {{
    let registered: DependencyRegistration = Build::dependency(plan, Source::Path {{ location: \"dep\" }});
{after}
    plan.product = BuildProduct::Application {{ name: \"prepares\" }};
    Build::bind(plan, linux_x86_64::ProgramEntry, |app|::main);
}}
"
    );
    project::check(
        name,
        &[
            ("main.omg", "public machine main() { }\n"),
            ("build.omg", &build),
            ("dep/build.omg", dep),
            ("dep/main.omg", "public machine v() -> i32 { 7 }\n"),
        ],
        &["run", "build.omg"],
    )
}

const PACKAGE: &str = "machine build(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::Package { name: \"kit_package\" };
}
";
const WAIT: &str = "    suspend Build::prepare_dependency(plan, registered);";

#[test]
fn a_build_waits_for_its_registered_dependency_and_goes_on() {
    let (ran, stderr) = check("prepared", WAIT, PACKAGE);
    assert!(ran, "{stderr}");
}

#[test]
fn the_wait_runs_the_build_of_the_dependency_it_names() {
    let (ran, stderr) = check(
        "failing-child",
        WAIT,
        "machine build(plan: @mutable BuildPlan) crashes Trap { crash Trap; }\n",
    );
    assert!(!ran);
    assert!(
        stderr.contains("while preparing the dependency"),
        "{stderr}"
    );
    assert!(stderr.contains("Trap"), "{stderr}");
}

#[test]
fn a_registration_the_build_wrote_itself_names_no_dependency() {
    let (ran, stderr) = check(
        "forged",
        "    let forged: DependencyRegistration = DependencyRegistration {};
    suspend Build::prepare_dependency(plan, forged);",
        PACKAGE,
    );
    assert!(!ran);
    assert!(
        stderr.contains("requires a registration this activation issued"),
        "{stderr}"
    );
}
