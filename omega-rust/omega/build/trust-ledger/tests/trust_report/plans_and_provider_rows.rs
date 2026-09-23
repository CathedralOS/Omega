use super::compile;
use compiler::{CheckedCompilation, CheckedCompileRequest, CompileOptions, compile_to_checked};

/// `65dc530cef` stopped ordinary compilation from writing the trust Markdown,
/// so these tests reconstruct the same report from the checked program the way
/// `admit_checked_compilation` does. Each fact is unchanged; only its carrier
/// is, and the rows are the structure the renderer used to print.
fn trust_report(checked: &CheckedCompilation) -> artifacts::TrustReport {
    trust_model::reconstruct_trust_report(
        checked.terminal_production_trees(),
        checked.root_grants(),
        checked.provider_plans(),
        checked.selected_provider_plans(),
        checked.accepted_template_classifications(),
    )
    .expect("the trust report reconstructs from the checked program")
}

/// Checks the project's root and reconstructs its trust report in one step,
/// for the tests that only ever compiled to reach the report.
fn checked_trust_report(project: &std::path::Path, expectation: &str) -> artifacts::TrustReport {
    let checked = compile_to_checked(CheckedCompileRequest::new(&project.join("main.omg"), None))
        .unwrap_or_else(|diagnostics| panic!("{expectation}: {diagnostics:?}"));
    trust_report(&checked)
}

/// Commitment names in row order, for assertion messages that used to print the
/// whole rendered document.
fn commitments(report: &artifacts::TrustReport) -> Vec<&str> {
    report
        .rows
        .iter()
        .map(|row| row.commitment.as_str())
        .collect()
}

/// One provider-requirement row selected by plan name and requirement method.
fn requirement_row<'report>(
    report: &'report artifacts::TrustReport,
    provider_plan: &str,
    method: &str,
) -> &'report artifacts::TrustProviderRequirementRow {
    report
        .provider_requirements
        .iter()
        .find(|row| row.provider_plan == provider_plan && row.method == method)
        .unwrap_or_else(|| {
            panic!(
                "no `{provider_plan}` requirement row for `{method}`:\n{:#?}",
                report.provider_requirements
            )
        })
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
            r#"use omega::language::core::service;
pub boundary trait Console {{ machine exit_process(return_code: i32); }}
pub boundary trait Flags {{
    machine open_read() -> i32;
}}
machine open_read() -> i32
    satisfies Flags::open_read via ForeignBinding::Syscall({slot});
