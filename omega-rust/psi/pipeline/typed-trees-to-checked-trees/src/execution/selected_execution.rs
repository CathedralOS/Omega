//! Rebuild checked execution plans after exact provider settlement.
//!
//! Scalar-callee plans are borrowed inputs to Unit planning, not provisional
//! mutations of published facts. Fallible rebuilds publish only on success.

use crate::execution::execution_plans::{ExecutionPlans, build_execution_plans};
use checked_trees::CheckedTrees;

/// Exact compiler-owned join from one authored operator use to the checked
/// machine selected to realize it. Selected execution supplies these rows only
/// after ProviderPlan settlement; ordinary checking supplies none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedOperatorApplication {
    pub expression: typed_trees::expression::ExpressionHandle,
    pub origin: checked_trees::CheckedValueOrigin,
    pub requirement_operator: symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub realization_machine: symbols::SymbolHandle,
    pub realization_state: symbols::SymbolHandle,
    pub operands: Vec<typed_trees::expression::ExpressionHandle>,
}

/// One compiler-intrinsic nearest IEEE FMA selected for an attached Unit
/// local initializer. This is intentionally disjoint from checked-body
/// operator adapters: no bodyless call or fabricated realization machine is
/// introduced into checked Psi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedIeeeFloatFmaUnitApplication {
    pub expression: typed_trees::expression::ExpressionHandle,
    pub origin: checked_trees::CheckedValueOrigin,
    pub requirement_operator: symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub format: semantic_vocabulary::IeeeFloatFormat,
    pub operands: Vec<typed_trees::expression::ExpressionHandle>,
}

/// Rebuild every checked Terminal plan whose exact shape depends on selected
/// operator execution. Unit-local scalar calls and direct structural-scalar
/// returns are one transaction so neither selected family can erase the
/// other's custody.
pub fn rebuild_checked_terminal_plans_with_selected_execution(
    program: &mut CheckedTrees,
    operator_applications: &[SelectedOperatorApplication],
    ieee_float_fma_applications: &[SelectedIeeeFloatFmaUnitApplication],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let call_frames = validation::CallFrameResolver::new(&program.typed);
    let ExecutionPlans {
        boundary_returns,
        unit_effects,
        structural_scalar_returns,
        cleanup_diagnostics,
    } = build_execution_plans(
        &program.typed,
        &program.facts,
        Some(&program.facts.flow.terminal_structural_scalar_returns),
        operator_applications,
        ieee_float_fma_applications,
        call_frames.as_ref(),
    );
    if !cleanup_diagnostics.is_empty() {
        return Err(cleanup_diagnostics);
    }
    program.facts.flow.terminal_boundary_scalar_returns = boundary_returns;
    program.facts.flow.terminal_unit_effects = unit_effects;
    program.facts.flow.terminal_structural_scalar_returns = structural_scalar_returns;
    Ok(())
}
