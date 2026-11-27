//! A Build can configure its first group after its dependency is ready and a second group exists.
#[path = "support/project.rs"]
mod project;

const BUILD: &str = r#"use main.omg as app;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "group_settings" };
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    let muted: TestGroup = builder.tests.group::<|app|::Checks::muted>();
    muted.disabled = false;
    let dependency: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "kit" }));
    suspend Build::prepare_dependency(builder, dependency);
    let live: TestGroup = builder.tests.group::<|app|::Checks::live>();
    live.disabled = true;
    muted.disabled = 2u32 + 3u32 == 5u32;
    live.disabled = !live.disabled;
}"#;

const PRODUCT: &str = r#"trait Checks {
    machine muted() crashes Trap;
    machine live() crashes Trap;
}
machine would_crash() satisfies Checks::muted crashes Trap { crash Trap; }
machine still_runs() satisfies Checks::live crashes Trap {}
machine main() {}
"#;

fn build(product: &str, configuration: &str) -> (bool, String) {
    let package = project::stage(
        "group-settings",
        &[
            (
                "kit/build.omg",
                r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "kit" }; }"#,
            ),
            ("kit/main.omg", "public machine version() -> u32 { 1u32 }"),
            ("main.omg", product),
            ("build.omg", configuration),
        ],
    );
    let result = project::omega(
        package.path(),
        &["build", "--target", "linux_x86_64", "build.omg"],
    );
    (
        result.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        ),
    )
}

#[test]
fn independent_groups_keep_their_last_evaluated_settings() {
    let (success, output) = build(PRODUCT, BUILD);
    assert!(success, "{output}");
    assert!(
        output.contains("group `Checks::muted` (target `linux_x86_64`): disabled"),
        "{output}"
    );
    assert!(
        output.contains(
            "test `still_runs` in group `Checks::live` (target `linux_x86_64`): passed on Psi"
        ),
        "{output}"
    );

    let (success, output) = build(PRODUCT, &BUILD.replace("== 5u32", "== 6u32"));
    assert!(!success, "{output}");
    assert!(output.contains("test `would_crash` in group `Checks::muted` failed on the Psi interpreter engine: crashed with Trap"), "{output}");
}

#[test]
fn a_disabled_group_still_checks_its_test_source() {
    let (success, output) = build(&PRODUCT.replace("crash Trap;", "absent_machine();"), BUILD);
    assert!(!success, "{output}");
    assert!(
        output.contains("this call has no visible declaration in its source scope"),
        "{output}"
    );
}

#[test]
fn trait_arguments_select_the_exact_satisfaction_edges() {
    let product = r#"data ScanFixture { value: u8; }
data WriteFixture { value: u8; }
trait Checks<T> { machine memory() crashes Trap; }
machine scan() satisfies Checks<ScanFixture>::memory crashes Trap {}
machine write() satisfies Checks<WriteFixture>::memory crashes Trap { crash Trap; }
machine main() {}"#;
    let configuration = r#"use main.omg as app;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "applied_groups" };
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    _ = builder.tests.group::<|app|::Checks<|app|::ScanFixture>::memory>();
    let writes: TestGroup = builder.tests.group::<|app|::Checks<|app|::WriteFixture>::memory>();
    writes.disabled = true;
}"#;
    let (success, output) = build(product, configuration);
    assert!(success, "{output}");
    assert!(
        output.contains("test `scan` in group `Checks<ScanFixture>::memory`"),
        "{output}"
    );
    assert!(
        output.contains("group `Checks<WriteFixture>::memory` (target `linux_x86_64`): disabled"),
        "{output}"
    );

    let (success, output) = build(
        product,
        &configuration.replace("writes.disabled = true;", ""),
    );
    assert!(!success, "{output}");
    assert!(
        output.contains("test `write` in group `Checks<WriteFixture>::memory`"),
        "{output}"
    );
    assert!(output.contains("crashed with Trap"), "{output}");
    assert!(
        !output.contains("test `write` in group `Checks<ScanFixture>::memory`"),
        "{output}"
    );

    let (success, output) = build(
        product,
        &configuration.replace("Checks<WriteFixture>", "Checks<ScanFixture>"),
    );
    assert!(!success, "{output}");
    assert!(
        output.contains("test group registration repeats the same requirement"),
        "{output}"
    );
}

#[test]
fn run_stops_before_the_entry_when_a_group_requires_a_native_test() {
    let configuration = r#"use main.omg as app;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "required_native" };
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    let group: TestGroup = builder.tests.group::<|app|::Checks::live>();
    TestGroup::engine(group, linux_x86_64, TestEngine::Native, TestEnginePolicy::Required);
}"#;
    let package = project::stage(
        "run-required-native-test",
        &[("main.omg", PRODUCT), ("build.omg", configuration)],
    );
    let (success, output) = project::status(&project::omega(
        package.path(),
        &[
            "run",
            "--target",
            "linux_x86_64",
            "--outputs",
            "observations",
            "build.omg",
        ],
    ));
    assert!(!success, "{output}");
    assert!(
        output.contains("use `omega build` for required native test execution"),
        "{output}"
    );
    assert!(output.contains("passed on Psi interpreter"), "{output}");
    assert!(!package.path().join("observations").exists());
}