data Main {{ console: Binding<Console>; }}
machine Main::exercise(&mut self) reaches Console {{
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
    let lock = std::fs::read_to_string(project.join("omega.admissions")).expect("lock written");
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
        r#"use omega::language::core::service;
pub boundary trait Console { machine exit_process(return_code: i32); }
pub boundary trait Flags {
    machine open_read() -> i32;
}
machine open_read() -> i32
    satisfies Flags::open_read via ForeignBinding::Syscall(101);
data Main { console: Binding<Console>; }
machine Main::exercise(&mut self) reaches Console {
    self.console.exit_process(70);
}
"#,
    )
    .expect("write main.omg");

    let report = checked_trust_report(&project, "external-leaf project should compile");
    let plan_row = report
        .rows
        .iter()
        .find(|row| {
            row.commitment
                .starts_with("provider plan: satisfies::Flags [")
        })
        .unwrap_or_else(|| {
            panic!(
                "expected the derived plan row:\n{:#?}",
                report
                    .rows
                    .iter()
                    .map(|row| &row.commitment)
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(plan_row.provenance, "own-package (dev-active)");
    assert!(
        plan_row.standing_warning,
        "an ungranted plan is dev-active with the warning"
    );
    assert_eq!(
        report.provider_requirements.len(),
        1,
        "the claim-free plan must still publish its exact requirement blast radius:\n{:#?}",
        report.provider_requirements
    );
    assert!(report.generic_accepted_instances.is_empty());
    let requirement_row = requirement_row(&report, "satisfies::Flags", "open_read");
    assert_eq!(requirement_row.requirement_owner, "Flags");
    assert_eq!(requirement_row.service_schema, "Flags");
    assert_eq!(
        requirement_row.provider_type, "",
        "an empty provider type denotes a free external leaf"
    );
    assert_eq!(
        requirement_row.target, "",
        "an empty target denotes all targets"
    );
    assert_eq!(requirement_row.calling_plan_report_fingerprint, None);
    assert_eq!(requirement_row.calling_plan_commitment, None);
    assert!(requirement_row.parameter_type_identities.is_empty());
    assert_eq!(
        requirement_row.result_type_identity.as_deref(),
        Some("named(name(i32))")
    );
    assert!(
        requirement_row
            .requirement_identity
            .contains("named-callable(path(Flags::open_read)")
    );
    assert_eq!(
        requirement_row.realization,
        artifacts::TrustProviderRealization::Syscall { number: 101 }
    );
    assert!(requirement_row.grant_selectors.is_empty());
    assert!(requirement_row.standing_warning);

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

pub data NoResultPolicy {}
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

pub boundary trait Tick: Calling<NoResultPolicy> { machine tick(); }
machine tick_leaf() satisfies Tick::tick via ForeignBinding::Syscall(102);

data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.join("main.omg"), None))
        .expect("calling-policy provider should check");
    let expected_selected_closure = checked.selected_provider_plans().report_fingerprint();
    let tick = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Tick")
        .expect("Tick boundary trait");
    let schema = provider_planning::service_schema::from_typed(&checked.typed, tick)
        .expect("Tick service schema");
    let method = &schema.methods[0];
    let expected = method
        .calling_plan_report_fingerprint
        .expect("evaluated calling-plan identity");
    assert!(method.parameter_type_identities.is_empty());
    assert_eq!(method.result_type_identity, None);

    let report = trust_report(&checked);
    assert_eq!(
        report.selected_provider_closure_report_fingerprint,
        expected_selected_closure
    );
    let requirement = requirement_row(&report, "satisfies::Tick", &method.name);
    assert_eq!(
        requirement.calling_plan_report_fingerprint,
        Some(expected),
        "the requirement row carries the evaluated calling contract"
    );
    assert!(
        requirement.calling_plan_commitment.is_some(),
        "an evaluated calling plan retains its strong commitment"
    );
    assert_eq!(requirement.service_schema, "Tick");
    assert!(requirement.parameter_type_identities.is_empty());
    assert_eq!(requirement.result_type_identity, None);

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
        r#"pub boundary trait Clock { machine tick(); }
pub boundary trait Callback { machine call(); }
pub boundary trait Pair {
    machine effectful(callback: &mut Callback)
    reaches Clock
    invokes callback;
    suspends;
    blocks;
    terminates;

    machine quiet();
}

machine effectful_leaf(callback: &mut Callback)
    satisfies Pair::effectful via ForeignBinding::Syscall(103);
machine quiet_leaf()
    satisfies Pair::quiet via ForeignBinding::Syscall(104);

data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");

    let report = checked_trust_report(&project, "operational provider schema should compile");
    let effectful = requirement_row(&report, "satisfies::Pair", "effectful");
    assert_eq!(effectful.provider_origin_package_identity, None);
    assert_eq!(effectful.provenance, "own-package (dev-active)");
    assert_eq!(effectful.service_reach, ["Callback", "Clock", "Pair"]);
    assert_eq!(effectful.synchronous_invocations, ["Callback"]);
    assert!(effectful.may_suspend);
    assert!(effectful.may_block);
    assert!(effectful.terminates_guarantee);

    let quiet = requirement_row(&report, "satisfies::Pair", "quiet");
    assert_eq!(
        quiet.service_reach,
        ["Pair"],
        "the quiet requirement's reach is its own, not its sibling's"
    );
    assert!(quiet.synchronous_invocations.is_empty());
    assert!(!quiet.may_suspend);
    assert!(!quiet.may_block);
    assert!(!quiet.terminates_guarantee);

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
        r#"pub data SchedulerHandle { id: u64; }
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;

pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}

pub boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair
    terminates;
}

machine wait_leaf(scheduler: SchedulerHandle)
    satisfies SchedulerRuntime::wait via ForeignBinding::Syscall(105);

