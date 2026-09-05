use super::{BUILD, assert_status, generated_fixture};
use crate::authority::authority_fixture;

const LOG_MARKER: &str = "package build execution marker";

#[test]
fn io_derived_dependency_rejects_before_any_build_logging() {
    discovery_precedes_build_io(
        "builder.depend(Source::Path { location: &input_bytes });",
        "dependency source field `location` must be a direct string literal",
    );
}

#[test]
fn missing_transitive_dependency_rejects_before_any_build_logging() {
    discovery_precedes_build_io(
        "builder.depend(Source::Path { location: \"../missing-package\" });",
        "missing-package",
    );
}

fn discovery_precedes_build_io(declaration: &str, expected: &str) {
    let fixture = generated_fixture();
    let logging_build = BUILD.replace(
        "builder.package(\"generated-table\");",
        &format!(
            "builder.package(\"generated-table\");\n    builder.log.write_line(\"{LOG_MARKER}\");"
        ),
    );
    fixture.write("dependency/build.omg", &logging_build);
    let installed = fixture.omega(&["install", "../dependency", "--target", "linux_x86_64"]);
    assert_status(&installed, 0);
    assert!(
        String::from_utf8_lossy(&installed.stdout)
            .lines()
            .any(|line| line == LOG_MARKER),
        "the positive control must demonstrate that executed BuildLog is visible"
    );
    let before = fixture.accepted_files();
    // The source read and log precede this declaration in authored execution
    // order. Discovery still rejects it before either operation can execute.
    let rejected_build = logging_build.replace(
        "    let generated: BuildPath",
        &format!("    {declaration}\n    let generated: BuildPath"),
    );
    assert_ne!(logging_build, rejected_build);
    fixture.write("dependency/build.omg", &rejected_build);
    let output = fixture.omega(&["update", "generated_table"]);
    assert_status(&output, 1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(expected), "{stderr}");
    assert!(!stderr.contains(LOG_MARKER), "{stderr}");
    assert!(!String::from_utf8_lossy(&output.stdout).contains(LOG_MARKER));
    assert_eq!(fixture.accepted_files(), before);
    assert_eq!(fixture.read("dependency/build.omg"), rejected_build);
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
}

#[test]
fn package_build_source_facet_rejects_absolute_and_parent_paths() {
    for relative in [false, true] {
        let fixture = generated_fixture();
        fixture.write("outside.txt", "fake outside input, not credentials\n");
        let location = if relative {
            "../outside.txt".to_owned()
        } else {
            fixture
                .path("outside.txt")
                .to_str()
                .unwrap()
                .replace('\\', "/")
        };
        fixture.write(
            "dependency/build.omg",
            &BUILD.replace("inputs/table.txt", &location),
        );
        let before = fixture.accepted_files();
        let output = fixture.omega(&["install", "../dependency", "--target", "linux_x86_64"]);
        assert_status(&output, 1);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let expected = if relative {
            "build-root path must use canonical relative components"
        } else {
            "build-root path must not use an absolute or host-specific spelling"
        };
        assert!(stderr.contains(expected), "{stderr}");
        assert_eq!(fixture.accepted_files(), before);
        assert_eq!(
            fixture.read("outside.txt"),
            "fake outside input, not credentials\n"
        );
        assert!(!fixture.path("root/build/package-manager/proposal").exists());
        assert!(!fixture.path("dependency/table.generated.omg").exists());
    }
}

#[test]
fn transitive_runtime_service_is_not_package_build_authority() {
    let fixture = authority_fixture(
        r#"machine build(builder: &mut Build) {
    builder.package("runtime-build");
    builder.depend(Source::Path { location: "../host-services" });
    touch_runtime();
}
"#,
        r#"use host_services::console;

machine touch_runtime()
reaches Console
{
    let mut console: Console;
    console.exit_process(0);
}
"#,
    );
    let before = fixture.accepted_files();
    let output = fixture.omega(&["install", "../dependency", "--target", "linux_x86_64"]);
    assert_status(&output, 1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("build.omg may not reach runtime boundary services"),
        "{stderr}"
    );
    assert!(stderr.contains("compiler-owned Build facets"), "{stderr}");
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
}
