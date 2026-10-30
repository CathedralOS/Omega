//! Exact checked boundary-operator realization joins.
//!
//! Application coverage and specialization rejoin the authored requirement,
//! selected plan and checked body here. Selection never rebuilds checked Psi
//! or guesses a missing execution route from the selected provider.

mod application_realization;
mod specialized_application_realization;

pub use application_realization::{
    CheckedOperatorAuthoredUseKind, derive_checked_nongeneric_operator_application_realizations,
};
pub use specialized_application_realization::derive_checked_specialized_operator_application_realizations;

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use typed_trees::expression::ExpressionHandle;

pub(super) fn resolve_checked_adapter_for_operator(
    checked: &CheckedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
    plan: &effects::provider_plan::ProviderPlan,
    expression: ExpressionHandle,
) -> Result<Option<(symbols::SymbolHandle, String, symbols::SymbolHandle)>, Diagnostic> {
    let overload_identity =
        typed_trees::operator::boundary_operator_requirement_identity(checked, operator);
    if overload_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked operator at expression {expression:?} has an empty canonical overload identity",
        )));
    }

    let [method] = plan.schema.methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-operator ProviderPlan `{}` must retain exactly one schema method",
            plan.name,
        )));
    };
    let [row] = plan.rows.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-operator ProviderPlan `{}` must retain exactly one realization row",
            plan.name,
        )));
    };
    let operator_package = checked
        .typed
        .symbols
        .symbol_package_identity(operator.symbol);
    if plan.schema.identity_text(plan.schema.trait_name) != overload_identity
        || plan.schema.trait_package_identity != operator_package
        || method.name != "realize"
        || method.requirement_owner != overload_identity
        || method.requirement_owner_package_identity != operator_package
        || plan.schema.identity_text(method.requirement_identity) != overload_identity
        || !plan.schema.row_binds_method(row, method, &plan.identities)
    {
        return Err(Diagnostic::error(format!(
            "selected checked-operator ProviderPlan `{}` does not bind exact overload `{overload_identity}`",
            plan.name,
        )));
    }

    let ProviderBinding::CheckedAdapter { .. } = &row.binding else {
        return Ok(None);
    };

    if plan.provider_type.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-operator ProviderPlan `{}` has no nominal provider type",
            plan.name,
        )));
    }
    let applications = checked
        .facts
        .operators
        .boundary_applications
        .iter()
        .filter(|application| {
            application.requirement_symbol == operator.symbol
                && matches!(
                    application.site,
                    checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
                        expression: retained_expression,
                        ..
                    } if retained_expression == expression
                )
        })
        .collect::<Vec<_>>();
    let application = match applications.as_slice() {
        [application] => Some(*application),
        [] => None,
        _ => {
            return Err(Diagnostic::error(format!(
                "selected checked-operator use at expression {expression:?} retains {} exact application demands; expected at most one",
                applications.len(),
            )));
        }
    };
    let application = application.ok_or_else(|| {
        Diagnostic::error(format!(
            "selected checked operator at expression {expression:?} has no retained application",
        ))
    })?;
    let body =
        provider_planning::selected_checked_operator_body(&checked.facts, application, plan)?;
    let provider = checked
        .typed
        .machine(body.machine)
        .ok_or_else(|| Diagnostic::error("selected checked operator lost its retained machine"))?;

    Ok(Some((
        provider.symbol,
        provider.name.as_str().to_owned(),
        body.entry,
    )))
}

pub(super) fn exact_operator_definition(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
    symbol: symbols::SymbolHandle,
) -> Result<&typed_trees::operator::OperatorDefinition, Diagnostic> {
    let operators = checked
        .typed
        .operators()
        .iter()
        .filter(|operator| operator.symbol == symbol)
        .collect::<Vec<_>>();
    let [operator] = operators.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked operator at expression {expression:?} resolves symbol {symbol:?} to {} operator definitions",
            operators.len(),
        )));
    };
    Ok(*operator)
}

/// This target's selected plan for one checked use, joined by requirement
/// identity. Checked uses carry no selection of their own.
pub(super) fn selected_use_plan<'plans>(
    checked: &CheckedTrees,
    selected_provider_plans: &'plans [effects::provider_plan::ProviderPlan],
    requirement: symbols::SymbolHandle,
    origin: checked_trees::CheckedValueOrigin,
) -> Option<&'plans effects::provider_plan::ProviderPlan> {
    provider_planning::selected_use_plan(
        checked,
        source::DependencyScope::Product,
        selected_provider_plans,
        requirement,
        origin,
    )
    .map(|(_, plan)| plan)
}

pub(super) fn resolve_exact_selected_plan<'plans>(
    checked: &CheckedTrees,
    selected_provider_plans: &'plans [effects::provider_plan::ProviderPlan],
    requirement: symbols::SymbolHandle,
    origin: checked_trees::CheckedValueOrigin,
    use_label: &str,
) -> Result<&'plans effects::provider_plan::ProviderPlan, Diagnostic> {
    selected_use_plan(checked, selected_provider_plans, requirement, origin).ok_or_else(|| {
        Diagnostic::error(format!(
            "{use_label} for requirement {requirement:?} has no exact selected ProviderPlan",
        ))
    })
}

/// One selected realization machine's single retained contract plan,
/// committed rather than empty.
pub(super) fn exact_realization_contract<'facts>(
    checked: &'facts CheckedTrees,
    realization_machine: symbols::SymbolHandle,
    realization_label: &str,
) -> Result<&'facts checked_trees::MachineContractPlan, Diagnostic> {
    let contracts = checked
        .facts
        .contract_plans
        .machines
        .iter()
        .filter(|contract| contract.machine == realization_machine)
        .collect::<Vec<_>>();
    let [contract] = contracts.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected {realization_label} realization retains {} exact machine contracts; expected one",
            contracts.len(),
        )));
    };
    if contract.commitment.is_zero() {
        return Err(Diagnostic::error(format!(
            "selected {realization_label} realization has an empty machine contract commitment",
        )));
    }
    Ok(contract)
}

/// One boundary operator's nonempty requirement overload identity.
pub(super) fn requirement_overload_identity(
    checked: &CheckedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
    realization_label: &str,
) -> Result<String, Diagnostic> {
    let identity = typed_trees::operator::boundary_operator_requirement_identity(checked, operator);
    if identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected {realization_label} realization has an empty requirement identity",
        )));
    }
    Ok(identity)
}
