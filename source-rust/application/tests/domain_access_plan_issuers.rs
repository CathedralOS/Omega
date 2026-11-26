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

#[test]
fn an_unrestricted_policy_checks_its_predicates_on_an_unissued_value() {
    use platform_custody::temporary_directory::TemporaryDirectory;

    let project = TemporaryDirectory::create("unrestricted-membership-test", false).unwrap();
    let build = include_str!("fixtures/domains/run/unrestricted_membership_test/build.omg");
    let source = include_str!("fixtures/domains/run/unrestricted_membership_test/main.omg");
    std::fs::write(project.path().join("build.omg"), build).unwrap();
    let arguments = [
        "--build-input",
        "build.omg",
        "--build-input",
        "main.omg",
        "build.omg",
    ];
    std::fs::write(project.path().join("main.omg"), source).unwrap();
    let (succeeded, report) = run_both(project.path(), &arguments);
    assert!(succeeded, "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }

    let unrestricted = "|layout|::Issuance::Unrestricted";
    assert_eq!(source.matches(unrestricted).count(), 1);
    let denied = source.replace(unrestricted, "|layout|::Issuance::Only { routes: [] }");
    std::fs::write(project.path().join("main.omg"), denied).unwrap();
    let (succeeded, report) = run_both(project.path(), &arguments);
    assert!(!succeeded, "{report}");
    assert!(
        report.contains("requires retained authorized provenance"),
        "{report}"
    );
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

/// A dependency's Build may select a provider whose member is the only code
/// that names a governed domain, as system's Windows filesystem adapters do with
/// their handle domains. The Product request never names that member, but
/// lowering reads it, so the Build must still run the domain's plan: the
/// member may then remove `Stamped` and the program runs. The twin's plan
/// keeps a reader rule, which a removal cannot carry, so it stays refused.
#[test]
fn a_domain_only_a_selected_dependency_provider_uses_has_its_plan_run() {
    use platform_custody::temporary_directory::TemporaryDirectory;
    let project = TemporaryDirectory::create("provider-domain-plans", false).unwrap();
    let kit = project.path().join("kit");
    std::fs::create_dir_all(kit.join("main")).unwrap();
    let targets = [
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
        "windows_x86_64",
    ];
    let mut kit_build = String::from(
        "machine build(builder: @mutable Build) {\n    Build::package(builder, \"stamp_kit\");\n",
    );
    let mut consumer_build = String::from(
        r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) {
    Build::application(builder, "stamp_consumer");
    let kit: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "kit" }));
"#,
    );
    for target in targets {
        kit_build.push_str(&format!(
            "    Build::select_provider::<Probe, StampedProbe>(builder, {target});\n"
        ));
        consumer_build.push_str(&format!(
            "    Build::bind(builder, {target}::ProgramEntry, main);\n"
        ));
    }
    kit_build.push_str("}\n");
    consumer_build.push_str("}\n");
    std::fs::write(kit.join("build.omg"), kit_build).unwrap();
    std::fs::write(kit.join("main.omg"), "use main;\n").unwrap();
    std::fs::write(project.path().join("build.omg"), consumer_build).unwrap();
    std::fs::write(
        project.path().join("main.omg"),
        r#"use stamp_kit\main\Probe;
machine main() crashes Trap {
    let value: i64 = Probe::probe();
    transition value == 4i64 { true -> done(), _ -> wrong() }
    machine done() {}
    machine wrong() { crash Trap; }
}
"#,
    )
    .unwrap();
    let issued = "|plans|::new_plan(@schema, |layout|::Issuance::Only { routes: [|reflection|::callable(stamp)] })";
    let source = r#"use {omega}/language/core/layout.omg as layout;
use {omega}/language/core/access/access.omg as plans;
use {omega}/language/core/reflect/reflect.omg as reflection;
public boundary trait Probe {
    machine probe() -> i64 reaches none crashes none;
}
domain i64::Stamped requires self != -1 access StampAccess;
StampAccess satisfies |layout|::AccessPolicy {
    machine plan(schema: |reflection|::TypeSchema) -> |layout|::AccessPlan {
        PLAN
    }
}
machine stamp() -> i64 in Stamped { 4i64 as +Stamped }
public StampedProbe satisfies Probe {
    machine probe() -> i64 {
        let stamped: i64 in Stamped = stamp();
        stamped as -Stamped
    }
}
"#;
    let read_restricted = format!(
        "let mutable plan: |layout|::AccessPlan = {issued};\n        |plans|::allow_readers(@mutable plan, |layout|::AccessRegion::Subject {{ schema: schema }}, [|reflection|::callable(stamp)]);\n        plan"
    );
    for (plan, runs) in [(issued.to_owned(), true), (read_restricted, false)] {
        let program = source.replace("PLAN", &plan);
        std::fs::write(kit.join("main").join("stamps.omg"), &program).unwrap();
        let (succeeded, report) = run_both(project.path(), &["build.omg"]);
        assert_eq!(succeeded, runs, "{report}");
        if runs {
            for receipt in ["native exit 0", "interp exit: 0"] {
                assert!(report.lines().any(|line| line == receipt), "{report}");
            }
        } else {
            // The checker names the member whose body holds the removal.
            let (prefix, _) = program.split_once("probe() -> i64 {").unwrap();
            let line = prefix.lines().count();
            assert!(
                report.contains("removal of a domain governed by an access policy")
                    && report.contains("the body of `main::StampedProbe::probe`")
                    && report.contains(&format!("stamps.omg:{line}:13")),
                "{report}"
            );
        }
    }
}
