//! `domain Ticket::Stamped access StampAccess;` leaves who may write
//! `ticket as +Stamped` to the plan `StampAccess::plan` returns
//! (`wiki/spec/language/domains.md`, "Domain-owned read and write
//! authority"). The Build runs that plan for the demanded domain, and the
//! lowered-form checker reads its routes: the machine the plan lists may cast,
//! and the twin whose plan lists another machine is refused for that.

mod support;

use std::process::{Command, Stdio};

fn run_both(project: &std::path::Path, arguments: &[&str]) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both"])
        .args(arguments)
        .current_dir(project)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs on the hosted target");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    (output.status.success(), report)
}

#[test]
fn the_machine_a_domain_plan_lists_may_issue_it() {
    let (succeeded, report) = run_both(
        &support::repository_root()
            .join("source-rust/application/tests/fixtures/domains/run/access_plan_issuer"),
        &["main.omg"],
    );
    assert!(succeeded, "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_machine_the_domain_plan_does_not_list_is_refused() {
    let (succeeded, report) = run_both(
        &support::repository_root()
            .join("source-rust/application/tests/fixtures/domains/fail/access_plan_other_issuer"),
        &["main.omg"],
    );
    assert!(!succeeded, "{report}");
    let source = include_str!("fixtures/domains/fail/access_plan_other_issuer/main.omg");
    assert_denied_cast(&report, source, "Ticket::Stamped");
}

fn assert_denied_cast(report: &str, source: &str, domain: &str) {
    let (prefix, _) = source.rsplit_once("+Stamped").unwrap();
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next().unwrap().chars().count() + 1;
    assert!(
        report.contains(&format!("main.omg:{line}:{column}")),
        "{report}"
    );
    assert!(
        report.contains(&format!("this cast adds `{domain}` in a machine that is not one of the domain's establishment routes"))
        && report.contains("in machine `stamp`"),
        "{report}"
    );
    assert!(!report.contains("defect of the checker"), "{report}");
}

#[test]
fn scalar_casts_name_the_denied_issuer_and_its_cast() {
    use platform_custody::temporary_directory::TemporaryDirectory;
    let project = TemporaryDirectory::create("scalar-domain-issuers", false).unwrap();
    let source = r#"use {omega}/language/core/layout.omg as layout;
use {omega}/language/core/access/access.omg as plans;
use {omega}/language/core/reflect/reflect.omg as reflection;
domain i64::Stamped requires self != -1 access StampAccess;
StampAccess satisfies |layout|::AccessPolicy {
    machine plan(schema: |reflection|::TypeSchema) -> |layout|::AccessPlan {
        |plans|::new_plan(@schema, |layout|::Issuance::Only { routes: [|reflection|::callable(stamp)] })
    }
}
machine stamp() -> i64 in Stamped { 1i64 as +Stamped }
machine unrelated() -> i64 { 1i64 }
machine main() crashes Trap {
    let result: i64 in Stamped = stamp();
    transition (result as -Stamped) == 1i64 { true -> done(), _ -> wrong() }
    machine done() {}
    machine wrong() { crash Trap; }
}
"#;
    std::fs::write(
        project.path().join("build.omg"),
        include_str!("fixtures/domains/run/access_plan_issuer/build.omg"),
    )
    .unwrap();
    for (route, allowed) in [
        ("|reflection|::callable(stamp)", true),
        ("|reflection|::callable(unrelated)", false),
        ("", false),
    ] {
        let program = source.replace("|reflection|::callable(stamp)", route);
        std::fs::write(project.path().join("main.omg"), &program).unwrap();
        let (succeeded, report) = run_both(
            project.path(),
            &[
                "--build-input",
                "main.omg",
                "--build-input",
                "build.omg",
                "build.omg",
            ],
        );
        assert_eq!(succeeded, allowed, "{report}");
        if allowed {
            assert!(
                report.contains("native exit 0") && report.contains("interp exit: 0"),
                "{report}"
            );
        } else {
            assert_denied_cast(&report, &program, "i64::Stamped");
        }
    }
}
