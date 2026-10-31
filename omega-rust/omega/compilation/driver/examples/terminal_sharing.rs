//! Measure real two-target production, outside parallel corpus counter traffic.
//!
//! `cargo run -p compiler --example terminal_sharing` compiles the existing
//! distinct-entry customer for Windows and Linux. Both native products must
//! borrow one Terminal allocation; this does not execute a foreign-host image.
//! Optional arguments select another corpus root followed by its target names.
//! Prefix `--check` or `--terminal` to measure those stops. `--reject TARGET`
//! expects that target to refuse while its siblings still produce their outputs.
//! `--reason TEXT` pins a different refusal than the default missing ProgramEntry.
//! `--run` also executes the host native product and its reloaded Terminal image.
//! `--expected-terminal-productions 1` measures a Check requiring shared scalar
//! evaluation; ordinary debt-free Check defaults to zero productions.
//! `--compare-single-targets` compares native bytes with independent requests.
//! `--assert-native-stage-counts` observes each target's real Omega 00-09 calls
//! and emission attempts; collection requires OMEGA_PHASE_PROFILE=1.
//! This does not claim numbered source/Psi invocation coverage.

#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[path = "../tests/support/joined_product_execution.rs"]
mod joined_product_execution;

use artifacts::compile_timings::counters;
use compiler::{CompileOutputKind, CompileRequest, RequestedCompileProduct};

