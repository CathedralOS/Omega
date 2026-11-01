//! Assert whole-invocation work through the public application Build API.
//!
//! Run alone: counters are process-wide. The measurement starts before source
//! preparation and ends after every requested outcome joins, including refusal.
//! Cross-target artifacts are produced but foreign native images are not executed.
//! Terminal mode interprets each target context and requires an empty Unit result.
//!
//! Usage: package_batch [--check | --terminal] [--targets name,name]
//! [--rollback path] [--native-evidence] [--compare-single-targets] [--run]
//! [--reject target --reason text] [root [sources
//! [terminals [builds [settled_checks]]]]]. Defaults describe the two-target
//! shared_program_distinct_roots_exit fixture: 1 source, 1 Terminal, 1 Build,
//! and 2 checks (Build + Product). Check-only defaults to zero Terminal work.

use artifacts::compile_timings::counters;
use omega_build::{BuildRequest, BuildTargets, NativeDestination, ProjectProduct, build};

#[derive(Debug, PartialEq, Eq)]
struct WorkCounts {
    sources: u64,
    terminals: u64,
    builds: u64,
    settled_checks: u64,
}

impl WorkCounts {
    fn snapshot() -> Self {
        Self {
            sources: counters::product_source_completion_count(),
            terminals: counters::terminal_production_count(),
            builds: counters::build_execution_count(),
            settled_checks: counters::settled_check_totals().0,
        }
    }

    fn since(before: &Self) -> Self {
        let after = Self::snapshot();
        Self {
            sources: after.sources - before.sources,
            terminals: after.terminals - before.terminals,
            builds: after.builds - before.builds,
            settled_checks: after.settled_checks - before.settled_checks,
        }
    }
}

struct Request {
    root: std::path::PathBuf,
    targets: Vec<target::TargetProfile>,
    product: Product,
    rollback: compiler::OptimizationRollback,
    expected: WorkCounts,
    native_evidence: bool,
    compare_single_targets: bool,
    run_host: bool,
    rejected_target: Option<target::TargetProfile>,
    rejection_reason: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Product {
    Check,
    Terminal,
    Native,
}

impl Product {
    fn outcome(self) -> &'static str {
        match self {
            Self::Check => "checked",
            Self::Terminal => "terminal",
            Self::Native => "native",
        }
    }
}

fn main() -> Result<(), String> {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--build-profile")) {
        println!("{}", compiler::compiler_build_profile());
        return Ok(());
    }
    // Match the shipped command worker: resolution precedes the compiler's
    // worker and still contains recursive tree walks.
    std::thread::Builder::new()
        .name("package-batch-measurement".to_owned())
        .stack_size(256 * 1024 * 1024)
        .spawn(|| measure().map_err(|error| error.to_string()))
        .map_err(|error| error.to_string())?
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

fn request() -> Result<Request, Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1).peekable();
    let mut product = Product::Native;
    let mut target_names = "windows_x86_64,linux_x86_64".to_owned();
    let mut rollback = Default::default();
    let mut native_evidence = false;
    let mut compare_single_targets = false;
    let mut run_host = false;
    let mut rejected_target = None;
    let mut rejection_reason = "ProgramEntry".to_owned();
    while arguments
        .peek()
        .is_some_and(|argument| argument.starts_with("--"))
    {
        match arguments.next().as_deref() {
            Some("--check") => product = Product::Check,
            Some("--terminal") => product = Product::Terminal,
            Some("--targets") => {
                target_names = arguments.next().ok_or("--targets requires names")?
            }
            Some("--rollback") => {
                let path = arguments.next().ok_or("--rollback requires a file")?;
                let names = std::fs::read_to_string(path)?;
                rollback = compiler::OptimizationRollback::from_exact_names(
                    names.lines().map(str::trim).filter(|name| !name.is_empty()),
                )?;
            }
            Some("--native-evidence") => native_evidence = true,
            Some("--compare-single-targets") => compare_single_targets = true,
            Some("--run") => run_host = true,
            Some("--reject") => {
                let name = arguments.next().ok_or("--reject requires a target")?;
                rejected_target = Some(
                    target::TargetProfile::from_canonical_target_name(&name)
                        .map_err(|error| error.message)?,
                );
            }
            Some("--reason") => {
                rejection_reason = arguments.next().ok_or("--reason requires text")?
            }
            flag => return Err(format!("unknown measurement flag: {flag:?}").into()),
        }
    }
    if (native_evidence || compare_single_targets || run_host) && product != Product::Native {
        return Err("native observations require the native product".into());
    }
    if native_evidence && !phase_profile::enabled() {
        return Err("native invocation observations require OMEGA_PHASE_PROFILE=1".into());
    }
    let root_argument = arguments.next();
    let root = root_argument
        .unwrap_or_else(|| {
            "tests/omega/pass/targets/shared_program_distinct_roots_exit/main.omg".to_owned()
        })
        .into();
    let targets = target::ExplicitTargetSet::from_caller_names(target_names.split(','))
        .map_err(|diagnostics| format!("{diagnostics:?}"))?
        .profiles()
        .to_vec();
    let mut expected_count = |default| -> Result<u64, std::num::ParseIntError> {
        Ok(arguments
            .next()
            .map(|value| value.parse())
            .transpose()?
            .unwrap_or(default))
    };
    let expected = WorkCounts {
        sources: expected_count(1)?,
        terminals: expected_count(u64::from(product != Product::Check))?,
        builds: expected_count(1)?,
        settled_checks: expected_count(2)?,
    };
    if arguments.next().is_some() {
        return Err(
            "expected at most root and source, Terminal, Build and settled-check counts".into(),
        );
    }
    Ok(Request {
        root,
        targets,
        product,
        rollback,
        expected,
        native_evidence,
        compare_single_targets,
        run_host,
        rejected_target,
        rejection_reason,
    })
}

