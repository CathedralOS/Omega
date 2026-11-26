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

#[test]
fn represented_result_fields_keep_the_domain_of_their_semantic_record() {
    let scratch = TemporaryDirectory::create("represented-result-membership", false).unwrap();
    std::fs::write(
        scratch.path().join("build.omg"),
        r#"machine build(plan: @mutable BuildPlan) {
            plan.product = BuildProduct::Package { name: "represented_result" };
        }"#,
    ).unwrap();
    let source = r#"use {omega}/language/core/c_layout.omg;
        public data Fields { bits: u64; }
        public data Other { bits: u64; }
        public domain Fields::Initialized;
        public domain Other::Initialized;
        public data Holder { field: |c_layout|::CLayout<Fields> in Fields::Initialized; }
        public machine keep(holder: Holder) -> Holder
        ensures result.field in Fields::Initialized { holder }"#;
    for (source, accepted) in [
        (source.to_owned(), true),
        (source.replace("ensures result.field in Fields", "ensures result.field in Other"), false),
    ] {
        std::fs::write(scratch.path().join("main.omg"), source).unwrap();
        let (output, report) = observations::run(&scratch, &["--check", "--target", "linux_x86_64"], INPUTS);
        assert_eq!(output.status.success(), accepted, "{report}");
        if !accepted {
            assert!(report.contains("domain declared on another carrier"), "{report}");
        }
    }
}
