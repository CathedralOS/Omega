//! Calls retain the domain facts their normal-result contracts promise.

#[path = "support/observations.rs"]
mod observations;
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::{Path, PathBuf};

fn fixture(tier: &str) -> PathBuf {
    support::repository_root()
        .join("tests/fixtures/domains")
        .join(tier)
        .join("normal_result_membership")
}

fn project(build: &Path, source: &str) -> TemporaryDirectory {
    let scratch = TemporaryDirectory::create("normal-result-membership", false).unwrap();
    std::fs::copy(build, scratch.path().join("build.omg")).unwrap();
    std::fs::write(scratch.path().join("main.omg"), source).unwrap();
    scratch
}

const INPUTS: &[&str] = &[
    "--build-input",
    "main.omg",
    "--build-input",
    "build.omg",
    "build.omg",
];

#[test]
fn ordinary_provider_and_native_results_retain_their_promised_domains() {
    let fixture = fixture("run");
    let source = std::fs::read_to_string(fixture.join("main.omg")).unwrap();
    observations::assert_silent_return(|| project(&fixture.join("build.omg"), &source), INPUTS);
}

#[test]
fn the_selected_body_must_establish_its_promised_domain() {
    let fixture = fixture("fail");
    let source = std::fs::read_to_string(fixture.join("main.omg")).unwrap();
    let (output, report) = observations::run(
        &project(&fixture.join("build.omg"), &source),
        &["--check"],
        INPUTS,
    );
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
    let selected = r"|access|::new_plan(@schema, |layout|::Issuance::Only { routes: [|reflect|::callable(tick_source)] })";
    assert_eq!(source.matches(selected).count(), 1);
    let source = source.replace(
        selected,
        r"|access|::new_plan(@schema, |layout|::Issuance::Unrestricted)",
    );
    observations::assert_silent_return(|| project(&fixture.join("build.omg"), &source), INPUTS);
}

#[test]
fn a_result_promise_cannot_hide_read_restrictions() {
    let fixture = fixture("run");
    let source = std::fs::read_to_string(fixture.join("main.omg")).unwrap();
    let selected = r"|access|::new_plan(@schema, |layout|::Issuance::Only { routes: [|reflect|::callable(stamp)] })";
    assert_eq!(source.matches(selected).count(), 1);
    let restricted = format!(
        "let mutable plan: |layout|::AccessPlan = {selected};\n\
         |access|::allow_readers(@mutable plan, |layout|::AccessRegion::Subject {{ schema: schema }}, \
         [|reflect|::callable(stamp)]);\nplan"
    );
    let source = source.replace(selected, &restricted);
    let (output, report) = observations::run(
        &project(&fixture.join("build.omg"), &source),
        &["--check"],
        INPUTS,
    );
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("runtime contract membership governed by an access policy"),
        "{report}"
    );
}
