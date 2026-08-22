//! GR5 (chapter-10 carrier): the trust report writes one row per admitted
//! semantic commitment. Today's rows are sealed-domain introductions --
//! own-package declarations are dev-active (grant locality v1) and carry
//! the standing warning until a root grant (GR3) flips their provenance.

use omega_compiler::{CompileOptions, compile_to_checked};

fn compile(
    options: CompileOptions,
) -> Result<omega_compiler::CompileReport, Vec<psi_diagnostics::Diagnostic>> {
    assert!(
        !options.write_output,
        "trust-report fixtures are entry-agnostic semantic checks"
    );
    let report = omega_compiler::compile(options)?;
    assert!(!report.wrote_output());
    assert!(report.program_storage_entry().is_none());
    Ok(report)
}

#[test]
fn trust_report_rows_dev_active_domain_introductions() {
    let project = std::env::temp_dir().join(format!("omega-trust-report-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"domain u32::Meters;
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::exercise(&mut self) {
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
        write_output: false,
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
machine Main::exercise(&mut self) {
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
        write_output: false,
    })
    .expect("plain program should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("the empty report is still written -- the honest no-commitments statement");
    let empty_selected_closure =
        omega_effects::SelectedProviderPlanFacts::default().normalized_identity();
    assert!(report.contains(&format!(
        "selected provider closure: {empty_selected_closure:016x}"
    )));
    assert!(
        report.contains("admitted commitments: 0"),
        "expected zero rows:\n{report}"
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn claim_free_boundary_symbols_do_not_consume_trust() {
    let project =
        std::env::temp_dir().join(format!("omega-trust-claim-free-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"boundary data Carrier;
boundary machine Carrier::combine(a: Carrier, b: Carrier) -> Carrier;
boundary trait AlgebraAudit {}
boundary machine combine_commutative(callback: &mut AlgebraAudit, a: Carrier, b: Carrier)
reaches AlgebraAudit
invokes callback;
suspends;
blocks;
terminates;
crashes Abort
ensures Carrier::combine(a, b) == Carrier::combine(b, a);

data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");

    let checked = compile_to_checked(&project.join("main.omg"), None)
        .expect("accepted claim should reach checked facts");
    let accepted = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "combine_commutative")
        .expect("accepted claim machine");
    let expected_contract_fingerprint = checked
        .facts
        .contract_plans
        .for_machine(accepted.symbol)
        .expect("accepted claim contract plan")
        .fingerprint;

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: false,
    })
    .expect("claim-free boundary symbol program should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    assert!(
        report.contains("admitted commitments: 1"),
        "only the authored axiom is a commitment:\n{report}"
    );
    assert!(
        report.contains("accepted fact: combine_commutative"),
        "the authored axiom remains visible:\n{report}"
    );
    let accepted_row = report
        .lines()
        .find(|line| line.contains("accepted fact: combine_commutative"))
        .expect("accepted claim row");
    assert!(accepted_row.contains(&format!(
        "machine contract: {expected_contract_fingerprint:016x}"
    )));
    assert!(accepted_row.contains("service reach: AlgebraAudit"));
    assert!(accepted_row.contains("synchronous invocations: parameter:0"));
    assert!(accepted_row.contains("may suspend: yes"));
    assert!(accepted_row.contains("may block: yes"));
    assert!(accepted_row.contains("termination guarantee: yes"));
    assert!(accepted_row.contains("crash routes: Abort[true]"));
    assert!(
        !report.contains("accepted fact: Carrier::combine"),
        "a claim-free symbol asserts nothing and needs no grant:\n{report}"
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn claim_free_boundary_symbols_are_not_runtime_providers() {
    let project = std::env::temp_dir().join(format!(
        "omega-trust-claim-free-runtime-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"boundary machine unexplained_runtime_operation();

data Main {}
machine Main::exercise(&mut self) {
    unexplained_runtime_operation();
}
"#,
    )
    .expect("write main.omg");

    let diagnostics = compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(project.join("build")),
        target_name: None,
        write_output: false,
    })
    .expect_err("a claim-free symbol has no executable provider");
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("has no executable realization"),
        "expected the bodyless-boundary runtime fence:\n{rendered}"
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
        r#"domain u32::Meters;
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::exercise(&mut self) {
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
        write_output: false,
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
    assert!(!meters_row.contains("machine contract:"));
    assert!(!meters_row.contains("service reach:"));
    assert!(!meters_row.contains("synchronous invocations:"));
    assert!(!meters_row.contains("may suspend:"));
    assert!(!meters_row.contains("may block:"));
    assert!(!meters_row.contains("termination guarantee:"));
    assert!(!meters_row.contains("crash routes:"));
    let unmatched_grant_row = report
        .lines()
        .find(|line| line.contains("accepted fact: walker_lib::collatz_cert_checked"))
        .expect("unmatched imported grant row");
    assert!(!unmatched_grant_row.contains("machine contract:"));
    assert!(!unmatched_grant_row.contains("service reach:"));
    assert!(!unmatched_grant_row.contains("synchronous invocations:"));
    assert!(!unmatched_grant_row.contains("may suspend:"));
    assert!(!unmatched_grant_row.contains("may block:"));
    assert!(!unmatched_grant_row.contains("termination guarantee:"));
    assert!(!unmatched_grant_row.contains("crash routes:"));

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn non_provider_grants_use_one_exact_subject_in_lock_and_report() {
    let project =
        std::env::temp_dir().join(format!("omega-trust-exact-grant-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"domain u32::Meters;
domain i32::Meters;
data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");
    let build_with = |grant: &str| {
        format!(
            r#"data Subsystem {{ case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }}
data Build {{ subsystem: Subsystem; freestanding: bool; }}

machine build(b: &mut Build) {{
    b.accept_boundary<{grant}>();
}}
"#
        )
    };
    let build_dir = project.join("build");
    let options = || CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: false,
    };

    std::fs::write(project.join("build.omg"), build_with("Meters")).expect("write ambiguous grant");
    let diagnostics = compile(options()).expect_err("ambiguous short grant must reject");
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("root grant `Meters` is ambiguous across non-provider trust subjects"),
        "expected exact grant ambiguity diagnostic:\n{rendered}",
    );
    assert!(!project.join("omega.lock").exists());
    assert!(!build_dir.join("trust_report.md").exists());

    std::fs::write(project.join("build.omg"), build_with("u32::Meters"))
        .expect("write exact grant");
    compile(options()).expect("exact qualified grant should compile");

    let lock = std::fs::read_to_string(project.join("omega.lock")).expect("trust lock written");
    assert!(lock.contains("domain introduction: u32::Meters"));
    assert!(!lock.contains("domain introduction: i32::Meters"));
    let report =
        std::fs::read_to_string(build_dir.join("trust_report.md")).expect("trust report written");
    let granted = report
        .lines()
        .find(|line| line.contains("domain introduction: u32::Meters"))
        .expect("exact granted domain row");
    let foreign = report
        .lines()
        .find(|line| line.contains("domain introduction: i32::Meters"))
        .expect("same-leaf foreign domain row");
    assert!(granted.contains("root grant (build.omg)"));
    assert!(!granted.contains("STANDING WARNING"));
    assert!(foreign.contains("own-package (dev-active)"));
    assert!(foreign.contains("STANDING WARNING"));

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
        let domain = if facts.trim().is_empty() {
            "domain u32::Meters;".to_owned()
        } else {
            format!("domain u32::Meters\nrequires\n    {}", facts.trim())
        };
        format!(
            r#"{domain}
boundary trait Console {{ machine exit_process(return_code: i32); }}
data Main {{ console: Console; }}
machine Main::exercise(&mut self) {{
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
        write_output: false,
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
fn trust_lock_requires_reapproval_for_added_removed_and_empty_claim_sets() {
    let project = std::env::temp_dir().join(format!("omega-trust-lock-set-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"domain u32::Alpha;
domain u32::Beta;
data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");
    let build_with = |grants: &[&str]| {
        let grants = grants
            .iter()
            .map(|grant| format!("    b.accept_boundary<{grant}>();"))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            r#"data Subsystem {{ case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }}
data Build {{ subsystem: Subsystem; freestanding: bool; }}

machine build(b: &mut Build) {{
{grants}
}}
"#
        )
    };
    let build_dir = project.join("build");
    let options = || CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: false,
    };
    let lock_path = project.join("omega.lock");

    std::fs::write(project.join("build.omg"), build_with(&["Alpha"])).expect("write first grant");
    compile(options()).expect("first approval writes one receipt");
    let one_receipt = std::fs::read_to_string(&lock_path).expect("read first receipt");

    std::fs::write(project.join("build.omg"), build_with(&["Beta", "Alpha"])).expect("add grant");
    let added = format!(
        "{:?}",
        compile(options()).expect_err("adding a grant requires reapproval")
    );
    assert!(added.contains("added: domain introduction: u32::Beta"));
    assert_eq!(
        std::fs::read_to_string(&lock_path).expect("read preserved one-row lock"),
        one_receipt
    );

    std::fs::remove_file(&lock_path).expect("delete lock to approve added grant");
    compile(options()).expect("deleted lock reapproves complete two-row set");
    let two_receipts = std::fs::read_to_string(&lock_path).expect("read two receipts");
    let rows = two_receipts.lines().collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].ends_with("domain introduction: u32::Alpha"));
    assert!(rows[1].ends_with("domain introduction: u32::Beta"));

    std::fs::write(project.join("build.omg"), build_with(&["Beta"])).expect("remove first grant");
    let removed = format!(
        "{:?}",
        compile(options()).expect_err("removing a grant requires reapproval")
    );
    assert!(removed.contains("removed: domain introduction: u32::Alpha"));
    assert_eq!(
        std::fs::read_to_string(&lock_path).expect("read preserved two-row lock"),
        two_receipts
    );

    std::fs::remove_file(&lock_path).expect("delete lock to approve removed grant");
    compile(options()).expect("deleted lock reapproves one-row set");
    let beta_receipt = std::fs::read_to_string(&lock_path).expect("read beta receipt");
    std::fs::write(project.join("build.omg"), build_with(&[])).expect("remove final grant");
    let empty = format!(
        "{:?}",
        compile(options()).expect_err("removing the final grant requires reapproval")
    );
    assert!(empty.contains("removed: domain introduction: u32::Beta"));
    assert_eq!(
        std::fs::read_to_string(&lock_path).expect("read preserved final receipt"),
        beta_receipt
    );

    std::fs::remove_file(&lock_path).expect("delete lock to approve empty set");
    compile(options()).expect("empty set with no lock needs no receipt file");
    assert!(!lock_path.exists());
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn trust_lock_rejects_corrupt_and_duplicate_rows_without_repair() {
    let project =
        std::env::temp_dir().join(format!("omega-trust-lock-corrupt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("build.omg"),
        r#"data Subsystem { case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }
data Build { subsystem: Subsystem; freestanding: bool; }
machine build(b: &mut Build) { b.accept_boundary<Alpha>(); }
"#,
    )
    .expect("write build.omg");
    std::fs::write(
        project.join("main.omg"),
        r#"domain u32::Alpha;
data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");
    let build_dir = project.join("build");
    let options = || CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: false,
    };
    let lock_path = project.join("omega.lock");
    compile(options()).expect("write canonical lock");
    let canonical = std::fs::read_to_string(&lock_path).expect("read canonical lock");

    let malformed = "not a v1 trust receipt\n";
    std::fs::write(&lock_path, malformed).expect("write malformed lock");
    let error = format!(
        "{:?}",
        compile(options()).expect_err("malformed lock must reject")
    );
    assert!(error.contains("malformed v1 receipt row"));
    assert_eq!(
        std::fs::read_to_string(&lock_path).expect("read preserved malformed lock"),
        malformed
    );

    let duplicate = format!("{canonical}{canonical}");
    std::fs::write(&lock_path, &duplicate).expect("write duplicate lock row");
    let error = format!(
        "{:?}",
        compile(options()).expect_err("duplicate lock commitment must reject")
    );
    assert!(error.contains("duplicate commitment"));
    assert_eq!(
        std::fs::read_to_string(&lock_path).expect("read preserved duplicate lock"),
        duplicate
    );

    std::fs::write(&lock_path, canonical.as_bytes()).expect("restore canonical lock");
    compile(options()).expect("unchanged canonical receipt remains accepted");
    assert_eq!(
        std::fs::read_to_string(&lock_path).expect("read canonical lock after rebuild"),
        canonical
    );
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

machine Main::exercise(&mut self) {{
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
        write_output: false,
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
fn granted_axiom_receipt_drifts_on_published_contract_axis_edit() {
    let project =
        std::env::temp_dir().join(format!("omega-axiom-axis-lock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("build.omg"),
        r#"data Subsystem { case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }
data Build { subsystem: Subsystem; freestanding: bool; }

machine build(b: &mut Build) {
    b.accept_boundary<admitted_axis>();
}
"#,
    )
    .expect("write build.omg");
    let main_with = |axis: &str| {
        format!(
            r#"boundary trait Console {{ machine exit_process(return_code: i32); }}
data Main {{ console: Console; }}

boundary machine admitted_axis()
{axis}
ensures true;

machine Main::exercise(&mut self) {{
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
        write_output: false,
    };
    compile(options()).expect("granted axiom project should compile");
    let lock = std::fs::read_to_string(project.join("omega.lock")).expect("lock written");
    assert!(lock.contains("accepted fact: admitted_axis"));
    let report =
        std::fs::read_to_string(build_dir.join("trust_report.md")).expect("trust report written");
    let admitted_row = report
        .lines()
        .find(|line| line.starts_with("- accepted fact: admitted_axis --"))
        .expect("nongeneric accepted row");
    assert!(
        !admitted_row.contains("accepted template:"),
        "nongeneric accepted rows have no universal template identity:\n{admitted_row}"
    );
    assert!(report.contains("generic accepted instances: 0"));

    std::fs::write(project.join("main.omg"), main_with("suspends;")).expect("rewrite main.omg");
    let message = format!(
        "{:?}",
        compile(options()).expect_err("published contract-axis drift should refuse")
    );
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
trait Ranked {{ machine Self::before(&self, other: &Self) -> bool; }}
data Card {{ rank: i32; }}
domain<T, const N: u64> T::Quantity<N>;
Ascending: Card satisfies Ranked {{
    machine before(&self, other: &Card) -> bool {{ self.rank < other.rank }}
}}
Descending: Card satisfies Ranked {{
    machine before(&self, other: &Card) -> bool {{ self.rank > other.rank }}
}}
data Main {{ console: Console; first: Card; second: Card; }}

machine selected_first(value: &Card) {{}}
machine selected_second(value: &Card) ensures true {{}}

boundary machine admitted<T, const N: u64, Order: T satisfies Ranked, machine F>(value: &T) -> i64 in Quantity<N>
where machine F(item: &T){requirement_clause};
ensures true;

machine Main::exercise(&mut self) {{
    let first_receipt: i64 in Quantity<1> = admitted<Card, Ascending, selected_first>(&self.first);
    let second_receipt: i64 in Quantity<2> = admitted<Card, Descending, selected_second>(&self.second);
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
        write_output: false,
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
    let receipt_identity = lock
        .lines()
        .find(|line| line.contains("accepted fact: admitted"))
        .and_then(|line| line.split_once("  "))
        .map(|(identity, _)| identity)
        .expect("accepted template receipt identity");
    let report =
        std::fs::read_to_string(build_dir.join("trust_report.md")).expect("trust report written");
    let admitted_rows = report
        .lines()
        .filter(|line| line.starts_with("- accepted fact: admitted --"))
        .collect::<Vec<_>>();
    assert_eq!(
        admitted_rows.len(),
        1,
        "one universal template grant should produce one trust row:\n{report}"
    );
    assert!(
        admitted_rows[0].contains(&format!("accepted template: {receipt_identity}")),
        "the trust row must publish the exact receipt identity:\n{}",
        admitted_rows[0]
    );
    let instance_rows = report
        .lines()
        .filter(|line| line.starts_with("- accepted template: admitted ["))
        .collect::<Vec<_>>();
    assert_eq!(
        instance_rows.len(),
        2,
        "two selected machine contracts instantiate one universal grant:\n{report}"
    );
    assert!(
        instance_rows
            .iter()
            .all(|line| line.contains(&format!("admitted [{receipt_identity}]")))
    );
    assert_ne!(
        instance_rows[0], instance_rows[1],
        "distinct selected contracts must retain distinct instance closure rows"
    );
    assert!(
        instance_rows
            .iter()
            .all(|line| line.contains("type argument identities: named(name(Card))")),
        "each instance must retain its exact normalized type identity"
    );
    assert!(
        instance_rows
            .iter()
            .any(|line| line.contains("const argument identities: named(name(1))"))
            && instance_rows
                .iter()
                .any(|line| line.contains("const argument identities: named(name(2))")),
        "each instance must retain its exact normalized const identity"
    );
    assert!(
        instance_rows
            .iter()
            .all(|line| !line.contains("machine argument contracts: none")),
        "each instance must retain its selected machine contract identity"
    );
    assert!(
        instance_rows
            .iter()
            .all(|line| !line.ends_with("conformance arguments: none")),
        "each instance must retain its selected closed conformance identity"
    );
    let manifest = std::fs::read_to_string(build_dir.join("05_machine_contracts.json"))
        .expect("machine contract manifest written");
    let instance_contract_fingerprints = instance_rows
        .iter()
        .map(|line| {
            line.split_once("instance contract: ")
                .and_then(|(_, rest)| rest.split_once(" --"))
                .map(|(fingerprint, _)| fingerprint)
                .expect("generic accepted instance must render its exact checked contract")
        })
        .collect::<Vec<_>>();
    for fingerprint in instance_contract_fingerprints {
        assert!(
            manifest.contains(&format!("\"fingerprint\": \"0x{fingerprint}\"")),
            "the trust instance contract must be present verbatim in the machine-contract manifest:\n{manifest}"
        );
    }
    assert!(manifest.contains("\"accepted_template_commitment\": \"admitted\""));
    assert!(manifest.contains("\"type_argument_identities\": [\"named(name(Card))\"]"));
    assert!(manifest.contains("\"const_argument_identities\": [\"named(name(1))\"]"));
    assert!(manifest.contains("\"const_argument_identities\": [\"named(name(2))\"]"));
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
    let main_with = |slot: i64| {
        format!(
            r#"boundary trait Console {{ machine exit_process(return_code: i32); }}
boundary trait Flags {{
    machine open_read() -> i32;
}}
machine open_read() -> i32
    satisfies Flags::open_read via Binding::VtableSlot({slot});
data Main {{ console: Console; }}
machine Main::exercise(&mut self) {{
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
        write_output: false,
    };
    compile(options()).expect("granted plan project should compile");
    let lock = std::fs::read_to_string(project.join("omega.lock")).expect("lock written");
    assert!(
        lock.contains("provider slot: Flags"),
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
    // A bodyless external leaf derives a ProviderPlan; the plan surfaces as a
    // dev-active trust row (fingerprint shown) until the final build grants it
    // by name or trait leaf.
    let project = std::env::temp_dir().join(format!("omega-plan-rows-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"boundary trait Console { machine exit_process(return_code: i32); }
boundary trait Flags {
    machine open_read() -> i32;
}
machine open_read() -> i32
    satisfies Flags::open_read via Binding::VtableSlot(1);
data Main { console: Console; }
machine Main::exercise(&mut self) {
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
        write_output: false,
    })
    .expect("external-leaf project should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    assert!(
        report.contains("provider plan: satisfies::Flags ["),
        "expected the derived plan row with its fingerprint:\n{report}"
    );
    let plan_row = report
        .lines()
        .find(|line| line.contains("provider plan: satisfies::Flags"))
        .unwrap_or_default();
    assert!(
        plan_row.contains("own-package (dev-active)") && plan_row.contains("STANDING WARNING"),
        "an ungranted plan is dev-active with the warning:\n{report}"
    );
    assert!(
        report.contains("provider requirements: 1"),
        "the claim-free plan must still publish its exact requirement blast radius:\n{report}"
    );
    assert!(report.contains("generic accepted instances: 0"));
    let requirement_row = report
        .lines()
        .find(|line| {
            line.contains("provider plan: satisfies::Flags [")
                && line.contains("requirement identity:")
        })
        .expect("exact claim-free provider requirement row");
    assert!(requirement_row.contains("requirement owner: Flags"));
    assert!(requirement_row.contains("service schema: Flags"));
    assert!(requirement_row.contains("provider type: <free external>"));
    assert!(requirement_row.contains("target: <all>"));
    assert!(requirement_row.contains("calling plan: <none>"));
    assert!(requirement_row.contains("parameter types: <none>"));
    assert!(requirement_row.contains("result type: named(name(i32))"));
    assert!(requirement_row.contains("named-callable(path(Flags::open_read)"));
    assert!(requirement_row.contains("method: open_read"));
    assert!(requirement_row.contains("realization: vtable slot 1"));
    assert!(requirement_row.contains("grant selectors: none"));
    assert!(requirement_row.contains("STANDING WARNING"));

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn provider_requirement_rows_retain_exact_calling_plan_identity() {
    let project = std::env::temp_dir().join(format!(
        "omega-provider-calling-plan-trust-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"use omega::language::std::calling;

data NoResultPolicy {}
NoResultPolicyCallingPolicy: NoResultPolicy satisfies CallingPolicy;

machine NoResultPolicy::plan(signature: BoundarySignature) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    transition signature.has_result {
        true -> reject()
        _ -> accept()
    }

    state accept() -> BoundaryPlanResult {
        let mut output: BoundaryEntryPlan;
        output.call.stack_alignment = 16;
        BoundaryPlanResult::Accepted { plan: output }
    }

    state reject() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "return values are not supported",
            },
        }
    }
}

boundary trait Tick: Calling<NoResultPolicy> { machine tick(); }
machine tick_leaf() satisfies Tick::tick via Binding::VtableSlot(1);

data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");

    let checked = compile_to_checked(&project.join("main.omg"), None)
        .expect("calling-policy provider should check");
    let expected_selected_closure = checked.selected_provider_plans().normalized_identity();
    let tick = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Tick")
        .expect("Tick boundary trait");
    let schema = omega_effects::provider_plan::ServiceSchema::from_typed(&checked.typed, tick)
        .expect("Tick service schema");
    let method = &schema.methods[0];
    let expected = method
        .calling_plan_fingerprint
        .expect("evaluated calling-plan identity");
    assert!(method.parameter_type_identities.is_empty());
    assert_eq!(method.result_type_identity, None);

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: false,
    })
    .expect("calling-policy provider should compile");
    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    assert!(report.contains(&format!(
        "selected provider closure: {expected_selected_closure:016x}"
    )));
    let requirement = report
        .lines()
        .find(|line| {
            line.contains("provider plan: satisfies::Tick [")
                && line.contains("requirement identity:")
        })
        .expect("Tick provider requirement row");
    assert!(requirement.contains(&format!("calling plan: {expected:016x}")));
    assert!(requirement.contains("service schema: Tick"));
    assert!(requirement.contains("parameter types: <none>"));
    assert!(requirement.contains("result type: <none>"));

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn provider_requirement_rows_keep_operational_blast_radius_axes_independent() {
    let project = std::env::temp_dir().join(format!(
        "omega-provider-requirement-operational-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"boundary trait Clock { machine tick(); }
boundary trait Callback { machine call(); }
boundary trait Pair {
    machine effectful(callback: &mut Callback)
    reaches Clock
    invokes callback;
    suspends;
    blocks;
    terminates;

    machine quiet();
}

machine effectful_leaf(callback: &mut Callback)
    satisfies Pair::effectful via Binding::VtableSlot(1);
machine quiet_leaf()
    satisfies Pair::quiet via Binding::VtableSlot(2);

data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: false,
    })
    .expect("operational provider schema should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    let effectful = report
        .lines()
        .find(|line| {
            line.contains("provider plan: satisfies::Pair [") && line.contains("method: effectful")
        })
        .expect("effectful provider requirement row");
    assert!(effectful.contains("provider origin package: <none>"));
    assert!(effectful.contains("own-package (dev-active)"));
    assert!(effectful.contains("service reach: Callback, Clock, Pair"));
    assert!(effectful.contains("synchronous invocations: Callback"));
    assert!(effectful.contains("may suspend: yes"));
    assert!(effectful.contains("may block: yes"));
    assert!(effectful.contains("termination guarantee: yes"));

    let quiet = report
        .lines()
        .find(|line| {
            line.contains("provider plan: satisfies::Pair [") && line.contains("method: quiet")
        })
        .expect("quiet provider requirement row");
    assert!(quiet.contains("service reach: Pair"));
    assert!(quiet.contains("synchronous invocations: none"));
    assert!(quiet.contains("may suspend: no"));
    assert!(quiet.contains("may block: no"));
    assert!(quiet.contains("termination guarantee: no"));
    assert!(!quiet.contains("Clock"));
    assert!(!quiet.contains("Callback"));

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn provider_requirement_rows_retain_public_progress_premise_schemas() {
    let project = std::env::temp_dir().join(format!(
        "omega-provider-requirement-progress-premises-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"data SchedulerHandle { id: u64; }
domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;

boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}

boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair
    terminates;
}

machine wait_leaf(scheduler: SchedulerHandle)
    satisfies SchedulerRuntime::wait via Binding::VtableSlot(1);

data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: false,
    })
    .expect("progress-premised provider requirement should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    let requirement = report
        .lines()
        .find(|line| {
            line.contains("provider plan: satisfies::SchedulerRuntime [")
                && line.contains("method: wait")
        })
        .expect("wait provider requirement row");
    assert!(requirement.contains("termination guarantee: yes"));
    assert!(requirement.contains("progress premises: SchedulerHandle::WeakFair(parameter:0)"));

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn routed_qualification_rows_retain_exact_plan_claims_and_provenance() {
    let project = std::env::temp_dir().join(format!(
        "omega-routed-qualification-rows-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"data Token [linear] { id: u64; }
domain Token::Granted
requires
    self.id > 0
established by StorageEntry::enter;

domain Token::Issued
established by Issuer::issue;

boundary trait StorageEntry {
    machine enter(token: Token in Granted) -> Token;
}

data StorageEntryProvider {}
StorageEntryProviderStorageEntry: StorageEntryProvider satisfies StorageEntry;

machine StorageEntryProvider::enter(token: Token in Granted) -> Token
    satisfies StorageEntry::enter
{
    token as Token
}

boundary trait Issuer {
    machine issue(id: u64) -> Token in Issued
    ensures
        result in Token::Issued;
}

machine issue_leaf(id: u64) -> Token in Issued
    satisfies Issuer::issue
    via Binding::VtableSlot(1);

data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: false,
    })
    .expect("routed provider plans should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    assert!(
        report.contains("routed qualifications: 2"),
        "one predicate-bearing entry claim and one bodyless result claim should be reported:\n{report}"
    );

    let extent_plan = report
        .lines()
        .find(|line| {
            line.contains("provider plan: StorageEntryProvider::satisfies::StorageEntry [")
                && line.contains("coverage")
        })
        .expect("extent provider-plan commitment row");
    let extent_fingerprint = extent_plan
        .split('[')
        .nth(1)
        .and_then(|suffix| suffix.split(']').next())
        .expect("provider-plan fingerprint");
    assert!(extent_plan.contains("provider origin package: <none>"));
    let extent_rows = report
        .lines()
        .filter(|line| {
            line.contains("provider plan: StorageEntryProvider::satisfies::StorageEntry [")
                && line.contains("subject: parameter:")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        extent_rows.len(),
        1,
        "expected one routed parameter row:\n{report}"
    );
    let entry_row = extent_rows[0];
    assert!(entry_row.contains(&format!("[{extent_fingerprint}]")));
    assert!(entry_row.contains("provider type: StorageEntryProvider"));
    assert!(entry_row.contains("target: <all>"));
    assert!(entry_row.contains("provider origin package: <none>"));
    assert!(entry_row.contains("service schema: StorageEntry"));
    assert!(entry_row.contains("selected: yes"));
    assert!(entry_row.contains("requirement owner: StorageEntry"));
    assert!(entry_row.contains("requirement identity: named-callable(path(StorageEntry::enter)"));
    assert!(entry_row.contains("subject: parameter:0"));
    assert!(entry_row.contains("flow: accepts"));
    assert!(entry_row.contains("domain: Token::Granted"));
    assert!(
        entry_row.contains(
            "carry: carry(suspension: forbidden, cpu: same, thread: same, address: stable)"
        )
    );
    assert!(entry_row.contains("predicate discharge: required"));
    assert!(entry_row.contains("own-package (dev-active)"));
    assert!(entry_row.contains("grant selectors: none"));
    assert!(entry_row.contains("STANDING WARNING"));

    let result_row = report
        .lines()
        .find(|line| {
            line.contains("provider plan: satisfies::Issuer [") && line.contains("subject: result")
        })
        .expect("routed result row");
    assert!(result_row.contains("provider type: <free external>"));
    assert!(result_row.contains("target: <all>"));
    assert!(result_row.contains("provider origin package: <none>"));
    assert!(result_row.contains("service schema: Issuer"));
    assert!(result_row.contains("requirement owner: Issuer"));
    assert!(result_row.contains("selected: yes"));
    assert!(result_row.contains("requirement identity: named-callable(path(Issuer::issue)"));
    assert!(result_row.contains("result-dispatch(declared:Token::Issued)"));
    assert!(result_row.contains("flow: returns"));
    assert!(result_row.contains("domain: Token::Issued"));
    assert!(result_row.contains("predicate discharge: none"));

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn routed_qualification_rows_retain_exact_root_grant_selectors() {
    let project = std::env::temp_dir().join(format!(
        "omega-routed-qualification-grant-provenance-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("build.omg"),
        r#"data Subsystem { case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }
data Build { subsystem: Subsystem; freestanding: bool; }

machine build(b: &mut Build) {
    b.accept_boundary<Issuer>();
}
"#,
    )
    .expect("write build.omg");
    std::fs::write(
        project.join("main.omg"),
        r#"data Token [linear] { id: u64; }
domain Token::Issued
established by Issuer::issue;

boundary trait Issuer {
    machine issue(id: u64) -> Token in Issued
    ensures
        result in Token::Issued;
}

machine issue_leaf(id: u64) -> Token in Issued
    satisfies Issuer::issue
    via Binding::VtableSlot(1);

data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: false,
    })
    .expect("root-granted routed provider plan should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    let row = report
        .lines()
        .find(|line| {
            line.contains("provider plan: satisfies::Issuer [") && line.contains("subject: result")
        })
        .expect("routed result row");
    assert!(row.contains("root grant (build.omg)"));
    assert!(row.contains("grant selectors: Issuer"));
    assert!(!row.contains("STANDING WARNING"));

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
machine Main::exercise(&mut self) {
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
        write_output: false,
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
fn partial_provider_reports_only_bound_requirement_qualifications() {
    let project = std::env::temp_dir().join(format!(
        "omega-partial-provider-qualification-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"data Token [linear] { id: u64; }
domain Token::Bound
established by Pair::bound;
domain Token::Unbound
established by Pair::unbound;

boundary trait Pair {
    machine bound() -> Token in Bound
    ensures
        result in Token::Bound;

    machine unbound() -> Token in Unbound
    ensures
        result in Token::Unbound;
}

machine bound_leaf() -> Token in Bound
    satisfies Pair::bound via Binding::VtableSlot(1);

data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: false,
    })
    .expect("partial routed provider candidate should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    let plan_row = report
        .lines()
        .find(|line| line.contains("provider plan: satisfies::Pair ["))
        .expect("partial provider plan row");
    assert!(plan_row.contains("coverage 1/2"));
    assert!(plan_row.contains("selected: no"));
    assert!(report.contains("provider requirements: 1"));
    assert!(report.contains("routed qualifications: 1"));
    let qualification = report
        .lines()
        .find(|line| {
            line.contains("provider plan: satisfies::Pair [") && line.contains("subject: result")
        })
        .expect("bound result qualification row");
    assert!(qualification.contains("requirement identity: named-callable(path(Pair::bound)"));
    assert!(qualification.contains("provider type: <free external>"));
    assert!(qualification.contains("target: <all>"));
    assert!(qualification.contains("selected: no"));
    assert!(qualification.contains("domain: Token::Bound"));
    assert!(!report.contains("subject: result -- flow: returns -- domain: Token::Unbound"));
    assert!(!report.lines().any(|line| {
        line.contains("provider plan: satisfies::Pair [")
            && line.contains("requirement identity: named-callable(path(Pair::unbound)")
    }));

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
machine Main::exercise(&mut self) {
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
        write_output: false,
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

#[test]
fn slot_grant_pins_only_the_selected_provider_plan() {
    let project = std::env::temp_dir().join(format!(
        "omega-selected-provider-grant-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    let build_with = |provider: &str| {
        format!(
            r#"data Subsystem {{ case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }}
data Build {{ subsystem: Subsystem; freestanding: bool; }}

machine build(b: &mut Build) {{
    b.accept_boundary<Pair>();
    b.select_provider<Pair, {provider}>();
}}
"#
        )
    };
    std::fs::write(project.join("build.omg"), build_with("SecondProvider"))
        .expect("write build.omg");
    std::fs::write(
        project.join("main.omg"),
        r#"boundary trait Pair { machine choose() -> i32; }

data FirstProvider {}
FirstProviderPair: FirstProvider satisfies Pair;
machine FirstProvider::choose() -> i32 satisfies Pair::choose { 1 }

data SecondProvider {}
SecondProviderPair: SecondProvider satisfies Pair;
machine SecondProvider::choose() -> i32 satisfies Pair::choose { 2 }

data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: false,
    })
    .expect("explicitly selected granted provider should compile");

    let report =
        std::fs::read_to_string(build_dir.join("trust_report.md")).expect("trust report written");
    let first = report
        .lines()
        .find(|line| line.contains("provider plan: FirstProvider::satisfies::Pair ["))
        .expect("first candidate row");
    let second = report
        .lines()
        .find(|line| line.contains("provider plan: SecondProvider::satisfies::Pair ["))
        .expect("second candidate row");
    assert!(first.contains("own-package (dev-active)") && first.contains("STANDING WARNING"));
    assert!(second.contains("root grant (build.omg)") && !second.contains("STANDING WARNING"));
    assert!(first.contains("selected: no"));
    assert!(second.contains("selected: yes"));
    assert!(first.contains("provider type: FirstProvider"));
    assert!(first.contains("target: <all>"));
    assert!(second.contains("provider type: SecondProvider"));
    assert!(second.contains("target: <all>"));
    let first_requirement = report
        .lines()
        .find(|line| {
            line.contains("provider plan: FirstProvider::satisfies::Pair [")
                && line.contains("requirement identity:")
        })
        .expect("first candidate requirement row");
    let second_requirement = report
        .lines()
        .find(|line| {
            line.contains("provider plan: SecondProvider::satisfies::Pair [")
                && line.contains("requirement identity:")
        })
        .expect("selected requirement row");
    assert!(first_requirement.contains("requirement owner: Pair"));
    assert!(first_requirement.contains("provider type: FirstProvider"));
    assert!(first_requirement.contains("target: <all>"));
    assert!(first_requirement.contains("selected: no"));
    assert!(first_requirement.contains("realization: checked adapter `FirstProvider::choose`"));
    assert!(first_requirement.contains("grant selectors: none"));
    assert!(first_requirement.contains("STANDING WARNING"));
    assert!(second_requirement.contains("requirement owner: Pair"));
    assert!(second_requirement.contains("provider type: SecondProvider"));
    assert!(second_requirement.contains("target: <all>"));
    assert!(second_requirement.contains("selected: yes"));
    assert!(second_requirement.contains("realization: checked adapter `SecondProvider::choose`"));
    assert!(second_requirement.contains("grant selectors: Pair"));
    assert!(second_requirement.contains("root grant (build.omg)"));
    assert!(!second_requirement.contains("STANDING WARNING"));
    assert!(
        !report
            .lines()
            .any(|line| line.starts_with("- accepted fact: Pair --")),
        "the selected provider-slot grant must not be relabeled as a bare accepted fact:\n{report}"
    );

    let lock = std::fs::read_to_string(project.join("omega.lock")).expect("trust lock written");
    assert!(lock.contains("provider slot: Pair"));

    std::fs::write(project.join("build.omg"), build_with("FirstProvider"))
        .expect("rewrite build.omg");
    let message = format!(
        "{:?}",
        compile(CompileOptions {
            root_path: project.join("main.omg"),
            build_dir: Some(build_dir),
            target_name: None,
            write_output: false,
        })
        .expect_err("changing the selected provider must drift the slot receipt")
    );
    assert!(
        message.contains("granted statement drifted"),
        "expected selection drift refusal, got: {message}"
    );

    let _ = std::fs::remove_dir_all(&project);
}
