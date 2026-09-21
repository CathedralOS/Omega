//! Console adapter for the project compilation operation.

use super::{admissions::report_unsettled_admissions, arguments::CompileArguments};
use compiler::CompileOptions;
use omega::compilation::{
    CompileProjectError, CompileProjectRequest, ProjectProduct, compile_project,
};

pub(crate) fn compile_project_command(arguments: CompileArguments) {
    let started = arguments.timings.then(std::time::Instant::now);
    let report_file = arguments.report_file.clone();
    let report_target_name = arguments.target_name.clone();
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
            for timing in outcome.report.timings() {
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
    if let Some(path) = &report_file {
        if let Err(error) = write_report_file(path, report_target_name.as_deref(), &outcome) {
            eprintln!(
                "cannot write compile report to `{}`: {error}",
                path.display()
            );
            std::process::exit(1);
        }
    }
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

/// Render the produced outcome as the plain-text observation file
/// `--report-file` requests: the requested product, the named target, the
/// report summary, and every publication line the console reports.
fn write_report_file(
    path: &std::path::Path,
    target_name: Option<&str>,
    outcome: &omega::compilation::CompileProjectOutcome,
) -> std::io::Result<()> {
    let mut contents = String::new();
    contents.push_str(match outcome.executable_path.is_some() {
        true => "product: native artifact\n",
        false => "product: check\n",
    });
    match target_name {
        Some(target) => contents.push_str(&format!("target: {target}\n")),
        None => contents.push_str("target: host default\n"),
    }
    contents.push_str(&outcome.report.summary());
    contents.push('\n');
    if let Some(receipt) = outcome.report.optimization_rollback_receipt() {
        contents.push_str(&format!("optimizer rollback: {receipt}\n"));
    }
    if let Some(path) = &outcome.executable_path {
        match outcome.report.checked_native_package_path() {
            Some(package_root) => {
                contents.push_str(&format!(
                    "published macOS package to {}\n",
                    package_root.display()
                ));
            }
            None => {
                contents.push_str(&format!("published native output to {}\n", path.display()));
            }
        }
    }
    for pair in outcome.report.pcc_publications() {
        contents.push_str(&format!("published {pair}\n"));
    }
    if let Some(directory) = outcome
        .report
        .build_outputs()
        .and_then(|outputs| outputs.published_directory())
    {
        contents.push_str(&format!(
            "published completed build outputs to {}\n",
            directory.display()
        ));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use super::write_report_file;
    use artifacts::compile_timings::CompileTimings;
    use compiler::CompileReport;
    use omega::compilation::CompileProjectOutcome;
    use std::path::PathBuf;

    #[test]
    fn report_file_observes_a_check_outcome() {
        let directory = std::env::temp_dir().join(format!(
            "omega-report-file-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        let path = directory.join("observations/report.txt");
        let outcome = CompileProjectOutcome {
            report: CompileReport::check_only(PathBuf::from("project/main.omg"), 3)
                .expect("check report"),
            timings: CompileTimings::default(),
            executable_path: None,
        };

        write_report_file(&path, Some("linux_x86_64"), &outcome).expect("report write");

        let contents = std::fs::read_to_string(&path).expect("report read");
        assert!(contents.starts_with("product: check\ntarget: linux_x86_64\n"));
        assert!(contents.contains("compiled 3 source file(s)"));
        assert!(contents.contains("wrote_output=false"));
        std::fs::remove_dir_all(&directory).ok();
    }
}