fn measure() -> Result<(), Box<dyn std::error::Error>> {
    let before = WorkCounts::snapshot();
    let request = request()?;
    let native_before = native_production::native_pipeline_invocations().len();
    let result = produce(&request);
    let actual = WorkCounts::since(&before);
    println!(
        "whole_invocation_builds={} whole_invocation_settled_checks={} source_completions={} terminal_productions={}",
        actual.builds, actual.settled_checks, actual.sources, actual.terminals
    );
    // Assert on failures too: an unrelated early refusal must not count as
    // checking the invalid unrequested row. Preserve its original diagnostic.
    assert_eq!(
        actual,
        request.expected,
        "whole-invocation work differs; API failure: {:?}",
        result.as_ref().err()
    );
    let reports = result?;
    if request.product == Product::Native {
        observe_native_products(&request, reports, native_before)?;
    }
    Ok(())
}

fn build_targets(
    request: &Request,
    profiles: &[target::TargetProfile],
    suffix: &str,
) -> Result<Vec<omega_build::TargetBuildOutcome>, Box<dyn std::error::Error>> {
    let directory = std::env::temp_dir().join(format!(
        "omega-package-batch-{}-{suffix}",
        std::process::id()
    ));
    let targets = target::ExplicitTargetSet::from_caller_names(
        profiles.iter().map(|profile| profile.target_name()),
    )
    .map_err(|diagnostics| format!("{diagnostics:?}"))?;
    let outcomes = build(BuildRequest {
        root_path: request.root.clone(),
        build_directory: Some(directory),
        targets: BuildTargets::Batch(targets),
        product: match request.product {
            Product::Check => ProjectProduct::Check,
            Product::Terminal => ProjectProduct::Terminal,
            Product::Native => ProjectProduct::Native {
                destination: NativeDestination::Retained,
                private_entry_ram: Vec::new(),
            },
        },
        timings: false,
        offline: true,
        verify: false,
        require_package_project: true,
        optimization_rollback: request.rollback.clone(),
        build_snapshot: None,
    })?;
    Ok(outcomes)
}

