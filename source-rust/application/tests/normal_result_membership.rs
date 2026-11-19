//! Calls retain the domain facts their normal-result contracts promise.

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn fixture(tier: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(tier)
        .join("domains/normal_result_membership")
}

fn invoke(build: &Path, source: &str) -> (Output, String) {
    let scratch = TemporaryDirectory::create("normal-result-membership", false).unwrap();
    std::fs::copy(build, scratch.path().join("build.omg")).unwrap();
    std::fs::write(scratch.path().join("main.omg"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args([
            "run",
            "--both",
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
            "main.omg",
        ])
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .expect("the compiler runs with the fixture's explicit inputs");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{report}");
    (output, report)
}

#[test]
fn ordinary_provider_and_native_results_retain_their_promised_domains() {
    let fixture = fixture("run");
    let source = std::fs::read_to_string(fixture.join("main.omg")).unwrap();
    let (output, report) = invoke(&fixture.join("build.omg"), &source);
    assert!(output.status.success(), "{report}");
    assert!(output.stdout.is_empty(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn the_selected_body_must_establish_its_promised_domain() {
    let fixture = fixture("fail");
    let source = std::fs::read_to_string(fixture.join("main.omg")).unwrap();
    let (output, report) = invoke(&fixture.join("build.omg"), &source);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("normal completion does not establish its promised runtime membership"),
        "{report}"
    );
}

#[test]
fn unrestricted_issuance_needs_no_separate_origin_fact() {
    let fixture = fixture("fail");
    let source = std::fs::read_to_string(fixture.join("main.omg")).unwrap();
    let selected =
        "access::new_plan(@schema, Issuance::Only { routes: [reflect::callable(tick_source)] })";
    assert_eq!(source.matches(selected).count(), 1);
    let source = source.replace(
        selected,
        "access::new_plan(@schema, Issuance::Unrestricted)",
    );
    let (output, report) = invoke(&fixture.join("build.omg"), &source);
    assert!(output.status.success(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_result_promise_cannot_hide_read_restrictions() {
    let fixture = fixture("run");
    let source = std::fs::read_to_string(fixture.join("main.omg")).unwrap();
    let selected =
        "access::new_plan(@schema, Issuance::Only { routes: [reflect::callable(stamp)] })";
    assert_eq!(source.matches(selected).count(), 1);
    let restricted = format!(
        "let mut plan: AccessPlan = {selected};\naccess::allow_readers(@mut plan, AccessRegion::Subject {{ schema: schema }}, [reflect::callable(stamp)]);\nplan"
    );
    let source = format!(
        "use omega::language::core::AccessRegion;\n{}",
        source.replace(selected, &restricted)
    );
    let (output, report) = invoke(&fixture.join("build.omg"), &source);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("runtime contract membership governed by an access policy"),
        "{report}"
    );
}
