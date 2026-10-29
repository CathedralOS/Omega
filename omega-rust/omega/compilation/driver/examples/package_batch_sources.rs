//! Measure mixed-source products from executed discovery, including preparation.
//!
//! Actual Build execution supplies the source graph; package admission supplies
//! Product plans. No fixture parser or second discovery pass is involved.
//! Run alone because work counters are process-wide.

use artifacts::compile_timings::counters;
use compilation_products::NativeProductRequest;
use compiler::{
    CompilationProduct, CompletedBuild, PackageCompilationRequest, PackageSourceRequest,
};
use omega_build::BuildSession;
use package_manager::operations::{
    ProjectCompilationAdmissionRequest, ProjectCompilationCompletion, admit_project_products,
    begin_local_project_preparation,
};
use std::path::Path;
use target::TargetProfile;
use terminal_psi_to_abstract_operations::provider_admission::current_terminal_authority_policy;

fn main() {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--build-profile")) {
        println!("{}", compiler::compiler_build_profile());
        return;
    }
    compiler::checked::run_on_compile_thread(|| {
        measure();
        Ok::<_, Vec<diagnostics::Diagnostic>>(())
    })
    .expect("batch worker");
}

fn measure() {
    let before = work_counts();
    let root = std::env::var_os("OMEGA_CORPUS_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(4)
                .expect("compiler repository")
                .to_path_buf()
        });
    let workspace = platform_custody::temporary_directory::TemporaryDirectory::create(
        "package-batch-sources",
        false,
    )
    .expect("batch workspace");
    let mut session = BuildSession::create(workspace.path()).expect("batch resources");
    let mut completed = Vec::new();
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--service-audits")) {
        let (mut requests, custody) = discover(
            &mut session,
            &mut completed,
            &root.join("tests/omega/pass/effects/root_service_audit/main.omg"),
            &[TargetProfile::host_if_supported().expect("Build host")],
            CompilationProduct::Check,
        );
        let build = omega_build::prepare_package(
            session.compiler(),
            requests.remove(0).occurrences,
            &mut completed,
        )
        .unwrap_or_else(|error| panic!("{:?}", error.failure.into_diagnostics()));
        let review = session
            .compiler()
            .review_package_build(build, usize::MAX)
            .expect("service audit review");
        let services = review.evidence.policy.service_audits();
        assert_eq!(
            services.len(),
            1,
            "duplicates collapse; untaken and dependency registrations stay absent"
        );
        assert_eq!(services[0].path(), "MotorControl");
        println!(
            "service_audits=1 selected=MotorControl duplicate_collapsed=true untaken_absent=true dependency_policy_absent=true"
        );
        drop(custody);
        session.close().expect("close audit resources");
        return;
    }
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--inspection-scalars")) {
        measure_inspection_scalars(&mut session, &mut completed, &root);
        measure_inspection_roots(&mut session, &mut completed, &root);
        session.close().expect("close batch resources");
        return;
    }
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--component-scalars")) {
        measure_component_scalars(&mut session, &mut completed, &root);
        session.close().expect("close batch resources");
        return;
    }
    let (mut first, first_custody) = discover(
        &mut session,
        &mut completed,
        &root.join("tests/omega/pass/targets/shared_program_distinct_roots_exit/main.omg"),
        &[TargetProfile::WindowsX64, TargetProfile::LinuxX64],
        native_product(),
    );
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--component-review")) {
        measure_component_review(&mut session, &mut completed, first.remove(0));
        drop(first_custody);
        session.close().expect("close batch resources");
        return;
    }
    let (mut second, second_custody) = discover(
        &mut session,
        &mut completed,
        &root.join("tests/omega/pass/optimizer/rollback_to_no_selection_empty_entry/main.omg"),
        &[TargetProfile::LinuxX64],
        native_product(),
    );
    assert_eq!(first.len(), 2);
    assert_eq!(second.len(), 1);
    reject_unsettled_inputs(&mut session, &mut completed, &first[0]);
    assert_eq!(
        work_since(before),
        (2, 2, 0, 0),
        "only discovery Builds have run"
    );
    let requests = vec![
        first.remove(0),
        PackageCompilationRequest {
            occurrences: Vec::new(),
            product: CompilationProduct::Check,
            terminal_permissions: None,
        },
        second.remove(0),
        first.remove(0),
    ];
    let prepared = work_counts();
    let mut outputs = omega_build::compile_packages(session.compiler(), requests, &mut completed);
    assert_eq!(outputs.len(), 4, "every request keeps its outcome");
    let rejected = match outputs.remove(1) {
        Err(error) => error,
        Ok(_) => panic!("a package with no root must reject"),
    };
    assert_eq!(rejected.occurrence, 0);
    assert!(
        rejected
            .failure
            .into_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message == "package compilation has no Product root")
    );
    let reports: Vec<_> = outputs
        .into_iter()
        .map(|result| {
            result
                .unwrap_or_else(|error| panic!("{:?}", error.failure.into_diagnostics()))
                .into_report()
                .unwrap_or_else(|error| panic!("{:?}", error.into_diagnostics()))
        })
        .collect();
    assert_eq!(
        work_since(prepared),
        (0, 2, 2, 2),
        "retained Builds are not executed or checked again"
    );
    assert_eq!(
        work_since(before),
        (2, 4, 2, 2),
        "one Build and one Product check per distinct source, including discovery"
    );
    let artifacts: Vec<_> = reports
        .iter()
        .map(|report| {
            report
                .retained_native_artifact()
                .expect("native product")
                .psi_artifact()
        })
        .collect();
    assert!(
        std::ptr::eq(artifacts[0], artifacts[2]),
        "equal source shares Terminal storage"
    );
    assert!(
        !std::ptr::eq(artifacts[0], artifacts[1]),
        "different sources stay distinct"
    );
    println!(
        "products=3 rejected_plans=1 batch_builds=0 batch_checks=2 whole_builds=2 whole_checks=4 sources=2 terminals=2 shared_storage=true"
    );
    drop((first_custody, second_custody));
    session.close().expect("close batch resources");
}

