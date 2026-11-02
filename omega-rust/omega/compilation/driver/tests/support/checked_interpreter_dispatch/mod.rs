//! Prepare the optional checked-tree interpreter's selected execution bindings.
//!
//! Product compilation does not execute this source oracle and retains no such
//! table. Terminal/native admission consumes its own portable demand catalog.
//! The corpus runner's `--checked-interpreter` leg is the only consumer and
//! requests these bindings immediately before execution. The oracle runs
//! checked source, so its bindings are prepared here rather than in Omega's
//! selected dispatch.

mod boundary_dispatch;
mod checked_body;

pub(crate) use checked_body::exact_checked_body_entry;

use boundary_dispatch::settle_selected_boundary_adapter_dispatch;
use diagnostics::Diagnostic;
use selected_dispatch::SelectedExecution;

/// Exact adapter and IEEE comparison bindings for one checked-source oracle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedInterpreterDispatch {
    pub boundary_adapters: Vec<checked_trees::CheckedBoundaryAdapterDispatch>,
    pub float_comparisons: Vec<checked_trees::CheckedSelectedFloatComparisonExecution>,
}

/// Bind the reference engine to the same checked program and selected plans.
/// Errors refuse reference execution; they never fall back to another engine.
pub fn prepare_checked_interpreter_dispatch(
    execution: &SelectedExecution,
) -> Result<CheckedInterpreterDispatch, Vec<Diagnostic>> {
    let checked = execution.provider_source.program();
    let selected_instances = execution.provider_source.project_instances()?;
    let selected_provider_plans = &selected_instances;
    let selected_provider_provenance = &execution.selected_provider_provenance;
    if selected_provider_provenance.iter().any(|provider| {
        selected_provider_plans
            .plan_by_exact_evidence(provider.plan.report_fingerprint(), &provider.plan)
            .is_none()
    }) {
        return Err(vec![Diagnostic::error(
            "comparison provider provenance disagrees with the selected provider plans",
        )]);
    }
    let boundary_adapters =
        settle_selected_boundary_adapter_dispatch(checked, selected_provider_plans)?;
    let float_comparisons = selected_dispatch::selected_float_comparison_executions(
        checked,
        source::DependencyScope::Product,
        selected_provider_provenance,
    )?;
    Ok(CheckedInterpreterDispatch {
        boundary_adapters,
        float_comparisons,
    })
}