fn produce(
    request: &Request,
) -> Result<Vec<(target::TargetProfile, compiler::CompileReport)>, Box<dyn std::error::Error>> {
    let outcomes = build_targets(request, &request.targets, "shared")?;
    assert_eq!(
        outcomes
            .iter()
            .map(|outcome| outcome.target)
            .collect::<Vec<_>>(),
        request.targets
    );
    for outcome in &outcomes {
        println!(
            "target={} outcome={}",
            outcome.target.target_name(),
            if outcome.result.is_ok() {
                request.product.outcome()
            } else {
                "rejected"
            }
        );
    }
    let mut reports = Vec::new();
    let mut refusals = 0;
    for outcome in outcomes {
        if request.rejected_target == Some(outcome.target) {
            let error = match outcome.result {
                Ok(_) => return Err(format!("{} must refuse", outcome.target.target_name()).into()),
                Err(error) => error,
            };
            if !error.to_string().contains(&request.rejection_reason) {
                return Err(format!(
                    "{} reached an unrelated refusal: {error}",
                    outcome.target.target_name()
                )
                .into());
            }
            refusals += 1;
        } else {
            let report = outcome.result?.into_report()?.report;
            reports.push((outcome.target, report));
        }
    }
    assert_eq!(refusals, usize::from(request.rejected_target.is_some()));
    if request.product == Product::Check {
        println!("checked_products={}", reports.len());
        return Ok(reports);
    }
    if request.product == Product::Terminal {
        println!("terminal_products={}", reports.len());
        let mut retained = Vec::with_capacity(reports.len());
        for (profile, report) in reports {
            if !request.rollback.is_empty() {
                let receipt = report
                    .optimization_rollback_receipt()
                    .ok_or("Terminal product lost the requested rollback receipt")?;
                println!("target={} rollback={receipt}", profile.target_name());
            }
            assert!(
                report.retained_native_artifact().is_none(),
                "Terminal stop produced native code"
            );
            assert!(
                report.checked_native_executable_path().is_none(),
                "Terminal stop published native code"
            );
            retained.push((
                profile,
                report
                    .into_retained_terminal_artifact()
                    .ok_or("missing Terminal artifact")?,
            ));
        }
        let first = retained
            .first()
            .ok_or("missing Terminal product")?
            .1
            .artifact();
        for (profile, product) in &retained {
            let artifact = product.artifact();
            assert!(
                std::ptr::eq(first, artifact),
                "equal source targets must share Terminal storage"
            );
            let companion = product
                .native_realization_proposal()
                .ok_or("Terminal product lost its Build companion")?;
            let mut matching = companion
                .target_provider_proposals()
                .iter()
                .filter(|proposal| proposal.target == profile.target_name());
            let proposal = matching
                .next()
                .ok_or("Terminal product lost its target selection")?;
            assert!(
                matching.next().is_none(),
                "duplicate Terminal target selection"
            );
            let observed = terminal_runtime::execute(terminal_runtime::TerminalRuntimeRequest {
                entry: Some(companion.program_entry().checked_entry()),
                filesystem: Default::default(),
                native_provider: None,
                artifact,
                target: Some(profile.target_name()),
                provisions: None,
                provider_proposal: Some(proposal),
                x86_flags: None,
                memory_ordering: None,
                fuel_allowance: 100_000,
                domain_exit: terminal_runtime::TerminalDomainExitDelegation::Unprovided,
                standard_input: None,
                clock: None,
            })?;
            assert!(
                matches!(
                    observed.status,
                    terminal_runtime::TerminalExecutionStatus::Complete(
                        terminal_runtime::TerminalExecutionResult::Unit
                    )
                ),
                "Terminal execution did not return normally: {:?}",
                observed.status
            );
            assert!(observed.stdout.is_empty() && observed.stderr.is_empty());
            println!("target={} terminal_execution=unit", profile.target_name());
        }
        return Ok(Vec::new());
    }
    println!("native_products={}", reports.len());
    if reports.is_empty() {
        return Ok(reports);
    }
    let first = reports
        .first()
        .and_then(|(_, report)| report.retained_native_artifact())
        .ok_or("missing native artifact")?
        .psi_artifact();
    for (_, report) in &reports[1..] {
        let artifact = report
            .retained_native_artifact()
            .ok_or("missing native artifact")?
            .psi_artifact();
        assert!(
            std::ptr::eq(first, artifact),
            "equal source target consumers must share Terminal storage"
        );
    }
    Ok(reports)
}

