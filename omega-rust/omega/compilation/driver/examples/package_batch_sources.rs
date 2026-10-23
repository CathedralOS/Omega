//! Measure complete fixture preparation and a real mixed-source package batch.
//! This uses the established repository fixture acceptance, not a package audit.
//! Its setup compiles each source under that policy: whole totals include those
//! sessions as well as the submitted batch, rather than claiming zero-cost setup.
#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use artifacts::compile_timings::counters;
use compilation_products::NativeProductRequest;
use compiler::{
    CheckedCompileRequest, CompilationProduct, CompilationSession, PackageCompilationRequest,
};
use terminal_psi_to_abstract_operations::provider_admission::current_terminal_authority_policy;

fn main() {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--build-profile")) {
        println!("{}", compiler::compiler_build_profile());
        return;
    }
    let whole_builds = counters::build_execution_count();
    let whole_checks = counters::settled_check_totals().0;
    let whole_sources = counters::product_source_completion_count();
    let whole_terminals = counters::terminal_production_count();
    let root = fixture_package_inputs::repo_root();
    let first = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &root.join("tests/omega/pass/targets/shared_program_distinct_roots_exit/main.omg"),
        Some("linux_x86_64"),
    )
    .expect("first source custody");
    let second = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &root.join("tests/omega/pass/optimizer/rollback_to_no_selection_empty_entry/main.omg"),
        Some("linux_x86_64"),
    )
    .expect("second source custody");
    let mut requests = Vec::new();
    for (prepared, target) in [
        (&first, "windows_x86_64"),
        (&first, "linux_x86_64"),
        (&second, "linux_x86_64"),
    ] {
        let mut request = CheckedCompileRequest::new(&prepared.root_path, Some(target));
        request.package_inputs = prepared.package_inputs.clone();
        request.build_execution_profile = target::TargetProfile::host_if_supported();
        request.build_snapshot =
            package_compilation::BuildSnapshotRequest::automatic_package_inventory(
                request.package_inputs.as_ref(),
            );
        let occurrences = fixture_package_inputs::fixture_occurrences(prepared, request)
            .expect("dependency-first plan");
        requests.push(PackageCompilationRequest {
            occurrences,
            product: CompilationProduct::Native(NativeProductRequest {
                private_entry_ram: Vec::new(),
                terminal_authority_policy: current_terminal_authority_policy(),
                terminal_authority_permission_policy: None,
                optimization_rollback: Default::default(),
            }),
            terminal_permissions: None,
        });
    }
    let builds = counters::build_execution_count();
    let checks = counters::settled_check_totals().0;
    let sources = counters::product_source_completion_count();
    let terminals = counters::terminal_production_count();
    println!(
        "preparation_builds={} preparation_settled_checks={}",
        builds - whole_builds,
        checks - whole_checks
    );
    let outputs = CompilationSession::default().compile_packages(requests);
    let reports: Vec<_> = outputs
        .into_iter()
        .map(|result| {
            result
                .unwrap_or_else(|failure| {
                    panic!(
                        "package occurrence {}: {:?}",
                        failure.occurrence,
                        failure.failure.into_diagnostics()
                    )
                })
                .into_report()
                .unwrap_or_else(|error| panic!("{:?}", error.into_diagnostics()))
        })
        .collect();
    println!(
        "products={} batch_builds={} batch_settled_checks={} batch_source_completions={} batch_terminal_productions={}",
        reports.len(),
        counters::build_execution_count() - builds,
        counters::settled_check_totals().0 - checks,
        counters::product_source_completion_count() - sources,
        counters::terminal_production_count() - terminals
    );
    println!(
        "whole_measurement_builds={} whole_measurement_settled_checks={} whole_source_completions={} whole_terminal_productions={}",
        counters::build_execution_count() - whole_builds,
        counters::settled_check_totals().0 - whole_checks,
        counters::product_source_completion_count() - whole_sources,
        counters::terminal_production_count() - whole_terminals
    );
    assert_eq!(reports.len(), 3);
    assert_eq!(
        (
            counters::build_execution_count() - builds,
            counters::settled_check_totals().0 - checks,
            counters::product_source_completion_count() - sources,
            counters::terminal_production_count() - terminals
        ),
        (2, 4, 2, 2),
        "one Build and one Product check per distinct source cohort"
    );
    assert_eq!(
        (
            counters::build_execution_count() - whole_builds,
            counters::settled_check_totals().0 - whole_checks,
            counters::product_source_completion_count() - whole_sources,
            counters::terminal_production_count() - whole_terminals
        ),
        (4, 8, 4, 2),
        "fixture acceptance preparation is real work, not a zero-cost setup"
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
        std::ptr::eq(artifacts[0], artifacts[1]),
        "equal source consumers share Terminal storage"
    );
    assert!(
        !std::ptr::eq(artifacts[0], artifacts[2]),
        "different source continuations remain distinct"
    );
}
