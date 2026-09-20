//! Console adapter for the project compilation operation.

use super::{admissions::report_unsettled_admissions, arguments::CompileArguments};
use compiler::CompileOptions;
use omega::compilation::{
    CompileProjectError, CompileProjectRequest, ProjectProduct, compile_project,
};

pub(crate) fn compile_project_command(arguments: CompileArguments) {
    let started = arguments.timings.then(std::time::Instant::now);
    let request = CompileProjectRequest {
        options: CompileOptions {
            build_dir: arguments.build_dir,
            root_path: arguments.root_path,
            target_name: arguments.target_name,
        },
        product: if arguments.check_only {
            ProjectProduct::Check
        } else {
            ProjectProduct::NativeArtifact
        },
        timings: arguments.timings,
        offline: arguments.offline,
        accept_admissions: arguments.accept_admissions,
        require_package_project: false,
        optimization_rollback: arguments.optimization_rollback,
        build_snapshot: arguments
            .build_inputs
            .map(|capture| compiler::BuildSnapshotRequest::scoped(std::iter::empty(), capture)),
    };
    let result = compile_project(request);
    if let Some(started) = started {
        if let Ok(outcome) = &result {
            for timing in outcome.timings.phases() {
                eprintln!(
                    "{:>10.3} ms  {}",
                    timing.microseconds as f64 / 1_000.0,
                    timing.phase
                );
            }
        }
        eprintln!(
            "{:>10.3} ms  total elapsed",
            started.elapsed().as_secs_f64() * 1_000.0
        );
    }
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(CompileProjectError::UnsettledAdmissions(settlement)) => {
            report_unsettled_admissions(&settlement);
            std::process::exit(1);
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    if let Some(path) = outcome.executable_path {
        if let Some(receipt) = outcome.report.optimization_rollback_receipt() {
            println!("optimizer rollback: {receipt}");
        }
        // A published macOS package reports its `.app` root; the inner
        // executable stays reachable through `checked_native_executable_path`.
        if let Some(package_root) = outcome.report.checked_native_package_path() {
            println!("published macOS package to {}", package_root.display());
        } else {
            println!("published native output to {}", path.display());
        }
        for pair in outcome.report.pcc_publications() {
            println!("published {pair}");
        }
    } else {
        println!("{}", outcome.report.summary());
    }
    if let Some(directory) = outcome
        .report
        .build_outputs()
        .and_then(|outputs| outputs.published_directory())
    {
        println!(
            "published completed build outputs to {}",
            directory.display()
        );
    }
}
