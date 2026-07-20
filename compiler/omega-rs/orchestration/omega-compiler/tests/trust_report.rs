//! GR5 (chapter-10 carrier): the trust report writes one row per admitted
//! semantic commitment. Today's rows are sealed-domain introductions --
//! own-package declarations are dev-active (grant locality v1) and carry
//! the standing warning until a root grant (GR3) flips their provenance.

use omega_compiler::{CompileOptions, compile};

#[test]
fn trust_report_rows_dev_active_domain_introductions() {
    let project = std::env::temp_dir().join(format!("omega-trust-report-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"domain u32::Meters {}
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::main(&mut self) {
    let d: u32 in Meters = (7 as u32 in Meters);
    self.console.exit_process(70);
}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("domain program should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    assert!(
        report.contains("admitted commitments: 1"),
        "expected one commitment row:\n{report}"
    );
    assert!(
        report.contains("domain introduction: u32::Meters -- own-package (dev-active)"),
        "expected the dev-active domain row:\n{report}"
    );
    assert!(
        report.contains("STANDING WARNING"),
        "dev-active rows carry the standing warning until granted:\n{report}"
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn trust_report_empty_without_commitments() {
    let project = std::env::temp_dir().join(format!("omega-trust-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::main(&mut self) {
    self.console.exit_process(70);
}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("plain program should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("the empty report is still written -- the honest no-commitments statement");
    assert!(
        report.contains("admitted commitments: 0"),
        "expected zero rows:\n{report}"
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn root_grant_flips_domain_row_and_retires_warning() {
    // GR3: `b.accept_boundary<Meters>();` in build.omg harvests as a root
    // grant; the granted domain's trust row flips provenance and drops the
    // standing warning. A grant naming no declared domain surfaces as an
    // accepted-fact row (the report sees every grant).
    let project = std::env::temp_dir().join(format!("omega-trust-grant-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("build.omg"),
        r#"data Subsystem { case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }
data Build { subsystem: Subsystem; freestanding: bool; }

machine build(b: &mut Build) {
    b.accept_boundary<Meters>();
    b.accept_boundary<walker_lib::collatz_cert_checked>();
}
"#,
    )
    .expect("write build.omg");
    std::fs::write(
        project.join("main.omg"),
        r#"domain u32::Meters {}
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::main(&mut self) {
    let d: u32 in Meters = (7 as u32 in Meters);
    self.console.exit_process(70);
}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("granted project should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    assert!(
        report.contains("domain introduction: u32::Meters -- root grant (build.omg)"),
        "expected the granted domain row:\n{report}"
    );
    assert!(
        report
            .contains("accepted fact: walker_lib::collatz_cert_checked -- root grant (build.omg)"),
        "expected the accepted-fact row:\n{report}"
    );
    let meters_row = report
        .lines()
        .find(|line| line.contains("u32::Meters"))
        .unwrap_or_default();
    assert!(
        !meters_row.contains("STANDING WARNING"),
        "a root-granted domain drops the standing warning:\n{report}"
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn lockfile_written_and_drift_fails_until_reapproved() {
    // GR4: a granted project writes omega.lock beside build.omg (one
    // receipt row per grant, statement hash recorded automatically); a
    // granted statement that drifts fails the build until re-approved.
    let project = std::env::temp_dir().join(format!("omega-trust-lock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("build.omg"),
        r#"data Subsystem { case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }
data Build { subsystem: Subsystem; freestanding: bool; }

machine build(b: &mut Build) {
    b.accept_boundary<Meters>();
}
"#,
    )
    .expect("write build.omg");
    let main_with = |facts: &str| {
        format!(
            r#"domain u32::Meters {{{facts}}}
boundary trait Console {{ machine exit_process(return_code: i32); }}
data Main {{ console: Console; }}
machine Main::main(&mut self) {{
    let d: u32 in Meters = (7 as u32 in Meters);
    self.console.exit_process(70);
}}
"#
        )
    };
    std::fs::write(project.join("main.omg"), main_with("")).expect("write main.omg");

    let build_dir = project.join("build");
    let options = || CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    };
    compile(options()).expect("granted project should compile");

    let lock = std::fs::read_to_string(project.join("omega.lock"))
        .expect("omega.lock should be written beside build.omg");
    assert!(
        lock.contains("domain introduction: u32::Meters"),
        "expected the domain receipt row:\n{lock}"
    );

    // Drift the granted statement (add a fact) -- the build must refuse.
    std::fs::write(project.join("main.omg"), main_with(" self >= 1; ")).expect("rewrite main.omg");
    let drifted = compile(options());
    let message = format!("{:?}", drifted.expect_err("drift should refuse"));
    assert!(
        message.contains("granted statement drifted"),
        "expected the drift refusal, got: {message}"
    );

    // Re-approve by deleting the lock; the build succeeds and re-pins.
    std::fs::remove_file(project.join("omega.lock")).expect("delete lock");
    compile(options()).expect("re-approved project should compile");

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn granted_axiom_receipt_drifts_on_claim_edit() {
    // GR6d lockfile polish: a granted axiom's receipt hashes its rendered
    // ensures -- editing the CLAIM under the grant is drift.
    let project = std::env::temp_dir().join(format!("omega-axiom-lock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("build.omg"),
        r#"data Subsystem { case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }
data Build { subsystem: Subsystem; freestanding: bool; }

machine build(b: &mut Build) {
    b.accept_boundary<mul_comm_axiom>();
}
"#,
    )
    .expect("write build.omg");
    let main_with = |claim: &str| {
        format!(
            r#"use omega::language::core::nat;
boundary trait Console {{ machine exit_process(return_code: i32); }}
data Main {{ console: Console; }}

boundary machine mul_comm_axiom(a: Nat, b: Nat) -> Nat
ensures
    {claim};

machine Main::main(&mut self) {{
    self.console.exit_process(70);
}}
"#
        )
    };
    std::fs::write(
        project.join("main.omg"),
        main_with("(mul(a, b)) == (mul(b, a))"),
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    let options = || CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    };
    compile(options()).expect("granted axiom project should compile");
    let lock = std::fs::read_to_string(project.join("omega.lock")).expect("lock written");
    assert!(
        lock.contains("accepted fact: mul_comm_axiom"),
        "expected the axiom receipt:\n{lock}"
    );

    // Edit the CLAIM under the grant -- drift refuses.
    std::fs::write(
        project.join("main.omg"),
        main_with("(mul(a, b)) == (mul(a, b))"),
    )
    .expect("rewrite main.omg");
    let message = format!("{:?}", compile(options()).expect_err("drift should refuse"));
    assert!(
        message.contains("granted statement drifted"),
        "expected the drift refusal, got: {message}"
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn granted_generic_axiom_receipt_pins_template_and_machine_requirement() {
    let project =
        std::env::temp_dir().join(format!("omega-generic-axiom-lock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("build.omg"),
        r#"data Subsystem { case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }
data Build { subsystem: Subsystem; freestanding: bool; }

machine build(b: &mut Build) {
    b.accept_boundary<admitted>();
}
"#,
    )
    .expect("write build.omg");
    let main_with = |requirement_clause: &str| {
        format!(
            r#"boundary trait Console {{ machine exit_process(return_code: i32); }}
data Main {{ console: Console; value: i32; }}

machine selected(value: &i32) {{}}

boundary machine admitted<T, machine F>(value: &T)
where machine F(item: &T){requirement_clause};
ensures true;

machine Main::main(&mut self) {{
    admitted<selected>(&self.value);
    self.console.exit_process(70);
}}
"#
        )
    };
    std::fs::write(project.join("main.omg"), main_with("")).expect("write generic axiom source");

    let build_dir = project.join("build");
    let options = || CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    };
    compile(options()).expect("granted generic axiom project should compile");
    let lock = std::fs::read_to_string(project.join("omega.lock")).expect("lock written");
    assert_eq!(
        lock.lines()
            .filter(|line| line.contains("accepted fact: admitted"))
            .count(),
        1,
        "one universal template grant should produce one receipt:\n{lock}"
    );
    let manifest = std::fs::read_to_string(build_dir.join("05_machine_contracts.json"))
        .expect("machine contract manifest written");
    assert!(manifest.contains("\"accepted_template_commitment\": \"admitted\""));
    assert!(manifest.contains("\"machine_argument_contract_fingerprints\": [\"0x"));

    // Changing the authored machine-parameter contract changes the universal
    // template statement under the existing grant. The lockfile gate runs
    // before instantiation, so it reports drift rather than spending a new
    // per-instance grant.
    std::fs::write(project.join("main.omg"), main_with(" ensures true"))
        .expect("rewrite generic axiom requirement");
    let message = format!(
        "{:?}",
        compile(options()).expect_err("template drift should refuse")
    );
    assert!(
        message.contains("granted statement drifted"),
        "expected generic template drift refusal, got: {message}"
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn granted_plan_receipt_pins_the_fingerprint() {
    // PRV3: granting a derived plan pins its normalized identity in the
    // lockfile; changing the plan's policy under the grant drifts.
    let project = std::env::temp_dir().join(format!("omega-plan-lock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("build.omg"),
        r#"data Subsystem { case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }
data Build { subsystem: Subsystem; freestanding: bool; }

machine build(b: &mut Build) {
    b.accept_boundary<Flags>();
}
"#,
    )
    .expect("write build.omg");
    let main_with = |value: i64| {
        format!(
            r#"boundary trait Console {{ machine exit_process(return_code: i32); }}
boundary trait Flags {{
    machine open_read() -> i32;
}}
demo_target provides Flags {{
    open_read -> {value}
}}
data Main {{ console: Console; }}
machine Main::main(&mut self) {{
    self.console.exit_process(70);
}}
"#
        )
    };
    std::fs::write(project.join("main.omg"), main_with(0)).expect("write main.omg");

    let build_dir = project.join("build");
    let options = || CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    };
    compile(options()).expect("granted plan project should compile");
    let lock = std::fs::read_to_string(project.join("omega.lock")).expect("lock written");
    assert!(
        lock.contains("provider plan: demo_target::Flags"),
        "expected the plan receipt:\n{lock}"
    );

    // Change the plan's POLICY under the grant -- drift refuses.
    std::fs::write(project.join("main.omg"), main_with(7)).expect("rewrite main.omg");
    let message = format!("{:?}", compile(options()).expect_err("drift should refuse"));
    assert!(
        message.contains("granted statement drifted"),
        "expected the drift refusal, got: {message}"
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn derived_provider_plans_surface_as_trust_rows() {
    // PRV3: an authored `provides` block derives a ProviderPlan; the plan
    // surfaces as a dev-active trust row (fingerprint shown) until the
    // final build grants it by name or trait leaf.
    let project = std::env::temp_dir().join(format!("omega-plan-rows-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"boundary trait Console { machine exit_process(return_code: i32); }
boundary trait Flags {
    machine open_read() -> i32;
}
demo_target provides Flags {
    open_read -> 0
}
data Main { console: Console; }
machine Main::main(&mut self) {
    self.console.exit_process(70);
}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("provides project should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    assert!(
        report.contains("provider plan: demo_target::Flags ["),
        "expected the derived plan row with its fingerprint:\n{report}"
    );
    let plan_row = report
        .lines()
        .find(|line| line.contains("provider plan: demo_target::Flags"))
        .unwrap_or_default();
    assert!(
        plan_row.contains("own-package (dev-active)") && plan_row.contains("STANDING WARNING"),
        "an ungranted plan is dev-active with the warning:\n{report}"
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn satisfies_leaves_derive_a_covered_plan() {
    // PRV4 step (2): external leaves assemble one plan per (trait, target)
    // with coverage counted against the typed schema; the trust row shows
    // the fingerprint and coverage.
    let project = std::env::temp_dir().join(format!("omega-sat-plan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"boundary trait Console { machine exit_process(return_code: i32); }
boundary trait Pair {
    machine first(code: i32) -> i32;
    machine second(code: i32) -> i32;
}

machine first_leaf(code: i32) -> i32 satisfies Pair::first via Binding::VtableSlot(1);

data Main { console: Console; }
machine Main::main(&mut self) {
    self.console.exit_process(70);
}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("satisfies-leaf project should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    assert!(
        report.contains("provider plan: satisfies::Pair ["),
        "expected the satisfies-derived plan row:\n{report}"
    );
    let row = report
        .lines()
        .find(|line| line.contains("provider plan: satisfies::Pair"))
        .unwrap_or_default();
    assert!(
        row.contains("coverage 1/2"),
        "one of two requirements satisfied -> coverage 1/2:\n{report}"
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn provider_type_conformance_closures_remain_separate() {
    // PRV4c prerequisite: rows attached to different provider types are
    // different candidates. Two half-providers must never become one covered
    // plan merely because they satisfy different requirements of one trait.
    let project =
        std::env::temp_dir().join(format!("omega-provider-closure-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"boundary trait Console { machine exit_process(return_code: i32); }
boundary trait Pair {
    machine first(code: i32) -> i32;
    machine second(code: i32) -> i32;
}

data FirstProvider { }
machine FirstProvider::first(code: i32) -> i32
    satisfies Pair::first via Binding::VtableSlot(1);

data SecondProvider { }
machine SecondProvider::second(code: i32) -> i32
    satisfies Pair::second via Binding::VtableSlot(2);

data Main { console: Console; }
machine Main::main(&mut self) {
    self.console.exit_process(70);
}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("separate partial provider candidates should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    for provider in ["FirstProvider", "SecondProvider"] {
        let needle = format!("provider plan: {provider}::satisfies::Pair");
        let row = report
            .lines()
            .find(|line| line.contains(&needle))
            .unwrap_or_else(|| panic!("missing {provider} candidate:\n{report}"));
        assert!(
            row.contains("coverage 1/2"),
            "{provider} must remain a half-provider:\n{report}"
        );
    }

    let _ = std::fs::remove_dir_all(&project);
}