fn discover(
    session: &mut BuildSession,
    completed: &mut Vec<CompletedBuild>,
    entry: &Path,
    targets: &[TargetProfile],
    product: CompilationProduct,
) -> (Vec<PackageCompilationRequest>, ProjectCompilationCompletion) {
    let host = TargetProfile::host_if_supported().expect("Build host");
    let preparation = begin_local_project_preparation(entry, targets, true)
        .expect("prepare project")
        .expect("package project");
    let resolved = omega_build::resolve_project_sources(
        session,
        omega_build::ProjectSourceRequest {
            preparation,
            execution: package_compilation::BuildExecutionInputs {
                profile: Some(host),
                snapshot: None,
                ..Default::default()
            },
            collect_timings: false,
        },
    )
    .expect("execute actual source requests");
    for build in resolved.builds {
        let inputs = build.package_inputs().clone();
        completed.push(
            build
                .settle(session.compiler(), host, inputs)
                .unwrap_or_else(|error| panic!("{:?}", error.into_diagnostics())),
        );
    }
    let admitted = admit_project_products(
        ProjectCompilationAdmissionRequest {
            prepared: resolved.prepared,
            target_profiles: targets,
            snapshot: None,
            verify: false,
            timings: false,
            product,
        },
        session.resources(),
        host,
    );
    assert_eq!(
        admitted.requests.len(),
        targets.len(),
        "every requested target admitted"
    );
    assert!(
        admitted.requests.iter().all(|request| request
            .occurrences
            .iter()
            .all(|occurrence| matches!(occurrence, PackageSourceRequest::Product(_)))),
        "Product admission issues no execution requests"
    );
    (admitted.requests, admitted.completion)
}

