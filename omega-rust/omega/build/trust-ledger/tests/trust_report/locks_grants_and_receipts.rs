use super::compile;
use compiler::{CheckedCompileRequest, CompileOptions, compile_to_checked};

#[test]
fn modern_package_lock_does_not_settle_fresh_compiler_obligations() {
    let project = std::env::temp_dir().join(format!(
        "omega-admissions-fresh-obligations-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("fresh-obligations");
    builder.accept_boundary<admitted>();
}
"#,
    )
    .unwrap();
    std::fs::write(
        project.join("main.omg"),
        "boundary machine admitted() ensures true;\ndata Main {}\nmachine Main::exercise(&mut self) {}\n",
    )
    .unwrap();
    let package_lock = b"OMEGA-PACKAGE-LOCK 1\n";
    let lock_path = project.join("omega.lock");
    std::fs::write(&lock_path, package_lock).unwrap();
    let root_path = project.join("main.omg");
    let check = || {
        let admissions = trust_ledger::read_trust_admissions(&root_path).unwrap();
        compiler::compile(
            compiler::CompileRequest::new(CompileOptions {
                root_path: root_path.clone(),
                build_dir: Some(project.join("build")),
                target_name: None,
            })
            .with_accepted_trust_admissions(admissions),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap()
    };
    // Ordinary coordinator checks must still reject this non-exact settlement.
    // Avoid the explicit-acceptance helper used by the older receipt tests.
    let fresh = check();
    assert!(!fresh.trust_admission_settlement().is_exactly_admitted());
    assert_eq!(fresh.trust_admission_settlement().unresolved().len(), 1);
    assert!(!fresh.wrote_output());
    assert!(!project.join("omega.admissions").exists());
    assert_eq!(std::fs::read(&lock_path).unwrap(), package_lock);

    trust_ledger::accept_trust_admissions(
        &root_path,
        fresh.trust_admission_settlement().required(),
    )
    .unwrap();
    assert!(check().trust_admission_settlement().is_exactly_admitted());
    assert_eq!(std::fs::read(&lock_path).unwrap(), package_lock);
    std::fs::remove_dir_all(project).unwrap();
}

#[test]
fn domain_declarations_do_not_create_trust_rows() {
    let project = std::env::temp_dir().join(format!("omega-trust-report-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"use omega::language::core::service;
domain u32::Meters;
pub boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Binding<Console>; }
machine Main::exercise(&mut self) reaches Console {
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
    })
    .expect("domain program should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    assert!(
        report.contains("admitted commitments: 0"),
        "a domain declaration is semantic structure, not a trust admission:\n{report}"
    );
    assert!(
        !report.contains("domain introduction:") && !report.contains("STANDING WARNING"),
        "domain declarations must not masquerade as grantable trust rows:\n{report}"
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
        r#"use omega::language::core::service;
pub boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Binding<Console>; }
machine Main::exercise(&mut self) reaches Console {
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
    })
    .expect("plain program should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("the empty report is still written -- the honest no-commitments statement");
    let empty_selected_closure = effects::SelectedProviderPlanFacts::default().report_fingerprint();
    assert!(report.contains(&format!(
        "selected provider closure report fingerprint: {empty_selected_closure:016x}"
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
        r#"pub boundary data Carrier;
pub boundary machine Carrier::combine(a: Carrier, b: Carrier) -> Carrier;
pub boundary trait AlgebraAudit {}
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

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.join("main.omg"), None))
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
        .report_fingerprint;

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
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
        "machine contract report fingerprint: {expected_contract_fingerprint:016x}"
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
fn domain_and_unmatched_root_grants_reject_without_receipts() {
    let project = std::env::temp_dir().join(format!("omega-trust-grant-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("trust-grant");
    builder.accept_boundary<Meters>();
    builder.accept_boundary<walker_lib::collatz_cert_checked>();
}
"#,
    )
    .expect("write build.omg");
    std::fs::write(
        project.join("main.omg"),
        r#"use omega::language::core::service;
domain u32::Meters;
pub boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Binding<Console>; }
machine Main::exercise(&mut self) reaches Console {
    let d: u32 in Meters = (7 as u32 in Meters);
    self.console.exit_process(70);
}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    let diagnostics = compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
    })
    .expect_err("domain and unmatched string grants must reject");
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("domain and arbitrary-string trust grants are unsupported"),
        "expected the retired legacy-grant diagnostic:\n{rendered}"
    );
    assert!(!project.join("omega.admissions").exists());
    assert!(!build_dir.join("trust_report.md").exists());

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
        r#"pub data First {}
pub data Second {}
boundary machine First::claim() ensures true;
boundary machine Second::claim() ensures true;
data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");
    let build_with = |grant: &str| {
        format!(
            r#"machine build(builder: &mut Build) {{
    builder.application("trust-grant-canonicalization");
    builder.accept_boundary<{grant}>();
}}
"#
        )
    };
    let build_dir = project.join("build");
    let options = || CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
    };

    std::fs::write(project.join("build.omg"), build_with("claim")).expect("write ambiguous grant");
    let diagnostics = compile(options()).expect_err("ambiguous short grant must reject");
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("root grant `claim` is ambiguous across non-provider trust subjects"),
        "expected exact grant ambiguity diagnostic:\n{rendered}",
    );
    assert!(!project.join("omega.admissions").exists());
    assert!(!build_dir.join("trust_report.md").exists());

    std::fs::write(project.join("build.omg"), build_with("First::claim"))
        .expect("write exact grant");
    compile(options()).expect("exact qualified grant should compile");

    let lock =
        std::fs::read_to_string(project.join("omega.admissions")).expect("trust lock written");
    assert!(lock.contains("accepted fact: First::claim"));
    assert!(!lock.contains("accepted fact: Second::claim"));
    let report =
        std::fs::read_to_string(build_dir.join("trust_report.md")).expect("trust report written");
    let granted = report
        .lines()
        .find(|line| line.contains("accepted fact: First::claim"))
        .expect("exact granted accepted-machine row");
    let foreign = report
        .lines()
        .find(|line| line.contains("accepted fact: Second::claim"))
        .expect("same-leaf foreign accepted-machine row");
    assert!(granted.contains("root grant (build.omg)"));
    assert!(!granted.contains("STANDING WARNING"));
    assert!(foreign.contains("own-package (dev-active)"));
    assert!(foreign.contains("STANDING WARNING"));

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn lockfile_written_and_drift_fails_until_reapproved() {
    // Legacy standalone accepted-machine grants still pin exact checked
    // contract identity until package-level admission replaces this lane.
    let project = std::env::temp_dir().join(format!("omega-trust-lock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("trust-lock");
    builder.accept_boundary<admitted>();
}
"#,
    )
    .expect("write build.omg");
    let main_with = |claim: &str| {
        let claim = if claim.trim().is_empty() {
            "true".to_owned()
        } else {
            claim.trim().to_owned()
        };
        format!(
            r#"use omega::language::core::service;
boundary machine admitted() ensures {claim};
pub boundary trait Console {{ machine exit_process(return_code: i32); }}
data Main {{ console: Binding<Console>; }}
machine Main::exercise(&mut self) reaches Console {{
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
    };
    compile(options()).expect("granted project should compile");

    let lock = std::fs::read_to_string(project.join("omega.admissions"))
        .expect("omega.admissions should be written beside build.omg");
    assert!(
        lock.contains("accepted fact: admitted"),
        "expected the accepted-machine receipt row:\n{lock}"
    );

    // Drift the granted statement -- the build must refuse.
    std::fs::write(project.join("main.omg"), main_with("false")).expect("rewrite main.omg");
    let drifted = compile(options());
    let message = format!("{:?}", drifted.expect_err("drift should refuse"));
    assert!(
        message.contains("granted statement drifted"),
        "expected the drift refusal, got: {message}"
    );

    // Re-approve by deleting the lock; the build succeeds and re-pins.
    std::fs::remove_file(project.join("omega.admissions")).expect("delete lock");
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
        r#"boundary machine Alpha() ensures true;
boundary machine Beta() ensures true;
data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");
    let build_with = |grants: &[&str]| {
        let grants = grants
            .iter()
            .map(|grant| format!("    builder.accept_boundary<{grant}>();"))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            r#"machine build(builder: &mut Build) {{
    builder.application("trust-lock-grants");
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
    };
    let lock_path = project.join("omega.admissions");

    std::fs::write(project.join("build.omg"), build_with(&["Alpha"])).expect("write first grant");
    compile(options()).expect("first approval writes one receipt");
    let one_receipt = std::fs::read_to_string(&lock_path).expect("read first receipt");

    std::fs::write(project.join("build.omg"), build_with(&["Beta", "Alpha"])).expect("add grant");
    let added = format!(
        "{:?}",
        compile(options()).expect_err("adding a grant requires reapproval")
    );
    assert!(added.contains("added: accepted fact: Beta"));
    assert_eq!(
        std::fs::read_to_string(&lock_path).expect("read preserved one-row lock"),
        one_receipt
    );

    std::fs::remove_file(&lock_path).expect("delete lock to approve added grant");
    compile(options()).expect("deleted lock reapproves complete two-row set");
    let two_receipts = std::fs::read_to_string(&lock_path).expect("read two receipts");
    let rows = two_receipts.lines().collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].ends_with("accepted fact: Alpha"));
    assert!(rows[1].ends_with("accepted fact: Beta"));

    std::fs::write(project.join("build.omg"), build_with(&["Beta"])).expect("remove first grant");
    let removed = format!(
        "{:?}",
        compile(options()).expect_err("removing a grant requires reapproval")
    );
    assert!(removed.contains("removed: accepted fact: Alpha"));
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
    assert!(empty.contains("removed: accepted fact: Beta"));
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
        r#"machine build(builder: &mut Build) { builder.application("trust-lock-corrupt"); builder.accept_boundary<Alpha>(); }
"#,
    )
    .expect("write build.omg");
    std::fs::write(
        project.join("main.omg"),
        r#"boundary machine Alpha() ensures true;
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
    };
    let lock_path = project.join("omega.admissions");
    compile(options()).expect("write canonical lock");
    let canonical = std::fs::read_to_string(&lock_path).expect("read canonical lock");

    let malformed = "not a v1 trust receipt\n";
    std::fs::write(&lock_path, malformed).expect("write malformed lock");
    let error = format!(
        "{:?}",
        compile(options()).expect_err("malformed lock must reject")
    );
    assert!(error.contains("malformed strong admission row"));
    assert_eq!(
        std::fs::read_to_string(&lock_path).expect("read preserved malformed lock"),
        malformed
    );

    let legacy = "0000000000000001  accepted fact: Alpha\n";
    std::fs::write(&lock_path, legacy).expect("write legacy compact lock");
    let error = format!(
        "{:?}",
        compile(options()).expect_err("legacy compact lock must not authorize")
    );
    assert!(error.contains("legacy 16-hex compact admission row"));
    assert!(error.contains("--accept-admissions"));
    assert_eq!(
        std::fs::read_to_string(&lock_path).expect("read preserved legacy lock"),
        legacy
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
        r#"machine build(builder: &mut Build) {
    builder.application("axiom-lock");
    builder.accept_boundary<mul_comm_axiom>();
}
"#,
    )
    .expect("write build.omg");
    let main_with = |claim: &str| {
        format!(
            r#"use omega::language::core::service;
use omega::language::core::nat;
pub boundary trait Console {{ machine exit_process(return_code: i32); }}
data Main {{ console: Binding<Console>; }}

boundary machine mul_comm_axiom(a: Nat, b: Nat) -> Nat
ensures
    {claim};

machine Main::exercise(&mut self) reaches Console {{
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
    };
    compile(options()).expect("granted axiom project should compile");
    let lock = std::fs::read_to_string(project.join("omega.admissions")).expect("lock written");
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
        r#"machine build(builder: &mut Build) {
    builder.application("axiom-axis-lock");
    builder.accept_boundary<admitted_axis>();
}
"#,
    )
    .expect("write build.omg");
    let main_with = |axis: &str| {
        format!(
            r#"use omega::language::core::service;
pub boundary trait Console {{ machine exit_process(return_code: i32); }}
data Main {{ console: Binding<Console>; }}

boundary machine admitted_axis()
{axis}
ensures true;

machine Main::exercise(&mut self) reaches Console {{
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
    };
    compile(options()).expect("granted axiom project should compile");
    let lock = std::fs::read_to_string(project.join("omega.admissions")).expect("lock written");
    assert!(lock.contains("accepted fact: admitted_axis"));
    let report =
        std::fs::read_to_string(build_dir.join("trust_report.md")).expect("trust report written");
    let admitted_row = report
        .lines()
        .find(|line| line.starts_with("- accepted fact: admitted_axis --"))
        .expect("nongeneric accepted row");
    assert!(
        !admitted_row.contains("accepted template report fingerprint:"),
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
        r#"machine build(builder: &mut Build) {
    builder.application("generic-axiom-lock");
    builder.accept_boundary<admitted>();
}
"#,
    )
    .expect("write build.omg");
    let main_with = |requirement_clause: &str| {
        format!(
            r#"use omega::language::core::service;
pub boundary trait Console {{ machine exit_process(return_code: i32); }}
pub trait Ranked {{ machine Self::before(&self, other: &Self) -> bool; }}
data Card {{ rank: i32; }}
pub domain<T, const N: u64> T::Quantity<N>;
Ascending: Card satisfies Ranked {{
    machine before(&self, other: &Card) -> bool {{ self.rank < other.rank }}
}}
Descending: Card satisfies Ranked {{
    machine before(&self, other: &Card) -> bool {{ self.rank > other.rank }}
}}
data Main {{ console: Binding<Console>; first: Card; second: Card; }}

machine selected_first(value: &Card) {{}}
machine selected_second(value: &Card) ensures true {{}}

boundary machine admitted<T, const N: u64, Order: T satisfies Ranked, machine F>(value: &T) -> i64 in Quantity<N>
where machine F(item: &T){requirement_clause};
ensures true;

machine Main::exercise(&mut self) reaches Console {{
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
    };
    compile(options()).expect("granted generic axiom project should compile");
    let lock = std::fs::read_to_string(project.join("omega.admissions")).expect("lock written");
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
    assert_eq!(receipt_identity.len(), 64);
    assert!(
        receipt_identity
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    );
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
    let template_report_identity = admitted_rows[0]
        .split_once("accepted template report fingerprint: ")
        .and_then(|(_, suffix)| suffix.split_whitespace().next())
        .expect("accepted template report coordinate");
    assert_eq!(template_report_identity.len(), 16);
    assert!(
        template_report_identity
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    );
    let instance_rows = report
        .lines()
        .filter(|line| line.starts_with("- accepted template: admitted --"))
        .collect::<Vec<_>>();
    assert_eq!(
        instance_rows.len(),
        2,
        "two selected machine contracts instantiate one universal grant:\n{report}"
    );
    assert!(instance_rows.iter().all(|line| line.contains(&format!(
        "template report fingerprint: {template_report_identity}"
    ))));
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
            .any(|line| line.contains("const argument identities: named(integer-const(1))"))
            && instance_rows
                .iter()
                .any(|line| line.contains("const argument identities: named(integer-const(2))")),
        "each instance must retain its exact normalized const identity"
    );
    assert!(
        instance_rows
            .iter()
            .all(|line| !line.contains("machine argument contract report fingerprints: none")),
        "each instance must retain its selected machine contract identity"
    );
    assert!(
        instance_rows
            .iter()
            .all(|line| !line.contains("conformance argument report fingerprints: none")),
        "each instance must retain its selected closed conformance identity"
    );
    let manifest = std::fs::read_to_string(build_dir.join("05_machine_contracts.json"))
        .expect("machine contract manifest written");
    let instance_contract_fingerprints = instance_rows
        .iter()
        .map(|line| {
            line.split_once("instance contract report fingerprint: ")
                .and_then(|(_, rest)| rest.split_once(" --"))
                .map(|(fingerprint, _)| fingerprint)
                .expect("generic accepted instance must render its exact checked contract")
        })
        .collect::<Vec<_>>();
    for fingerprint in instance_contract_fingerprints {
        assert!(
            manifest.contains(&format!("\"report_fingerprint\": \"0x{fingerprint}\"")),
            "the trust instance contract must be present verbatim in the machine-contract manifest:\n{manifest}"
        );
    }
    assert!(manifest.contains("\"accepted_template_commitment\": \"admitted\""));
    assert!(manifest.contains("\"type_argument_identities\": [\"named(name(Card))\"]"));
    assert!(manifest.contains("\"const_argument_identities\": [\"named(integer-const(1))\"]"));
    assert!(manifest.contains("\"const_argument_identities\": [\"named(integer-const(2))\"]"));
    assert!(manifest.contains("\"machine_argument_contract_report_fingerprints\": [\"0x"));

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