data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");

    let report = checked_trust_report(
        &project,
        "progress-premised provider requirement should compile",
    );
    let requirement = requirement_row(&report, "satisfies::SchedulerRuntime", "wait");
    assert!(requirement.terminates_guarantee);
    assert_eq!(
        requirement.termination_premises,
        vec![artifacts::TrustProgressPremiseRow {
            profile: "SchedulerHandle::WeakFair".to_owned(),
            subject: artifacts::TrustProgressPremiseSubject::Parameter(0),
            subject_projections: Vec::new(),
        }]
    );

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
        r#"pub data Token [linear] { id: u64; }
pub domain Token::Granted
requires
    self.id > 0
established by StorageEntry::enter;

pub domain Token::Issued
established by Issuer::issue;

pub boundary trait StorageEntry {
    machine enter(token: Token in Granted) -> Token;
}

data StorageEntryProvider {}
StorageEntryProviderStorageEntry: StorageEntryProvider satisfies StorageEntry;

machine StorageEntryProvider::enter(token: Token in Granted) -> Token
    satisfies StorageEntry::enter
{
    token as Token
}

pub boundary trait Issuer {
    machine issue(id: u64) -> Token in Issued
    ensures
        result in Token::Issued;
}

machine issue_leaf(id: u64) -> Token in Issued
    satisfies Issuer::issue
    via ForeignBinding::Syscall(106);

