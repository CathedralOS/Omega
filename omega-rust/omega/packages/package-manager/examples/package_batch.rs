//! Measure two-target production through the real package-management API.
//!
//! Run from the repository root with `cargo run -p package-manager --example
//! package_batch`. An optional argument selects another package entry. This
//! measures producer work and shared storage, not foreign-host execution.
//! `--check <root>` measures source-only packages, including generated dependencies.

use artifacts::compile_timings::counters;
use package_manager::operations::{
    PreparedLocalProjectCheckRequest, PreparedLocalProjectNativeRequest,
    check_prepared_local_project_for_targets, compile_prepared_local_project_for_native_targets,
    prepare_local_project_for_targets,
};

fn main() -> Result<(), String> {
    // Match the shipped CLI's command worker: package resolution precedes
    // the compiler's own worker and still contains recursive tree walks.
    std::thread::Builder::new()
        .name("package-batch-measurement".to_owned())
        .stack_size(256 * 1024 * 1024)
        .spawn(|| measure().map_err(|error| error.to_string()))
        .map_err(|error| error.to_string())?
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

fn measure() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let first = arguments.next();
    let check_only = first.as_deref() == Some("--check");
    let root = std::path::PathBuf::from(
        (if check_only { arguments.next() } else { first }).unwrap_or_else(|| {
            "tests/omega/pass/targets/shared_program_distinct_roots_exit/main.omg".to_owned()
        }),
    );
    let targets = ["windows_x86_64", "linux_x86_64"]
        .into_iter()
        .map(|name| {
            target::TargetProfile::from_omega_target_name(Some(name))
                .map_err(|diagnostic| format!("{diagnostic:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let prepared = prepare_local_project_for_targets(&root, &targets, true)?
        .ok_or("the measurement requires a package build.omg")?;
    let sources = counters::product_source_completion_count();
    let builds = counters::build_execution_count();
    let terminals = counters::terminal_production_count();
    let directory =
        std::env::temp_dir().join(format!("omega-package-batch-{}", std::process::id()));
    if check_only {
        let reports = check_prepared_local_project_for_targets(
            PreparedLocalProjectCheckRequest::for_targets(prepared, directory, targets),
        )
        .into_iter()
        .map(|outcome| outcome.result)
        .collect::<Result<Vec<_>, _>>()?;
        let sources = counters::product_source_completion_count() - sources;
        let builds = counters::build_execution_count() - builds;
        let terminals = counters::terminal_production_count() - terminals;
        println!(
            "checked_products={} source_completions={sources} builds={builds} terminal_productions={terminals}",
            reports.len()
        );
        assert_eq!((reports.len(), sources, terminals), (2, 1, 0));
        return Ok(());
    }
    let reports = compile_prepared_local_project_for_native_targets(
        PreparedLocalProjectNativeRequest::for_targets(prepared, directory, targets),
    )
    .into_iter()
    .map(|outcome| outcome.result)
    .collect::<Result<Vec<_>, _>>()?;
    let sources = counters::product_source_completion_count() - sources;
    let builds = counters::build_execution_count() - builds;
    let terminals = counters::terminal_production_count() - terminals;
    println!(
        "native_products={} source_completions={sources} builds={builds} terminal_productions={terminals}",
        reports.len()
    );
    assert_eq!(reports.len(), 2);
    assert_eq!(
        sources, 1,
        "one Product source completion, not one per target"
    );
    assert_eq!(terminals, 1);
    let first = reports[0]
        .retained_native_artifact()
        .ok_or("missing native artifact")?
        .psi_artifact();
    let second = reports[1]
        .retained_native_artifact()
        .ok_or("missing native artifact")?
        .psi_artifact();
    assert!(
        std::ptr::eq(first, second),
        "native targets must share Terminal storage"
    );
    Ok(())
}
