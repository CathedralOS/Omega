//! Assert whole-invocation work through the public package-management API.
//!
//! Run alone: counters are process-wide. The measurement starts before source
//! preparation and ends after every requested outcome joins, including refusal.
//! Cross-target artifacts are produced but foreign images are not executed.
//!
//! Usage: package_batch [--check] [--targets name,name] [root [sources
//! [terminals [builds [settled_checks]]]]]. Defaults describe the two-target
//! shared_program_distinct_roots_exit fixture: 1 source, 1 Terminal, 1 Build,
//! and 2 checks (Build + Product). Check-only defaults to zero Terminal work.

use artifacts::compile_timings::counters;
use package_manager::operations::{
    PreparedLocalProjectCheckRequest, PreparedLocalProjectNativeRequest,
    check_prepared_local_project_for_targets, compile_prepared_local_project_for_native_targets,
    prepare_local_project_for_targets,
};

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
    check_only: bool,
    expected: WorkCounts,
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
    let mut arguments = std::env::args().skip(1);
    let first = arguments.next();
    let check_only = first.as_deref() == Some("--check");
    let mut root_argument = if check_only { arguments.next() } else { first };
    let target_names = if root_argument.as_deref() == Some("--targets") {
        let names = arguments
            .next()
            .ok_or("--targets requires comma-separated target names")?;
        root_argument = arguments.next();
        names
    } else {
        "windows_x86_64,linux_x86_64".to_owned()
    };
    let root = root_argument
        .unwrap_or_else(|| {
            "tests/omega/pass/targets/shared_program_distinct_roots_exit/main.omg".to_owned()
        })
        .into();
    let targets = target_names
        .split(',')
        .map(|name| {
            target::TargetProfile::from_omega_target_name(Some(name))
                .map_err(|diagnostic| format!("{diagnostic:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut expected_count = |default| -> Result<u64, std::num::ParseIntError> {
        Ok(arguments
            .next()
            .map(|value| value.parse())
            .transpose()?
            .unwrap_or(default))
    };
    let expected = WorkCounts {
        sources: expected_count(1)?,
        terminals: expected_count(u64::from(!check_only))?,
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
        check_only,
        expected,
    })
}

fn measure() -> Result<(), Box<dyn std::error::Error>> {
    let before = WorkCounts::snapshot();
    let request = request()?;
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
    result
}

fn produce(request: &Request) -> Result<(), Box<dyn std::error::Error>> {
    let prepared = prepare_local_project_for_targets(&request.root, &request.targets, true)?
        .ok_or("the measurement requires a package build.omg")?;
    let directory =
        std::env::temp_dir().join(format!("omega-package-batch-{}", std::process::id()));
    if request.check_only {
        let outcomes = check_prepared_local_project_for_targets(
            PreparedLocalProjectCheckRequest::for_targets(
                prepared,
                directory,
                request.targets.clone(),
            ),
        );
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
                    "checked"
                } else {
                    "rejected"
                }
            );
        }
        let reports = outcomes
            .into_iter()
            .map(|outcome| outcome.result)
            .collect::<Result<Vec<_>, _>>()?;
        println!("checked_products={}", reports.len());
        return Ok(());
    }
    let outcomes = compile_prepared_local_project_for_native_targets(
        PreparedLocalProjectNativeRequest::for_targets(
            prepared,
            directory,
            request.targets.clone(),
        ),
    );
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
                "native"
            } else {
                "rejected"
            }
        );
    }
    let reports = outcomes
        .into_iter()
        .map(|outcome| outcome.result)
        .collect::<Result<Vec<_>, _>>()?;
    println!("native_products={}", reports.len());
    let first = reports
        .first()
        .and_then(|report| report.retained_native_artifact())
        .ok_or("missing native artifact")?
        .psi_artifact();
    for report in &reports[1..] {
        let artifact = report
            .retained_native_artifact()
            .ok_or("missing native artifact")?
            .psi_artifact();
        assert!(
            std::ptr::eq(first, artifact),
            "equal source target consumers must share Terminal storage"
        );
    }
    Ok(())
}
