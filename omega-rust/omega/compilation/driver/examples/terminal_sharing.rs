//! Measure real two-target production, outside parallel corpus counter traffic.
//!
//! `cargo run -p compiler --example terminal_sharing` compiles the existing
//! distinct-entry customer for Windows and Linux. Both native products must
//! borrow one Terminal allocation; this does not execute a foreign-host image.
//! Optional arguments select another corpus root followed by its target names.

#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use artifacts::compile_timings::counters;
use compiler::{CompileRequest, RequestedCompileProduct};

fn main() {
    let mut arguments = std::env::args().skip(1);
    let root = fixture_package_inputs::repo_root().join(arguments.next().unwrap_or_else(|| {
        "tests/omega/pass/targets/shared_program_distinct_roots_exit/main.omg".to_owned()
    }));
    let mut target_names: Vec<_> = arguments.collect();
    if target_names.is_empty() {
        target_names = vec!["windows_x86_64".to_owned(), "linux_x86_64".to_owned()];
    }
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        &root,
        Some(&target_names[0]),
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
    .with_requested_product(RequestedCompileProduct::NativeArtifact);
    if let Some(inputs) = prepared.package_inputs {
        request = request.with_package_inputs(inputs);
    }
    let sources_before = counters::product_source_completion_count();
    let builds_before = counters::build_execution_count();
    let checks_before = counters::settled_check_totals().0;
    let terminals_before = counters::terminal_production_count();
    let reports: Vec<_> = compiler::compile(request)
        .expect("request admission")
        .into_outcomes()
        .into_iter()
        .map(|outcome| outcome.into_result().expect("native product"))
        .collect();
    assert_eq!(reports.len(), target_names.len());
    let first = reports[0]
        .retained_native_artifact()
        .expect("first native artifact")
        .psi_artifact();
    for report in &reports[1..] {
        let second = report
            .retained_native_artifact()
            .expect("second native artifact")
            .psi_artifact();
        assert!(
            std::ptr::eq(first, second),
            "native products must retain one Terminal allocation"
        );
        assert_eq!(first.manifest().semantic(), second.manifest().semantic());
    }
    let sources = counters::product_source_completion_count() - sources_before;
    let builds = counters::build_execution_count() - builds_before;
    let checks = counters::settled_check_totals().0 - checks_before;
    let terminals = counters::terminal_production_count() - terminals_before;
    assert_eq!((sources, builds, checks, terminals), (1, 1, 2, 1));
    println!(
        "native_products={} source_completions={sources} builds={builds} checks={checks} terminal_productions={terminals} shared_allocation=true",
        reports.len()
    );
}
