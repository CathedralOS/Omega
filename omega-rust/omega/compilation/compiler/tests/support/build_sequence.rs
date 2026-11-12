//! Direct invocation tests execute the original root Build through the
//! application executor, retaining its actual execution charges.
//!
//! Each test or example that includes this file uses a different subset.
#![allow(dead_code)]

pub fn return_package(
    invocation: &mut compiler::PackageBuildInvocation,
) -> Option<build_settings::ReturnedBuild> {
    let mut queries = invocation.product_queries();
    orchestration::execute_build_activation(invocation.execution(), Some(&mut queries))
        .unwrap_or_else(|failure| panic!("Build execution: {:?}", failure.into_diagnostics()))
}

/// Show actual attempts before checking the existing no-replay expectations.
/// These process totals do not describe a coherent concurrent snapshot.
pub fn print_execution_counts(context: &str) {
    use artifacts::compile_timings::counters;
    println!(
        "Build starts ({context}): total={} source={} supplier={}",
        counters::build_execution_count(),
        counters::source_build_execution_count(),
        counters::provider_defaults_execution_count(),
    );
}
