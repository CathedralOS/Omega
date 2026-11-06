//! Direct invocation tests use the application executor for supplier work.
//!
//! They may then inspect or interleave the original root activation. Keeping
//! this preparation explicit also makes its real execution charges visible.
//!
//! Each test or example that includes this file uses a different subset.
#![allow(dead_code)]

pub fn prepare_package_root(invocation: &mut compiler::PackageBuildInvocation) {
    let mut queries = invocation.product_queries();
    let returned =
        orchestration::execute_build_activation(invocation.execution(), Some(&mut queries));
    assert!(matches!(
        invocation
            .advance(returned)
            .unwrap_or_else(|failure| panic!("core companion: {:?}", failure.into_diagnostics())),
        compiler::BuildAdvance::Continue
    ));
}

pub fn prepare_source_root(invocation: &mut compiler::SourceCompilation) {
    let mut queries = invocation.product_queries();
    let returned =
        orchestration::execute_build_activation(invocation.execution(), Some(&mut queries));
    assert!(matches!(
        invocation
            .advance(returned)
            .unwrap_or_else(|failure| panic!("core companion: {:?}", failure.into_diagnostics())),
        compiler::BuildAdvance::Continue
    ));
}

pub fn return_package(
    invocation: &mut compiler::PackageBuildInvocation,
) -> Option<build_settings::ReturnedBuild> {
    loop {
        let mut queries = invocation.product_queries();
        let returned =
            orchestration::execute_build_activation(invocation.execution(), Some(&mut queries));
        match invocation
            .advance(returned)
            .unwrap_or_else(|failure| panic!("Build progression: {:?}", failure.into_diagnostics()))
        {
            compiler::BuildAdvance::Continue => continue,
            compiler::BuildAdvance::Complete(returned) => {
                return returned.unwrap_or_else(|failure| {
                    panic!("Build execution: {:?}", failure.into_diagnostics())
                });
            }
        }
    }
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
