//! Legacy standalone trust-report coverage. Only exact accepted-machine and
//! selected-provider grants may create receipts; domains and unmatched strings
//! are not trust subjects.

use omega_compiler::{CompileOptions, compile_to_checked};

fn compile(
    options: CompileOptions,
) -> Result<omega_compiler::CompileReport, Vec<psi_diagnostics::Diagnostic>> {
    let admissions = omega_trust_ledger::read_trust_admissions(&options.root_path)?;
    let root_path = options.root_path.clone();
    let report = omega_compiler::compile(
        omega_compiler::CompileRequest::new(options).with_accepted_trust_admissions(admissions),
    )?;
    let settlement = report.trust_admission_settlement();
    if settlement.is_exactly_admitted() {
        assert!(!report.wrote_output());
        return Ok(report);
    }
    if !root_path
        .parent()
        .is_some_and(|project| project.join("omega.lock").exists())
    {
        omega_trust_ledger::accept_trust_admissions(&root_path, settlement.required())?;
        assert!(!report.wrote_output());
        return Ok(report);
    }
    let added = settlement
        .unresolved()
        .iter()
        .filter(|required| {
            !settlement
                .unused()
                .iter()
                .any(|accepted| accepted.commitment() == required.commitment())
        })
        .map(|row| row.commitment().to_owned())
        .collect::<Vec<_>>();
    let removed = settlement
        .unused()
        .iter()
        .filter(|accepted| {
            !settlement
                .unresolved()
                .iter()
                .any(|required| required.commitment() == accepted.commitment())
        })
        .map(|row| row.commitment().to_owned())
        .collect::<Vec<_>>();
    let changed = settlement
        .unresolved()
        .iter()
        .filter_map(|required| {
            settlement
                .unused()
                .iter()
                .find(|accepted| accepted.commitment() == required.commitment())
                .map(|accepted| {
                    format!(
                        "{} ({:016x} -> {:016x})",
                        required.commitment(),
                        accepted.identity(),
                        required.identity()
                    )
                })
        })
        .collect::<Vec<_>>();
    let display = |rows: &[String]| {
        if rows.is_empty() {
            "none".to_owned()
        } else {
            rows.join(", ")
        }
    };
    Err(vec![psi_diagnostics::Diagnostic::error(format!(
        "granted statement drifted: the complete trust receipt set no longer matches omega.lock -- added: {}; removed: {}; changed: {}",
        display(&added),
        display(&removed),
        display(&changed),
    ))])
}

#[test]
fn domain_declarations_do_not_create_trust_rows() {
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
    })
    .expect("plain program should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("the empty report is still written -- the honest no-commitments statement");
    let empty_selected_closure =
        omega_effects::SelectedProviderPlanFacts::default().normalized_identity();
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
    assert!(!project.join("omega.lock").exists());
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
    assert!(!project.join("omega.lock").exists());
    assert!(!build_dir.join("trust_report.md").exists());

    std::fs::write(project.join("build.omg"), build_with("First::claim"))
        .expect("write exact grant");
    compile(options()).expect("exact qualified grant should compile");

    let lock = std::fs::read_to_string(project.join("omega.lock")).expect("trust lock written");
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
            r#"boundary machine admitted() ensures {claim};
boundary trait Console {{ machine exit_process(return_code: i32); }}
data Main {{ console: Console; }}
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
    };
    compile(options()).expect("granted project should compile");

    let lock = std::fs::read_to_string(project.join("omega.lock"))
        .expect("omega.lock should be written beside build.omg");
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
    let lock_path = project.join("omega.lock");

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
        r#"machine build(builder: &mut Build) {
    builder.application("axiom-lock");
    builder.accept_boundary<mul_comm_axiom>();
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
        r#"machine build(builder: &mut Build) {
    builder.application("axiom-axis-lock");
    builder.accept_boundary<admitted_axis>();
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
            r#"boundary trait Console {{ machine exit_process(return_code: i32); }}
