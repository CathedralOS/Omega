//! Rebuild checked execution plans after exact provider settlement.
//!
//! Scalar-callee plans are borrowed inputs to Unit planning, not provisional
//! mutations of published facts. Fallible rebuilds publish only on success.

use crate::flow;
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
    let boundary_returns =
        flow::build_checked_boundary_scalar_return_plans(&program.typed, &program.facts);
    let primitive_returns =
        flow::build_checked_primitive_store_scalar_return_plans(&program.typed, &program.facts);
    let structural_returns = flow::reconcile_primitive_store_scalar_returns(
        &program.facts.flow.terminal_structural_scalar_returns,
        primitive_returns,
    );
    let scalar_callees = flow::ScalarCalleePlans {
        boundary_returns: &boundary_returns,
        structural_returns: &structural_returns,
    };
    let terminal_unit_effects = flow::build_checked_unit_effect_plans(
        &program.typed,
        &program.facts,
        scalar_callees,
        operator_applications,
        ieee_float_fma_applications,
    );
    let mut diagnostics = Vec::new();
    let structural_scalar_returns = flow::build_checked_structural_scalar_return_plans(
        &program.typed,
        &program.facts,
        &terminal_unit_effects,
        operator_applications,
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    program.facts.flow.terminal_boundary_scalar_returns = boundary_returns;
    program.facts.flow.terminal_unit_effects = terminal_unit_effects;
    program.facts.flow.terminal_structural_scalar_returns = structural_scalar_returns;
    Ok(())
}