fn work_counts() -> (u64, u64, u64, u64) {
    (
        counters::build_execution_count(),
        counters::settled_check_totals().0,
        counters::product_source_completion_count(),
        counters::terminal_production_count(),
    )
}

fn native_product() -> CompilationProduct {
    CompilationProduct::Native(NativeProductRequest {
        private_entry_ram: Vec::new(),
        terminal_authority_policy: current_terminal_authority_policy(),
        terminal_authority_permission_policy: None,
        optimization_rollback: Default::default(),
    })
}

fn reject_unsettled_inputs(
    session: &mut BuildSession,
    completed: &mut [CompletedBuild],
    request: &PackageCompilationRequest,
) {
    let copy_product = || {
        request
            .occurrences
            .iter()
            .map(|occurrence| {
                let PackageSourceRequest::Product(source) = occurrence else {
                    panic!("discovery must leave only Product inputs");
                };
                PackageSourceRequest::Product(compiler::PackageProductSourceRequest {
                    target: source.target,
                    inputs: source.inputs.clone(),
                    execution: source.execution.clone(),
                    options: compiler::PackageProductOptions {
                        optimization_rollback: source.options.optimization_rollback.clone(),
                    },
                })
            })
            .collect::<Vec<_>>()
    };
    let before = work_counts();
    let missing = omega_build::prepare_package(session.compiler(), copy_product(), &mut []);
    let Err(missing) = missing else {
        panic!("missing settled custody must reject");
    };
    assert!(
        missing
            .failure
            .into_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message == "Product request has no settled source Build")
    );
    let mut changed = copy_product();
    let Some(PackageSourceRequest::Product(root)) = changed.last_mut() else {
        panic!("Product root");
    };
    root.execution.snapshot = Some(package_compilation::BuildSnapshotRequest::new([
        b"late-required-output.omg".to_vec(),
    ]));
    let late = omega_build::prepare_package(session.compiler(), changed, completed);
    let Err(late) = late else {
        panic!("late execution obligations must reject");
    };
    assert!(late.failure.into_diagnostics().iter().any(|diagnostic|
        diagnostic.message == "Product preparation requires different Build execution inputs or output obligations"));
    assert_eq!(
        work_since(before),
        (0, 0, 0, 0),
        "refusals cannot replay Build or prepare Product"
    );
    println!("missing_custody=refused late_execution_obligation=refused refusal_work=0");
}

fn work_since(before: (u64, u64, u64, u64)) -> (u64, u64, u64, u64) {
    let after = work_counts();
    (
        after.0 - before.0,
        after.1 - before.1,
        after.2 - before.2,
        after.3 - before.3,
    )
}

fn measure_component_review(
    session: &mut BuildSession,
    completed: &mut [CompletedBuild],
    request: PackageCompilationRequest,
) {
    let build = omega_build::prepare_package(session.compiler(), request.occurrences, completed)
        .unwrap_or_else(|error| panic!("{:?}", error.failure.into_diagnostics()));
    let review = session
        .compiler()
        .review_package_build(build, usize::MAX)
        .expect("review product");
    let before = work_counts();
    let first = review
        .source
        .publish_component()
        .expect("first lazy component");
    let second = review
        .source
        .publish_component()
        .expect("second lazy component");
    assert_eq!(first, second, "same reviewed component bytes and identity");
    assert_eq!(first.package(), review.evidence.policy.package());
    assert_eq!(
        work_since(before),
        (0, 0, 0, 2),
        "publication retains checked source"
    );
    println!(
        "lazy_component_publications=2 equal_descriptions=true builds=0 checks=0 source_completions=0"
    );
}