pub trait Ranked {{ machine Self::before(&self, other: &Self) -> bool; }}
data Card {{ rank: i32; }}
pub domain<T, const N: u64> T::Quantity<N>;
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
        admitted_rows[0].contains(&format!(
            "accepted template report fingerprint: {receipt_identity}"
        )),
        "the trust row must publish the exact receipt identity:\n{}",
        admitted_rows[0]
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
    assert!(
        instance_rows
            .iter()
            .all(|line| line.contains(&format!("template report fingerprint: {receipt_identity}")))
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
            manifest.contains(&format!("\"fingerprint\": \"0x{fingerprint}\"")),
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

#[test]
fn granted_plan_receipt_pins_the_fingerprint() {
    // PRV3: granting a derived plan pins its normalized identity in the
    // lockfile; changing the plan's policy under the grant drifts.
    let project = std::env::temp_dir().join(format!("omega-plan-lock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("plan-lock");
    builder.accept_boundary<Flags>();
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
            line.contains("provider plan: satisfies::Flags -- plan report fingerprint:")
                && line.contains("requirement identity:")
        })
        .expect("exact claim-free provider requirement row");
    assert!(requirement_row.contains("requirement owner: Flags"));
    assert!(requirement_row.contains("service schema: Flags"));
    assert!(requirement_row.contains("provider type: <free external>"));
    assert!(requirement_row.contains("target: <all>"));
    assert!(requirement_row.contains("calling plan report fingerprint: <none>"));
    assert!(requirement_row.contains("calling plan commitment: <none>"));
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
        .calling_plan_report_fingerprint
        .expect("evaluated calling-plan identity");
    assert!(method.parameter_type_identities.is_empty());
    assert_eq!(method.result_type_identity, None);

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
    })
    .expect("calling-policy provider should compile");
    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    assert!(report.contains(&format!(
        "selected provider closure report fingerprint: {expected_selected_closure:016x}"
    )));
    let requirement = report
        .lines()
        .find(|line| {
            line.contains("provider plan: satisfies::Tick -- plan report fingerprint:")
                && line.contains("requirement identity:")
        })
        .expect("Tick provider requirement row");
    assert!(requirement.contains(&format!("calling plan report fingerprint: {expected:016x}")));
    assert!(requirement.contains("calling plan commitment: 0x"));
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
    })
    .expect("operational provider schema should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    let effectful = report
        .lines()
        .find(|line| {
            line.contains("provider plan: satisfies::Pair -- plan report fingerprint:")
                && line.contains("method: effectful")
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
            line.contains("provider plan: satisfies::Pair -- plan report fingerprint:")
                && line.contains("method: quiet")
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
    })
    .expect("progress-premised provider requirement should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    let requirement = report
        .lines()
        .find(|line| {
            line.contains("provider plan: satisfies::SchedulerRuntime -- plan report fingerprint:")
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
            line.contains(
                "provider plan: StorageEntryProvider::satisfies::StorageEntry -- plan report fingerprint:",
            )
                && line.contains("subject: parameter:")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        extent_rows.len(),
        1,
        "expected one routed parameter row:\n{report}"
    );
    let entry_row = extent_rows[0];
    assert!(entry_row.contains(&format!("plan report fingerprint: {extent_fingerprint}")));
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
            line.contains("provider plan: satisfies::Issuer -- plan report fingerprint:")
                && line.contains("subject: result")
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
        r#"machine build(builder: &mut Build) {
    builder.application("trust-abstract-issuer");
    builder.accept_boundary<Issuer>();
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
    })
    .expect("root-granted routed provider plan should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    let row = report
        .lines()
        .find(|line| {
            line.contains("provider plan: satisfies::Issuer -- plan report fingerprint:")
                && line.contains("subject: result")
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
            line.contains("provider plan: satisfies::Pair -- plan report fingerprint:")
                && line.contains("subject: result")
        })
        .expect("bound result qualification row");
    assert!(qualification.contains("requirement identity: named-callable(path(Pair::bound)"));
    assert!(qualification.contains("provider type: <free external>"));
    assert!(qualification.contains("target: <all>"));
    assert!(qualification.contains("selected: no"));
    assert!(qualification.contains("domain: Token::Bound"));
    assert!(!report.contains("subject: result -- flow: returns -- domain: Token::Unbound"));
    assert!(!report.lines().any(|line| {
        line.contains("provider plan: satisfies::Pair -- plan report fingerprint:")
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
            r#"machine build(builder: &mut Build) {{
    builder.application("trust-provider-selection");
    builder.accept_boundary<Pair>();
    builder.select_provider<Pair, {provider}>();
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
            line.contains(
                "provider plan: FirstProvider::satisfies::Pair -- plan report fingerprint:",
            ) && line.contains("requirement identity:")
        })
        .expect("first candidate requirement row");
    let second_requirement = report
        .lines()
        .find(|line| {
            line.contains(
                "provider plan: SecondProvider::satisfies::Pair -- plan report fingerprint:",
            ) && line.contains("requirement identity:")
        })
        .expect("selected requirement row");
    assert!(first_requirement.contains("requirement owner: Pair"));
    assert!(first_requirement.contains("provider type: FirstProvider"));
    assert!(first_requirement.contains("target: <all>"));
    assert!(first_requirement.contains("selected: no"));
    assert!(first_requirement.contains(
        "realization: checked adapter `named-callable(path(FirstProvider::choose),parameters(),result-dispatch())`"
    ));
    assert!(first_requirement.contains("grant selectors: none"));
    assert!(first_requirement.contains("STANDING WARNING"));
    assert!(second_requirement.contains("requirement owner: Pair"));
    assert!(second_requirement.contains("provider type: SecondProvider"));
    assert!(second_requirement.contains("target: <all>"));
    assert!(second_requirement.contains("selected: yes"));
    assert!(second_requirement.contains(
        "realization: checked adapter `named-callable(path(SecondProvider::choose),parameters(),result-dispatch())`"
    ));
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
        })
        .expect_err("changing the selected provider must drift the slot receipt")
    );
    assert!(
        message.contains("granted statement drifted"),
        "expected selection drift refusal, got: {message}"
    );

    let _ = std::fs::remove_dir_all(&project);
}
