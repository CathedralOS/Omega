//! A Build can configure its first group after its dependency is ready and a second group exists.
use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Stdio};

const BUILD: &str = r#"machine build(builder: @mutable Build) suspends {
    Build::application(builder, "group_settings");
    Build::bind(builder, linux_x86_64\ProgramEntry, main);
    let muted: TestGroup = builder.tests.group::<Checks::muted>();
    muted.disabled = false;
    let dependency = Build::build_dependency(builder, dependency\from_source(Source::Path { location: "kit" }));
    suspend Build::prepare_dependency(builder, dependency);
    let live = builder.tests.group::<Checks::live>();
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
    let package = TemporaryDirectory::create("omega-group-settings", false).unwrap();
    let root = package.path();
    let kit = root.join("kit");
    std::fs::create_dir(&kit).unwrap();
    std::fs::write(
        kit.join("build.omg"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "kit"); }"#,
    )
    .unwrap();
    std::fs::write(
        kit.join("main.omg"),
        "public machine version() -> u32 { 1u32 }",
    )
    .unwrap();
    std::fs::write(root.join("main.omg"), product).unwrap();
    std::fs::write(root.join("build.omg"), configuration).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["build", "--target", "linux_x86_64", "build.omg"])
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .unwrap();
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