/// A component cohort owes one portable production, including scalar checks.
/// These are unchanged authored fixtures; component publication gets no new
/// ProgramEntry authority and the false contract must still reject both targets.
fn measure_component_scalars(
    session: &mut BuildSession,
    completed: &mut Vec<CompletedBuild>,
    root: &Path,
) {
    let targets = [TargetProfile::WindowsX64, TargetProfile::MacosArm64];
    let mut observed = Vec::new();
    for (fixture, expected_failure) in [
        ("pass/layouts/runtime_constant_layout_contract_exit", None),
        (
            "fail/layouts/false_constant_layout_contract",
            Some("scalar predicate is false"),
        ),
        (
            "pass/build/component_declaration_scalar_conditions_exit",
            None,
        ),
        (
            "fail/build/component_declaration_scalar_conditions",
            Some("cause: Trap"),
        ),
    ] {
        let (requests, custody) = discover(
            session,
            completed,
            &root.join("tests/omega").join(fixture).join("main.omg"),
            &targets,
            native_product(),
        );
        let mut builds = Vec::new();
        for (target, request) in targets.into_iter().zip(requests) {
            let build =
                omega_build::prepare_package(session.compiler(), request.occurrences, completed)
                    .unwrap_or_else(|error| {
                        panic!("preparation: {:?}", error.failure.into_diagnostics())
                    });
            builds.push((target, build));
        }
        let before = work_counts();
        let outputs = session
            .compiler()
            .complete_package_components(builds)
            .expect("component batch");
        let counts = work_since(before);
        assert_eq!(outputs.len(), 2);
        let mut subjects = Vec::new();
        for (expected_target, (target, output)) in targets.into_iter().zip(outputs) {
            assert_eq!(target, expected_target, "target order and identity");
            match expected_failure {
                Some(reason) => {
                    let diagnostics = output.expect_err("false component contract must reject");
                    assert!(
                        diagnostics
                            .iter()
                            .any(|diagnostic| diagnostic.message.contains(reason)),
                        "{diagnostics:?}"
                    );
                }
                None => subjects.push(output.expect("valid component").expected_subject()),
            }
        }
        if expected_failure.is_none() {
            assert_eq!(
                subjects[0], subjects[1],
                "targets retain one canonical component identity"
            );
        }
        println!(
            "component_fixture={fixture} builds={} checks={} source_completions={} terminal_productions={} outcomes=2 rejected={}",
            counts.0,
            counts.1,
            counts.2,
            counts.3,
            if expected_failure.is_some() { 2 } else { 0 }
        );
        observed.push(counts);
        drop(custody);
    }
    for counts in observed {
        assert_eq!(counts.0, 0, "component publication must not replay Build");
        assert_eq!(
            counts.3, 1,
            "scalar checking and component publication share one Terminal production"
        );
    }
}

/// Inspection shares portable production without requiring a ProgramEntry.
fn measure_inspection_scalars(
    session: &mut BuildSession,
    completed: &mut Vec<CompletedBuild>,
    root: &Path,
) {
    let targets = [TargetProfile::WindowsX64, TargetProfile::MacosArm64];
    let mut observed = Vec::new();
    for (fixture, machine, expected_failure) in [
        (
            "pass/layouts/runtime_constant_layout_contract_exit",
            "program",
            None,
        ),
        (
            "fail/layouts/false_constant_layout_contract",
            "program",
            Some("scalar predicate is false"),
        ),
        (
            "pass/terminal_psi/inspection_scalar_library",
            "extent",
            None,
        ),
        (
            "fail/terminal_psi/inspection_scalar_library",
            "extent",
            Some("cause: Trap"),
        ),
    ] {
        let (requests, custody) = discover(
            session,
            completed,
            &root.join("tests/omega").join(fixture).join("main.omg"),
            &targets,
            CompilationProduct::Inspection(machine.to_owned()),
        );
        let before = work_counts();
        let outputs = omega_build::compile_packages(session.compiler(), requests, completed);
        let counts = work_since(before);
        assert_eq!(outputs.len(), 2);
        let mut inspected_programs = Vec::new();
        for output in outputs {
            match expected_failure {
                Some(reason) => {
                    let error = match output {
                        Err(error) => error,
                        Ok(_) => panic!("invalid inspection must reject"),
                    };
                    let diagnostics = error.failure.into_diagnostics();
                    assert!(
                        diagnostics
                            .iter()
                            .any(|diagnostic| diagnostic.message.contains(reason)),
                        "{diagnostics:?}"
                    );
                }
                None => {
                    let inspected = output
                        .unwrap_or_else(|error| panic!("{:?}", error.failure.into_diagnostics()))
                        .into_inspection()
                        .expect("inspection output");
                    assert!(!inspected.artifact().semantic_bytes().is_empty());
                    assert!(
                        inspected.applications().is_empty(),
                        "inspection grants no ProgramEntry receipt"
                    );
                    inspected_programs.push(inspected);
                }
            }
        }
        if expected_failure.is_none() {
            assert!(
                std::sync::Arc::ptr_eq(&inspected_programs[0], &inspected_programs[1]),
                "same-name inspection shares code and companions"
            );
        }
        println!(
            "inspection_fixture={fixture} builds={} checks={} source_completions={} terminal_productions={} outcomes=2 rejected={}",
            counts.0,
            counts.1,
            counts.2,
            counts.3,
            if expected_failure.is_some() { 2 } else { 0 }
        );
        observed.push(counts);
        drop(custody);
    }
    for counts in observed {
        assert_eq!(counts.0, 0, "inspection must not replay Build");
        assert_eq!(
            counts.3, 1,
            "inspection and scalar checks share one Terminal production"
        );
    }
}