fn observe_native_products(
    request: &Request,
    mut reports: Vec<(target::TargetProfile, compiler::CompileReport)>,
    before: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = request.native_evidence.then(|| {
        std::path::PathBuf::from(format!(
            "build/package-batch-observations-{}",
            std::process::id()
        ))
    });
    if let Some(directory) = &directory {
        std::fs::create_dir_all(directory)?;
        let invocations = native_production::native_pipeline_invocations()
            .into_iter()
            .skip(before)
            .collect::<Vec<_>>();
        for invocation in &invocations {
            assert!(request.targets.contains(&invocation.profile));
            assert_eq!(
                invocation.unit,
                native_production::NativePipelineUnit::Application
            );
            assert!(invocation.stages.iter().all(|count| *count <= 1));
        }
        for profile in &request.targets {
            let rows = invocations
                .iter()
                .filter(|row| row.profile == *profile)
                .collect::<Vec<_>>();
            if request.rejected_target == Some(*profile) {
                assert!(rows.len() <= 1 && rows.iter().all(|row| !row.completed));
            } else {
                assert_eq!(
                    rows.len(),
                    1,
                    "one native invocation for {}",
                    profile.target_name()
                );
                assert!(rows[0].completed);
                assert_eq!(rows[0].stages, [1; 10]);
                assert_eq!(rows[0].fragment_emission_attempts, 1);
                assert_eq!(rows[0].image_emission_attempts, 1);
            }
        }
        std::fs::write(
            directory.join("invocations.txt"),
            invocations
                .iter()
                .map(|row| format!("{row:?}\n"))
                .collect::<String>(),
        )?;
        for (profile, report) in &reports {
            let native = report
                .retained_native_artifact()
                .ok_or("native product missing")?;
            std::fs::write(
                directory.join(format!("{}-shared.image", profile.target_name())),
                &native.image().output().bytes,
            )?;
        }
        if let Some((_, report)) = reports.first() {
            std::fs::write(
                directory.join("program.oterm"),
                report
                    .retained_native_artifact()
                    .ok_or("native product missing")?
                    .psi_artifact()
                    .to_bytes(),
            )?;
        }
        println!("native_pipeline_evidence={}", directory.display());
    }
    // Whole-invocation counts and entered stage records were captured before
    // these separate invocations. Each obtains its own Build/source custody.
    if request.compare_single_targets {
        for (profile, shared) in &reports {
            let mut outcomes = build_targets(
                request,
                &[*profile],
                &format!("single-{}", profile.target_name()),
            )?;
            assert_eq!(outcomes.len(), 1);
            let single = outcomes.remove(0).result?.into_report()?.report;
            let shared = &shared
                .retained_native_artifact()
                .ok_or("shared image missing")?
                .image()
                .output()
                .bytes;
            let single = &single
                .retained_native_artifact()
                .ok_or("single image missing")?
                .image()
                .output()
                .bytes;
            assert!(
                shared == single,
                "{}: shared and independent images differ",
                profile.target_name()
            );
            if let Some(directory) = &directory {
                std::fs::write(
                    directory.join(format!("{}-single.image", profile.target_name())),
                    single,
                )?;
            }
            println!(
                "target={} single_target_bytes_agree={}",
                profile.target_name(),
                shared.len()
            );
        }
    }
    if request.run_host {
        let host =
            target::TargetProfile::host_if_supported().ok_or("unsupported execution host")?;
        let index = reports
            .iter()
            .position(|(profile, _)| *profile == host)
            .ok_or("the accepted target roster does not contain the host")?;
        let (_, report) = reports.swap_remove(index);
        let artifact = report
            .retained_native_artifact()
            .ok_or("host native product missing")?
            .psi_artifact();
        let reloaded = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())?;
        let observed = terminal_runtime::execute(terminal_runtime::TerminalRuntimeRequest {
            entry: None,
            filesystem: Default::default(),
            native_provider: None,
            x86_flags: None,
            memory_ordering: None,
            provider_proposal: None,
            artifact: &reloaded,
            target: Some(host.target_name()),
            provisions: report.runtime_provisions(),
            fuel_allowance: 100_000,
            domain_exit: terminal_runtime::TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: None,
            clock: None,
        })?;
        assert!(
            matches!(
                observed.status,
                terminal_runtime::TerminalExecutionStatus::Complete(
                    terminal_runtime::TerminalExecutionResult::Unit
                )
            ),
            "host Terminal product did not return Unit: {:?}",
            observed.status
        );
        let output =
            std::path::PathBuf::from(format!("build/package-batch-native-{}", std::process::id()));
        let published = compilation_products::publication::publish_compilation(report, &output)?;
        let observed = std::process::Command::new(
            published
                .checked_native_executable_path()
                .ok_or("host executable missing")?,
        )
        .output()?;
        assert_eq!(observed.status.code(), Some(0), "{observed:?}");
        println!("host_native_exit=0 reloaded_terminal=Unit");
    }
    Ok(())
}