fn main() {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--build-profile")) {
        println!("{}", compiler::compiler_build_profile());
        return;
    }
    let mut arguments = std::env::args().skip(1).peekable();
    let mut execute_host = false;
    let mut compare_single_targets = false;
    let mut assert_native_stage_counts = false;
    let mut product = RequestedCompileProduct::NativeArtifact;
    let mut rejected_target = None;
    let mut expected_terminal_productions = None;
    let mut rejection_reason = "ProgramEntry".to_owned();
    while arguments
        .peek()
        .is_some_and(|argument| argument.starts_with("--"))
    {
        match arguments.next().as_deref() {
            Some("--run") => execute_host = true,
            Some("--compare-single-targets") => compare_single_targets = true,
            Some("--assert-native-stage-counts") => assert_native_stage_counts = true,
            Some("--check") => product = RequestedCompileProduct::Check,
            Some("--terminal") => product = RequestedCompileProduct::TerminalArtifact,
            Some("--expected-terminal-productions") => {
                expected_terminal_productions = Some(
                    arguments
                        .next()
                        .expect("expected Terminal production count")
                        .parse::<u64>()
                        .expect("numeric Terminal production count"),
                );
            }
            Some("--reject") => rejected_target = Some(arguments.next().expect("rejected target")),
            Some("--reason") => rejection_reason = arguments.next().expect("refusal diagnostic"),
            flag => panic!("unknown measurement flag: {flag:?}"),
        }
    }
    let root = fixture_package_inputs::repo_root().join(arguments.next().unwrap_or_else(|| {
        "tests/omega/pass/targets/shared_program_distinct_roots_exit/main.omg".to_owned()
    }));
    let mut target_names: Vec<_> = arguments.collect();
    if target_names.is_empty() {
        target_names = vec!["windows_x86_64".to_owned(), "linux_x86_64".to_owned()];
    }
    // The standalone request normalizes target order before compilation.
    target_names.sort_by_key(|name| {
        target::TargetProfile::ALL
            .iter()
            .position(|profile| profile.target_name() == name)
    });
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &root,
        Some(
            target_names
                .iter()
                .find(|name| rejected_target.as_ref() != Some(name))
                .expect("one admitted target for fixture preparation"),
        ),
    )
    .expect("fixture package admission");
    let targets = target_names
        .iter()
        .map(|name| {
            Some(target::TargetProfile::from_omega_target_name(Some(name)).expect("target"))
        })
        .collect();
    let mut request = CompileRequest::for_targets(
        prepared.root_path.clone(),
        std::env::temp_dir().join(format!("omega-terminal-sharing-{}", std::process::id())),
        targets,
    )
    .with_requested_product(product);
    if let Some(inputs) = &prepared.package_inputs {
        request = request.with_package_inputs(inputs.clone());
    }
    if assert_native_stage_counts {
        assert!(
            phase_profile::enabled(),
            "stage observations require OMEGA_PHASE_PROFILE=1"
        );
        assert_eq!(product, RequestedCompileProduct::NativeArtifact);
    }
    let evidence_directory = assert_native_stage_counts.then(|| {
        fixture_package_inputs::repo_root().join(format!(
            "build/terminal-sharing-observations-{}",
            std::process::id()
        ))
    });
    let native_invocations_before = native_production::native_pipeline_invocations().len();
    let sources_before = counters::product_source_completion_count();
    let builds_before = counters::build_execution_count();
    let checks_before = counters::settled_check_totals().0;
    let terminals_before = counters::terminal_production_count();
    let outcomes =
        omega_build::compile_sources(&mut compiler::CompilationSession::default(), request)
            .expect("request admission")
            .into_outcomes();
    assert_eq!(outcomes.len(), target_names.len());
    let mut reports = Vec::new();
    let mut host_report = None;
    let mut rejections = 0;
    for (outcome, name) in outcomes.into_iter().zip(&target_names) {
        assert_eq!(
            outcome
                .target_profile()
                .map(|profile| profile.target_name()),
            Some(name.as_str())
        );
        let result = outcome.into_result();
        if rejected_target.as_ref() == Some(name) {
            let diagnostics = match result {
                Err(diagnostics) => diagnostics,
                Ok(_) => panic!("target {name} must refuse with {rejection_reason:?}"),
            };
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(&rejection_reason)),
                "expected {rejection_reason:?} refusal, got {diagnostics:?}"
            );
            rejections += 1;
        } else {
            if Some(name.as_str())
                == target::TargetProfile::host_if_supported().map(|profile| profile.target_name())
            {
                host_report = Some(reports.len());
            }
            reports.push(result.expect("requested product"));
        }
    }
    assert_eq!(rejections, usize::from(rejected_target.is_some()));
    assert!(!reports.is_empty());
    let expected_kind = match product {
        RequestedCompileProduct::Check => CompileOutputKind::CheckOnly,
        RequestedCompileProduct::TerminalArtifact => CompileOutputKind::TerminalArtifact,
        RequestedCompileProduct::NativeArtifact => CompileOutputKind::RetainedNativeArtifact,
    };
    for report in &reports {
        assert_eq!(report.output_kind(), expected_kind);
    }
    if product != RequestedCompileProduct::Check {
        let artifacts: Vec<_> = reports
            .iter()
            .map(|report| {
                report
                    .artifact()
                    .or_else(|| {
                        report
                            .retained_native_artifact()
                            .map(|native| native.psi_artifact())
                    })
                    .expect("portable artifact")
            })
            .collect();
        for artifact in &artifacts[1..] {
            assert!(
                std::ptr::eq(artifacts[0], *artifact),
                "products must retain one Terminal allocation"
            );
            assert_eq!(
                artifacts[0].manifest().semantic(),
                artifact.manifest().semantic()
            );
        }
    }
    let sources = counters::product_source_completion_count() - sources_before;
    let builds = counters::build_execution_count() - builds_before;
    let checks = counters::settled_check_totals().0 - checks_before;
    let terminals = counters::terminal_production_count() - terminals_before;
    assert_eq!(
        (sources, builds, checks, terminals),
        (
            1,
            1,
            2,
            expected_terminal_productions
                .unwrap_or_else(|| u64::from(product != RequestedCompileProduct::Check))
        )
    );
    println!(
        "product={product:?} products={} rejected={rejections} source_completions={sources} builds={builds} checks={checks} terminal_productions={terminals}",
        reports.len()
    );
    if assert_native_stage_counts {
        let invocations: Vec<_> = native_production::native_pipeline_invocations()
            .into_iter()
            .skip(native_invocations_before)
            .collect();
        for invocation in &invocations {
            println!("native_pipeline_invocation={invocation:?}");
            assert!(
                target_names
                    .iter()
                    .any(|name| name == invocation.profile.target_name())
            );
            assert_eq!(
                invocation.unit,
                native_production::NativePipelineUnit::Application,
                "additional callable pipeline observed in once-per-target fixture"
            );
            assert!(invocation.stages.iter().all(|count| *count <= 1));
        }
        for name in &target_names {
            let target_invocations: Vec<_> = invocations
                .iter()
                .filter(|invocation| invocation.profile.target_name() == name)
                .collect();
            if rejected_target.as_ref() == Some(name) {
                assert!(target_invocations.len() <= 1);
                assert!(
                    target_invocations
                        .iter()
                        .all(|invocation| !invocation.completed)
                );
                println!(
                    "target={name} rejected_native_invocations={}",
                    target_invocations.len()
                );
            } else {
                assert_eq!(
                    target_invocations.len(),
                    1,
                    "one native invocation for {name}"
                );
                let invocation = target_invocations[0];
                assert!(invocation.completed);
                assert_eq!(invocation.stages, [1; 10], "Omega 00-09 for {name}");
                assert_eq!(invocation.fragment_emission_attempts, 1);
                assert_eq!(invocation.image_emission_attempts, 1);
            }
        }
        let directory = evidence_directory.as_ref().expect("observation output");
        std::fs::create_dir_all(directory).expect("retain observation directory");
        std::fs::write(
            directory.join("invocations.txt"),
            invocations
                .iter()
                .map(|invocation| format!("{invocation:?}\n"))
                .collect::<String>(),
        )
        .expect("retain actual invocation counts");
        let admitted_targets = target_names
            .iter()
            .filter(|name| rejected_target.as_ref() != Some(*name));
        for (name, report) in admitted_targets.zip(&reports) {
            let retained = report.retained_native_artifact().expect("native product");
            std::fs::write(
                directory.join(format!("{name}-shared.image")),
                &retained.image().output().bytes,
            )
            .expect("retain shared-target image");
        }
        std::fs::write(
            directory.join("program.oterm"),
            reports[0]
                .retained_native_artifact()
                .expect("shared native product")
                .psi_artifact()
                .to_bytes(),
        )
        .expect("retain shared canonical Terminal artifact");
        println!("native_pipeline_evidence={}", directory.display());
    }
    // Measure the shared invocation before these independent compilations.
    // Allocation identity proves sharing; byte equality separately checks that
    // sharing has not changed either target's realized program.
    if compare_single_targets {
        assert_eq!(product, RequestedCompileProduct::NativeArtifact);
        let admitted_targets = target_names
            .iter()
            .filter(|name| rejected_target.as_ref() != Some(*name));
        for (name, shared) in admitted_targets.zip(&reports) {
            let profile = target::TargetProfile::from_omega_target_name(Some(name))
                .expect("single target profile");
            let mut request = CompileRequest::for_targets(
                prepared.root_path.clone(),
                std::env::temp_dir().join(format!(
                    "omega-terminal-sharing-{}-{name}-single",
                    std::process::id()
                )),
                vec![Some(profile)],
            )
            .with_requested_product(product);
            if let Some(inputs) = &prepared.package_inputs {
                request = request.with_package_inputs(inputs.clone());
            }
            let single =
                omega_build::compile_sources(&mut compiler::CompilationSession::default(), request)
                    .expect("single target admission")
                    .into_single_report()
                    .expect("single target production");
            let shared = &shared
                .retained_native_artifact()
                .expect("shared native")
                .image()
                .output()
                .bytes;
            let single = &single
                .retained_native_artifact()
                .expect("single native")
                .image()
                .output()
                .bytes;
            assert!(
                shared == single,
                "{name}: shared and single-target bytes differ"
            );
            if let Some(directory) = &evidence_directory {
                std::fs::write(directory.join(format!("{name}-single.image")), single)
                    .expect("retain independently compiled image");
            }
            println!("target={name} single_target_bytes_agree={}", shared.len());
        }
    }
    if execute_host {
        assert_eq!(product, RequestedCompileProduct::NativeArtifact);
        let report = reports.swap_remove(host_report.expect("one successful host target"));
        joined_product_execution::execute_report(
            report,
            &fixture_package_inputs::repo_root().join(format!(
                "build/shared-terminal-native-{}",
                std::process::id()
            )),
        );
        println!("host_native_exit=0 reloaded_terminal=Unit");
    }
}
