//! Prepare the optional checked-tree interpreter's selected execution bindings.
//!
//! Product compilation does not execute this source oracle and retains no such
//! table. Terminal/native admission consumes its own portable demand catalog.
//! The reference consumer requests these bindings immediately before execution.

mod boundary_dispatch;

use crate::SelectedExecution;
use diagnostics::Diagnostic;

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
    let boundary_adapters = boundary_dispatch::settle_selected_boundary_adapter_dispatch(
        checked,
        selected_provider_plans,
    )?;
    let float_comparisons = crate::selected_dispatch::float_comparisons::selected_executions(
        checked,
        source::DependencyScope::Product,
        selected_provider_provenance,
    )?;
    Ok(CheckedInterpreterDispatch {
        boundary_adapters,
        float_comparisons,
    })
}