data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");

    let report = checked_trust_report(&project, "routed provider plans should compile");
    assert_eq!(
        report.qualifications.len(),
        2,
        "one predicate-bearing entry claim and one bodyless result claim should be reported:\n{:#?}",
        report.qualifications
    );

    let extent_plan = report
        .rows
        .iter()
        .find(|row| {
            row.commitment
                .starts_with("provider plan: StorageEntryProvider::satisfies::StorageEntry [")
        })
        .unwrap_or_else(|| {
            panic!(
                "extent provider-plan commitment row:\n{:#?}",
                commitments(&report)
            )
        });
    assert!(
        extent_plan
            .commitment
            .contains("provider origin package: <none>"),
        "{}",
        extent_plan.commitment
    );
    let extent_rows = report
        .qualifications
        .iter()
        .filter(|row| {
            row.provider_plan == "StorageEntryProvider::satisfies::StorageEntry"
                && row.subject.starts_with("parameter:")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        extent_rows.len(),
        1,
        "expected one routed parameter row:\n{:#?}",
        report.qualifications
    );
    let entry_row = extent_rows[0];
    assert_eq!(entry_row.provider_type, "StorageEntryProvider");
    assert_eq!(entry_row.target, "");
    assert_eq!(entry_row.provider_origin_package_identity, None);
    assert_eq!(entry_row.service_schema, "StorageEntry");
    assert!(entry_row.selected);
    assert_eq!(entry_row.requirement_owner, "StorageEntry");
    assert!(
        entry_row
            .requirement_identity
            .contains("named-callable(path(StorageEntry::enter)")
    );
    assert_eq!(entry_row.subject, "parameter:0");
    assert_eq!(entry_row.authority_flow, "accepts");
    assert_eq!(entry_row.domain, "Token::Granted");
    assert_eq!(
        entry_row.effective_carry,
        "carry(suspension: forbidden, cpu: same, thread: same, address: stable)"
    );
    assert!(entry_row.predicate_discharge_required);
    assert_eq!(entry_row.provenance, "own-package (dev-active)");
    assert!(entry_row.grant_selectors.is_empty());
    assert!(entry_row.standing_warning);

    let result_row = report
        .qualifications
        .iter()
        .find(|row| row.provider_plan == "satisfies::Issuer" && row.subject == "result")
        .unwrap_or_else(|| panic!("routed result row:\n{:#?}", report.qualifications));
    assert_eq!(
        result_row.provider_type, "",
        "an empty provider type denotes a free external leaf"
    );
    assert_eq!(result_row.target, "");
    assert_eq!(result_row.provider_origin_package_identity, None);
    assert_eq!(result_row.service_schema, "Issuer");
    assert_eq!(result_row.requirement_owner, "Issuer");
    assert!(result_row.selected);
    assert!(
        result_row
            .requirement_identity
            .contains("named-callable(path(Issuer::issue)")
    );
    assert!(
        result_row
            .requirement_identity
            .contains("result-dispatch(declared:Token::Issued)")
    );
    assert_eq!(result_row.authority_flow, "returns");
    assert_eq!(result_row.domain, "Token::Issued");
    assert!(!result_row.predicate_discharge_required);

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
        r#"pub data Token [linear] { id: u64; }
pub domain Token::Issued
established by Issuer::issue;

pub boundary trait Issuer {
    machine issue(id: u64) -> Token in Issued
    ensures
        result in Token::Issued;
}

machine issue_leaf(id: u64) -> Token in Issued
    satisfies Issuer::issue
    via ForeignBinding::Syscall(106);

data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");

    let report = checked_trust_report(&project, "root-granted routed provider plan should compile");
    let row = report
        .qualifications
        .iter()
        .find(|row| row.provider_plan == "satisfies::Issuer" && row.subject == "result")
        .unwrap_or_else(|| panic!("routed result row:\n{:#?}", report.qualifications));
    assert_eq!(row.provenance, "root grant (build.omg)");
    assert_eq!(row.grant_selectors, ["Issuer"]);
    assert!(!row.standing_warning);

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
        r#"use omega::language::core::service;
pub boundary trait Console { machine exit_process(return_code: i32); }
pub boundary trait Pair {
    machine first(code: i32) -> i32;
    machine second(code: i32) -> i32;
}

machine first_leaf(code: i32) -> i32 satisfies Pair::first via ForeignBinding::Syscall(107);

data Main { console: Binding<Console>; }
machine Main::exercise(&mut self) reaches Console {
    self.console.exit_process(70);
}
"#,
    )
    .expect("write main.omg");

    let report = checked_trust_report(&project, "satisfies-leaf project should compile");
    let row = report
        .rows
        .iter()
        .find(|row| {
            row.commitment
                .starts_with("provider plan: satisfies::Pair [")
        })
        .unwrap_or_else(|| {
            panic!(
                "expected the satisfies-derived plan row:\n{:#?}",
                commitments(&report)
            )
        });
    assert!(
        row.commitment.contains("coverage 1/2"),
        "one of two requirements satisfied -> coverage 1/2: {}",
        row.commitment
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
        r#"pub data Token [linear] { id: u64; }
pub domain Token::Bound
established by Pair::bound;
pub domain Token::Unbound
established by Pair::unbound;

pub boundary trait Pair {
    machine bound() -> Token in Bound
    ensures
        result in Token::Bound;

    machine unbound() -> Token in Unbound
    ensures
        result in Token::Unbound;
}

machine bound_leaf() -> Token in Bound
    satisfies Pair::bound via ForeignBinding::Syscall(108);

data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write main.omg");

    let report = checked_trust_report(&project, "partial routed provider candidate should compile");
    let plan_row = report
        .rows
        .iter()
        .find(|row| {
            row.commitment
                .starts_with("provider plan: satisfies::Pair [")
        })
        .unwrap_or_else(|| panic!("partial provider plan row:\n{:#?}", commitments(&report)));
    assert!(plan_row.commitment.contains("coverage 1/2"));
    assert!(plan_row.commitment.contains("selected: no"));
    assert_eq!(report.provider_requirements.len(), 1);
    assert_eq!(report.qualifications.len(), 1);
    let qualification = report
        .qualifications
        .iter()
        .find(|row| row.provider_plan == "satisfies::Pair" && row.subject == "result")
        .unwrap_or_else(|| {
            panic!(
                "bound result qualification row:\n{:#?}",
                report.qualifications
            )
        });
    assert!(
        qualification
            .requirement_identity
            .contains("named-callable(path(Pair::bound)")
    );
    assert_eq!(
        qualification.provider_type, "",
        "an empty provider type denotes a free external leaf"
    );
    assert_eq!(qualification.target, "");
    assert!(!qualification.selected);
    assert_eq!(qualification.domain, "Token::Bound");
    assert!(
        report
            .qualifications
            .iter()
            .all(|row| row.domain != "Token::Unbound"),
        "the unbound requirement has no provider, so it routes nothing:\n{:#?}",
        report.qualifications
    );
    assert!(
        report.provider_requirements.iter().all(|row| !row
            .requirement_identity
            .contains("named-callable(path(Pair::unbound)")),
        "an unbound requirement publishes no blast radius:\n{:#?}",
        report.provider_requirements
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
        r#"use omega::language::core::service;
pub boundary trait Console { machine exit_process(return_code: i32); }
pub boundary trait Pair {
    machine first(code: i32) -> i32;
    machine second(code: i32) -> i32;
}

data FirstProvider { first: addr; }
machine FirstProvider::first(code: i32) -> i32
    satisfies Pair::first via ForeignBinding::VtableField(first);

data SecondProvider { second: addr; }
machine SecondProvider::second(code: i32) -> i32
    satisfies Pair::second via ForeignBinding::VtableField(second);

data Main { console: Binding<Console>; }
machine Main::exercise(&mut self) reaches Console {
    self.console.exit_process(70);
}
"#,
    )
    .expect("write main.omg");

    let report = checked_trust_report(
        &project,
        "separate partial provider candidates should compile",
    );
    for provider in ["FirstProvider", "SecondProvider"] {
        let prefix = format!("provider plan: {provider}::satisfies::Pair [");
        let row = report
            .rows
            .iter()
            .find(|row| row.commitment.starts_with(&prefix))
            .unwrap_or_else(|| {
                panic!("missing {provider} candidate:\n{:#?}", commitments(&report))
            });
        assert!(
            row.commitment.contains("coverage 1/2"),
            "{provider} must remain a half-provider: {}",
            row.commitment
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
        r#"pub boundary trait Pair { machine choose() -> i32; }

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

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.join("main.omg"), None))
        .expect("explicitly selected granted provider should check");
    let report = trust_report(&checked);
    let plan = |provider: &str| {
        let prefix = format!("provider plan: {provider}::satisfies::Pair [");
        report
            .rows
            .iter()
            .find(|row| row.commitment.starts_with(&prefix))
            .unwrap_or_else(|| panic!("{provider} candidate row:\n{:#?}", commitments(&report)))
    };
    let first = plan("FirstProvider");
    let second = plan("SecondProvider");
    assert_eq!(first.provenance, "own-package (dev-active)");
    assert!(first.standing_warning);
    assert_eq!(second.provenance, "root grant (build.omg)");
    assert!(!second.standing_warning);
    assert!(first.commitment.contains("selected: no"));
    assert!(second.commitment.contains("selected: yes"));
    assert!(first.commitment.contains("provider type: FirstProvider"));
    assert!(first.commitment.contains("target: <all>"));
    assert!(second.commitment.contains("provider type: SecondProvider"));
    assert!(second.commitment.contains("target: <all>"));

    let requirement = |provider: &str| {
        let plan = format!("{provider}::satisfies::Pair");
        report
            .provider_requirements
            .iter()
            .find(|row| row.provider_plan == plan)
            .unwrap_or_else(|| {
                panic!(
                    "{provider} requirement row:\n{:#?}",
                    report.provider_requirements
                )
            })
    };
    let first_requirement = requirement("FirstProvider");
    let second_requirement = requirement("SecondProvider");
    let adapter = |provider: &str| artifacts::TrustProviderRealization::CheckedAdapter {
        machine_identity: format!(
            "named-callable(path({provider}::choose),parameters(),result-dispatch())"
        ),
        machine_package_identity: None,
    };
    assert_eq!(first_requirement.requirement_owner, "Pair");
    assert_eq!(first_requirement.provider_type, "FirstProvider");
    assert_eq!(first_requirement.target, "");
    assert!(!first_requirement.selected);
    assert_eq!(first_requirement.realization, adapter("FirstProvider"));
    assert!(first_requirement.grant_selectors.is_empty());
    assert!(first_requirement.standing_warning);
    assert_eq!(second_requirement.requirement_owner, "Pair");
    assert_eq!(second_requirement.provider_type, "SecondProvider");
    assert_eq!(second_requirement.target, "");
    assert!(second_requirement.selected);
    assert_eq!(second_requirement.realization, adapter("SecondProvider"));
    assert_eq!(second_requirement.grant_selectors, ["Pair"]);
    assert_eq!(second_requirement.provenance, "root grant (build.omg)");
    assert!(!second_requirement.standing_warning);
    assert!(
        !report
            .rows
            .iter()
            .any(|row| row.commitment.starts_with("accepted fact: Pair")),
        "the selected provider-slot grant must not be relabeled as a bare accepted fact:\n{:#?}",
        commitments(&report)
    );

    let lock =
        std::fs::read_to_string(project.join("omega.admissions")).expect("trust lock written");
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
