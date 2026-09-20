use super::compile;
use compiler::{CheckedCompileRequest, CompileOptions, compile_to_checked};

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
    satisfies Flags::open_read via Binding::Syscall({slot});
data Main {{ console: Service<Console>; }}
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
    satisfies Flags::open_read via Binding::Syscall(101);
data Main { console: Service<Console>; }
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
    assert!(requirement_row.contains("realization: syscall 101"));
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

pub boundary trait Tick: Calling<NoResultPolicy> { machine tick(); }
machine tick_leaf() satisfies Tick::tick via Binding::Syscall(102);

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
    satisfies Pair::effectful via Binding::Syscall(103);
machine quiet_leaf()
    satisfies Pair::quiet via Binding::Syscall(104);

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

pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}

pub boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair
    terminates;
}

machine wait_leaf(scheduler: SchedulerHandle)
    satisfies SchedulerRuntime::wait via Binding::Syscall(105);

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
    via Binding::Syscall(106);

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

pub boundary trait Issuer {
    machine issue(id: u64) -> Token in Issued
    ensures
        result in Token::Issued;
}

machine issue_leaf(id: u64) -> Token in Issued
    satisfies Issuer::issue
    via Binding::Syscall(106);

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
        r#"use omega::language::core::service;
pub boundary trait Console { machine exit_process(return_code: i32); }
pub boundary trait Pair {
    machine first(code: i32) -> i32;
    machine second(code: i32) -> i32;
}

machine first_leaf(code: i32) -> i32 satisfies Pair::first via Binding::Syscall(107);

data Main { console: Service<Console>; }
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

pub boundary trait Pair {
    machine bound() -> Token in Bound
    ensures
        result in Token::Bound;

    machine unbound() -> Token in Unbound
    ensures
        result in Token::Unbound;
}

machine bound_leaf() -> Token in Bound
    satisfies Pair::bound via Binding::Syscall(108);

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
        r#"use omega::language::core::service;
pub boundary trait Console { machine exit_process(return_code: i32); }
pub boundary trait Pair {
    machine first(code: i32) -> i32;
    machine second(code: i32) -> i32;
}

data FirstProvider { first: addr; }
machine FirstProvider::first(code: i32) -> i32
    satisfies Pair::first via Binding::VtableField(first);

data SecondProvider { second: addr; }
machine SecondProvider::second(code: i32) -> i32
    satisfies Pair::second via Binding::VtableField(second);

data Main { console: Service<Console>; }
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
