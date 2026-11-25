/*

End-to-end CLI check that a Build can wait for a provider from a build
dependency and then call through it (`omega --check`).

`suspend Build::await_provider<ProgramImage, backing::TestImage>(builder);`
asks the build evaluator to make `backing::TestImage` the implementation of
`ProgramImage` for the rest of this Build
(`wiki/spec/build/provider_selection.md`, "Build provider readiness"). The
Build parks at that call. The evaluator checks the dependency it registered
as `backing`, publishes the named implementation, installs it, and resumes
the Build, which then reads `ProgramImage::retained_suffix_start()` through
the installed provider.

The kept fixture does exactly that and binds its entry only if the value it
read is the one `TestImage` returns (64). The twins are the same project
with one name changed. `OtherImage` returns 128, so the Build takes the arm
that declares a second project role and is refused for it: the call really
went through the provider the Build named. `PrivateImage` is not public and
`MissingImage` does not exist, so neither can be named as a backing.

*/

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture() -> PathBuf {
    support::repository_root().join("tests/fixtures/build/pass/explicit_provider_readiness_exit")
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("staging directory");
    for entry in std::fs::read_dir(from).expect("fixture directory") {
        let entry = entry.expect("fixture entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("fixture entry type").is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("fixture file");
        }
    }
}

/// `omega --check` on the fixture with `backing::TestImage` replaced by
/// `backing::{backing}` in its Build.
fn check(backing: &str) -> (bool, String) {
    check_with_product_wait(backing, None)
}

fn check_with_product_wait(backing: &str, observe_before_wait: Option<bool>) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-provider-readiness-{}-{backing}-{observe_before_wait:?}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    copy(&fixture(), &root);
    let build = root.join("build.omg");
    let text = std::fs::read_to_string(&build).expect("fixture Build");
    let marker = r"backing\TestImage";
    assert!(text.contains(marker));
    let mut text = text.replace(marker, &format!(r"backing\{backing}"));
    if let Some(read_first) = observe_before_wait {
        // Only the provider request names the backing package. Its code never
        // becomes an ordinary source import of the running Build.
        std::fs::rename(root.join("backing"), root.join("vendor")).unwrap();
        text = text.replace(r#"location: "backing""#, r#"location: "vendor""#);
        let product = root.join("product");
        std::fs::create_dir(&product).unwrap();
        std::fs::write(
            product.join("build.omg"),
            r#"machine build(builder: @mutable Build) { Build::package(builder, "product"); }"#,
        )
        .unwrap();
        std::fs::write(product.join("main.omg"), "public data Item {}").unwrap();
        text = text.replace("    let observed:", r#"    let request = dependency\from_source(Source::Path { location: "product" });
    let registration = Build::dependency(builder, request);
    suspend Build::prepare_dependency(builder, registration);
    let expected: addr = expected_offset();
    let observed:"#).replace("observed == 64", "observed == expected");
        text.push_str("\nmachine expected_offset() -> addr { 64 }\n");
        if read_first {
            text = text.replace("    suspend Build::prepare_dependency", "    let prior: addr = ProgramImage::retained_suffix_start();\n    suspend Build::prepare_dependency")
                .replace("observed == expected", "prior == observed && observed == expected");
        }
    }
    std::fs::write(&build, text).expect("staged Build");
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
fn a_build_waits_for_its_named_provider_and_calls_through_it() {
    let (compiled, stderr) = check("TestImage");
    assert!(compiled, "{stderr}");
}

#[test]
fn the_call_after_the_wait_goes_through_the_provider_the_build_named() {
    let (compiled, stderr) = check("OtherImage");
    assert!(!compiled);
    assert!(stderr.contains("project role"), "{stderr}");
}

#[test]
fn a_backing_that_is_private_or_absent_is_refused() {
    for backing in ["PrivateImage", "MissingImage"] {
        let (compiled, stderr) = check(backing);
        assert!(!compiled, "{backing}");
        assert!(
            stderr.contains(
                "Build backing names no public implementation in its exact dependency package"
            ),
            "{stderr}"
        );
    }
}

#[test]
fn the_installed_provider_survives_a_product_dependency_wait() {
    for read_first in [false, true] {
        let (compiled, report) = check_with_product_wait("TestImage", Some(read_first));
        assert!(compiled, "{report}");
        let (compiled, report) = check_with_product_wait("OtherImage", Some(read_first));
        assert!(!compiled && report.contains("project role"), "{report}");
    }
}