fn measure_inspection_roots(
    session: &mut BuildSession,
    completed: &mut Vec<CompletedBuild>,
    root: &Path,
) {
    let targets = [
        TargetProfile::WindowsX64,
        TargetProfile::MacosArm64,
        TargetProfile::LinuxX64,
    ];
    let (mut requests, custody) = discover(
        session,
        completed,
        &root.join("tests/omega/pass/terminal_psi/debug_identity_complete_module/main.omg"),
        &targets,
        CompilationProduct::Inspection("constant".to_owned()),
    );
    requests[1].product = CompilationProduct::Inspection("state_graph".to_owned());
    let (mut missing_requests, missing_custody) = discover(
        session,
        completed,
        &root.join("tests/omega/pass/terminal_psi/debug_identity_complete_module/main.omg"),
        &[TargetProfile::LinuxArm64],
        CompilationProduct::Inspection("missing_inspection_name".to_owned()),
    );
    requests.insert(1, missing_requests.remove(0));
    let before = work_counts();
    let mut outputs = omega_build::compile_packages(session.compiler(), requests, completed);
    let counts = work_since(before);
    assert_eq!(outputs.len(), 4);
    let error = match outputs.remove(1) {
        Err(error) => error,
        Ok(_) => panic!("missing machine must reject at its original request position"),
    };
    assert!(
        error
            .failure
            .into_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("MachineNotFound")
                && diagnostic.message.contains("missing_inspection_name"))
    );
    let inspected = outputs
        .into_iter()
        .map(|output| {
            output
                .unwrap_or_else(|error| panic!("{:?}", error.failure.into_diagnostics()))
                .into_inspection()
                .expect("inspection product")
        })
        .collect::<Vec<_>>();
    assert_eq!(inspected.len(), 3);
    assert!(std::sync::Arc::ptr_eq(&inspected[0], &inspected[2]));
    assert!(!std::sync::Arc::ptr_eq(&inspected[0], &inspected[1]));
    assert_ne!(
        inspected[0].artifact().semantic_bytes(),
        inspected[1].artifact().semantic_bytes(),
        "different selected roots retain different semantics"
    );
    assert_eq!(counts.0, 0);
    assert_eq!(
        counts.3, 3,
        "two selected roots and one refused name attempt"
    );
    println!(
        "inspection_roots=3 missing_name=refused distinct_roots=2 terminal_productions=3 same_root_storage=true different_root_semantics=true"
    );
    drop((custody, missing_custody));
}
