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
    let mut product = RequestedCompileProduct::NativeArtifact;
    let mut rejected_target = None;
    let mut rejection_reason = "ProgramEntry".to_owned();
    while arguments
        .peek()
        .is_some_and(|argument| argument.starts_with("--"))
    {
        match arguments.next().as_deref() {
            Some("--run") => execute_host = true,
            Some("--check") => product = RequestedCompileProduct::Check,
            Some("--terminal") => product = RequestedCompileProduct::TerminalArtifact,
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
        prepared.root_path,
        std::env::temp_dir().join(format!("omega-terminal-sharing-{}", std::process::id())),
        targets,
    )
    .with_requested_product(product);
    if let Some(inputs) = prepared.package_inputs {
        request = request.with_package_inputs(inputs);
    }
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
            u64::from(product != RequestedCompileProduct::Check)
        )
    );
    println!(
        "product={product:?} products={} rejected={rejections} source_completions={sources} builds={builds} checks={checks} terminal_productions={terminals}",
        reports.len()
    );
    if execute_host {
        assert_eq!(product, RequestedCompileProduct::NativeArtifact);
        let report = reports.swap_remove(host_report.expect("one successful host target"));
        joined_product_execution::execute_report(
            report,
            &fixture_package_inputs::repo_root().join("build/shared-terminal-native"),
        );
        println!("host_native_exit=0 reloaded_terminal=Unit");
    }
}
