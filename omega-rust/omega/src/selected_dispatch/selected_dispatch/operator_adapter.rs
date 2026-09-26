//! Checked boundary-operator ProviderPlan applications.
//!
//! Semantic checking and retained facts name the public boundary operator,
//! and execution keeps naming it: a named or fixed-token use whose exact plan
//! row is a checked adapter is found here only so its missing requirement-level
//! route reports `unimplemented:`. Application realizations for review and
//! Terminal coverage derive from the same checked facts.

mod application_realization;
mod specialized_application_realization;
mod spelled;

pub use application_realization::{
    CheckedNongenericOperatorApplicationRealization, CheckedOperatorAuthoredUseKind,
    derive_checked_nongeneric_operator_application_realizations,
};
pub use specialized_application_realization::{
    CheckedSpecializedOperatorApplicationRealization,
    derive_checked_specialized_operator_application_realizations,
};

use abstract_operations_to_target_operations::effects::provider_plan::ProviderBinding;
use diagnostics::Diagnostic;
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::{
    ExpressionHandle, ExpressionNode,
};
use typed_trees_to_checked_trees::checked_trees::{CheckedTrees, CheckedValueOrigin};

/// One authored boundary-operator use whose selected row is a checked
/// adapter, keyed by the requirement it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OperatorAdapterRewrite {
    pub(super) expression: ExpressionHandle,
    pub(super) origin: CheckedValueOrigin,
    pub(super) requirement_operator: symbols::SymbolHandle,
}

/// A selected operator's crash invocation has no Terminal replay support, so
/// a program that retains one cannot enter Terminal production.
pub fn validate_selected_operator_terminal_custody(
    checked: &CheckedTrees,
) -> Result<(), Vec<Diagnostic>> {
    if checked
        .facts
        .operators
        .has_crash_qualified_uses(&checked.typed)
    {
        return Err(vec![Diagnostic::error(
            "selected operator crash invocations have no Terminal replay support",
        )]);
    }
    Ok(())
}

pub(super) fn plan_selected_operator_adapter_rewrites(
    checked: &CheckedTrees,
    selected_provider_plans: &abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts,
) -> Result<Vec<OperatorAdapterRewrite>, Vec<Diagnostic>> {
    let mut rewrites = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, operator_use) in checked.facts.operators.named_uses.iter() {
        if selected_use_plan(
            checked,
            selected_provider_plans.plans(),
            operator_use.selected_operator_symbol,
            operator_use.origin,
        )
        .is_none()
        {
            continue;
        }
        let rewrite = match resolve_selected_operator_adapter_call(
            checked,
            selected_provider_plans.plans(),
            operator_use,
        ) {
            Ok(Some(rewrite)) => rewrite,
            Ok(None) => continue,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        stage_operator_adapter_rewrite(&mut rewrites, &mut diagnostics, rewrite);
    }

    for (_, operator_use) in checked.facts.operators.uses.iter() {
        // A Match equality is an arm decision, not an expression replacement.
        // Retain its selected demand; execution owners must supply arm-local
        // saved-subject custody before they can execute it.
        if operator_use.occurrence
            != typed_trees_to_checked_trees::checked_trees::CheckedOperatorOccurrence::Expression
        {
            continue;
        }
        if selected_use_plan(
            checked,
            selected_provider_plans.plans(),
            operator_use.selected_operator_symbol,
            operator_use.origin,
        )
        .is_none()
        {
            continue;
        }
        let rewrite = match spelled::resolve_selected_spelled_operator_adapter_call(
            checked,
            selected_provider_plans.plans(),
            operator_use,
        ) {
            Ok(Some(rewrite)) => rewrite,
            Ok(None) => continue,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        stage_operator_adapter_rewrite(&mut rewrites, &mut diagnostics, rewrite);
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(rewrites)
}

fn stage_operator_adapter_rewrite(
    rewrites: &mut Vec<OperatorAdapterRewrite>,
    diagnostics: &mut Vec<Diagnostic>,
    rewrite: OperatorAdapterRewrite,
) {
    if let Some(selected) = rewrites
        .iter()
        .find(|selected| selected.expression == rewrite.expression)
    {
        if selected != &rewrite {
            diagnostics.push(Diagnostic::error(format!(
                "operator expression {:?} carries contradictory checked-adapter realizations",
                rewrite.expression,
            )));
        }
        return;
    }
    rewrites.push(rewrite);
}

fn resolve_selected_operator_adapter_call(
    checked: &CheckedTrees,
    selected_provider_plans: &[abstract_operations_to_target_operations::effects::provider_plan::ProviderPlan],
    operator_use: &typed_trees_to_checked_trees::checked_trees::CheckedNamedOperatorUseFact,
) -> Result<Option<OperatorAdapterRewrite>, Diagnostic> {
    let plan = resolve_exact_selected_plan(
        checked,
        selected_provider_plans,
        operator_use.selected_operator_symbol,
        operator_use.origin,
        "named operator use",
    )?;

    resolve_operator_adapter_call(checked, operator_use, plan)
}

fn resolve_operator_adapter_call(
    checked: &CheckedTrees,
    operator_use: &typed_trees_to_checked_trees::checked_trees::CheckedNamedOperatorUseFact,
    plan: &abstract_operations_to_target_operations::effects::provider_plan::ProviderPlan,
) -> Result<Option<OperatorAdapterRewrite>, Diagnostic> {
    let operator = exact_operator_definition(
        checked,
        operator_use.expression,
        operator_use.selected_operator_symbol,
    )?;
    if !operator.is_boundary {
        return Err(Diagnostic::error(format!(
            "selected checked operator at expression {:?} does not name a boundary operator",
            operator_use.expression,
        )));
    }
    let ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression(operator_use.expression)
    else {
        return Err(Diagnostic::error(format!(
            "selected checked operator at expression {:?} is not a named call",
            operator_use.expression,
        )));
    };
    let source_names_selected_operator =
        symbol_resolved_trees_to_typed_trees::typed_trees::operator::resolve_named_expression_call(
            &checked.typed,
            call,
        )
        .map(|resolved| resolved.symbol)
            == Some(operator.symbol);
    let source_maps_to_selected_builtin =
        typed_trees_to_checked_trees::resolve_checked_builtin_float_operator_requirement(
            &checked.typed,
            operator_use.expression,
            operator_use.origin,
        ) == Some(operator.symbol);
    if !source_names_selected_operator && !source_maps_to_selected_builtin {
        return Err(Diagnostic::error(format!(
            "selected checked operator at expression {:?} no longer names its checked operator symbol",
            operator_use.expression,
        )));
    }

    if resolve_checked_adapter_for_operator(checked, operator, plan, operator_use.expression)?
        .is_none()
    {
        return Ok(None);
    }

    Ok(Some(OperatorAdapterRewrite {
        expression: operator_use.expression,
        origin: operator_use.origin,
        requirement_operator: operator_use.selected_operator_symbol,
    }))
}

pub(super) fn resolve_checked_adapter_for_operator(
    checked: &CheckedTrees,
    operator: &symbol_resolved_trees_to_typed_trees::typed_trees::operator::OperatorDefinition,
    plan: &abstract_operations_to_target_operations::effects::provider_plan::ProviderPlan,
    expression: ExpressionHandle,
) -> Result<Option<(symbols::SymbolHandle, String, symbols::SymbolHandle)>, Diagnostic> {
    let overload_identity =
        symbol_resolved_trees_to_typed_trees::typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator);
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
    if plan.schema.trait_name != overload_identity
        || plan.schema.trait_package_identity != operator_package
        || method.name != "realize"
        || method.requirement_owner != overload_identity
        || method.requirement_owner_package_identity != operator_package
        || method.requirement_identity != overload_identity
        || !plan.schema.row_binds_method(row, method)
    {
        return Err(Diagnostic::error(format!(
            "selected checked-operator ProviderPlan `{}` does not bind exact overload `{overload_identity}`",
            plan.name,
        )));
    }

    let ProviderBinding::CheckedAdapter {
        machine_identity,
        machine_package_identity,
    } = &row.binding
    else {
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
                    typed_trees_to_checked_trees::checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
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
    let direct_provider =
        crate::provider_planning::exact_checked_adapter(&checked.typed, plan, row);
    let specialized = checked
        .typed
        .machine_specializations
        .iter()
        .filter(|specialization| {
            typed_trees_to_checked_trees::validation::machine_specialization_matches_template_identity(
                &checked.typed,
                specialization,
                machine_identity,
                *machine_package_identity,
            ) && specialization
                .operator_realizations
                .iter()
                .any(|realization| {
                    realization.requirement_symbol == operator.symbol
                        && application.is_some_and(|application| {
                            specialized_application_realization::applications_match(
                                checked,
                                application,
                                realization,
                            )
                        })
                })
        })
        .filter_map(|specialization| {
            checked
                .typed
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.instance)
        })
        .filter(|machine| {
            checked
                .typed
                .symbols
                .symbol_package_identity(machine.symbol)
                == *machine_package_identity
        })
        .collect::<Vec<_>>();
    let requires_specialization =
        application.is_some_and(|application| !application.arguments.is_empty());
    let provider = if requires_specialization {
        let [provider] = specialized.as_slice() else {
            return Err(Diagnostic::error(format!(
                "selected checked-operator adapter `{machine_identity}` resolves to {} exact specializations for one application",
                specialized.len(),
            )));
        };
        *provider
    } else {
        match direct_provider {
            Ok(provider) => provider,
            Err(direct_error) => {
                let [provider] = specialized.as_slice() else {
                    return Err(direct_error);
                };
                *provider
            }
        }
    };
    if checked.typed.attached_data_path(provider).as_deref() != Some(plan.provider_type.as_str()) {
        return Err(Diagnostic::error(format!(
            "selected checked-operator adapter `{machine_identity}` does not belong to nominal provider `{}`",
            plan.provider_type,
        )));
    }
    if provider.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
        return Err(Diagnostic::error(format!(
            "selected checked-operator adapter `{machine_identity}` is not a checked body",
        )));
    }
    let Some(entry) = checked.typed.machine_states(provider).first() else {
        return Err(Diagnostic::error(format!(
            "selected checked-operator adapter `{machine_identity}` has no executable entry state",
        )));
    };

    let [namespace, requirement] = checked.typed.operator_path_members(operator.name) else {
        return Err(Diagnostic::error(format!(
            "selected checked operator overload `{overload_identity}` has no exact namespace and requirement path",
        )));
    };
    let conformances = checked
        .typed
        .machine_trait_conformances(provider)
        .iter()
        .filter(|conformance| {
            conformance.external_binding.is_none()
                && conformance.name.as_str() == namespace.as_str()
                && conformance.requirement.as_ref().map(|name| name.as_str())
                    == Some(requirement.as_str())
                && (symbol_resolved_trees_to_typed_trees::typed_trees::operator::resolve_satisfied_checked_operator_for_conformance(
                    &checked.typed,
                    provider,
                    conformance,
                )
                .is_some_and(|resolved| resolved.symbol == operator.symbol)
                    || symbol_resolved_trees_to_typed_trees::typed_trees::operator::resolve_specialized_checked_operator_application(
                        &checked.typed,
                        provider,
                        namespace.as_str(),
                        requirement.as_str(),
                    )
                    .is_some_and(|(resolved, _)| resolved.symbol == operator.symbol))
        })
        .count();
    if conformances != 1 {
        return Err(Diagnostic::error(format!(
            "selected checked-operator adapter `{machine_identity}` binds exact overload `{overload_identity}` through {conformances} checked conformances",
        )));
    }

    Ok(Some((
        provider.symbol,
        provider.name.as_str().to_owned(),
        entry.symbol,
    )))
}

pub(super) fn exact_operator_definition(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
    symbol: symbols::SymbolHandle,
) -> Result<
    &symbol_resolved_trees_to_typed_trees::typed_trees::operator::OperatorDefinition,
    Diagnostic,
> {
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
    selected_provider_plans: &'plans [abstract_operations_to_target_operations::effects::provider_plan::ProviderPlan],
    requirement: symbols::SymbolHandle,
    origin: typed_trees_to_checked_trees::checked_trees::CheckedValueOrigin,
) -> Option<&'plans abstract_operations_to_target_operations::effects::provider_plan::ProviderPlan>
{
    crate::provider_planning::selected_use_plan(
        checked,
        selected_provider_plans,
        requirement,
        origin,
    )
    .map(|(_, plan)| plan)
}

pub(super) fn resolve_exact_selected_plan<'plans>(
    checked: &CheckedTrees,
    selected_provider_plans: &'plans [abstract_operations_to_target_operations::effects::provider_plan::ProviderPlan],
    requirement: symbols::SymbolHandle,
    origin: typed_trees_to_checked_trees::checked_trees::CheckedValueOrigin,
    use_label: &str,
) -> Result<
    &'plans abstract_operations_to_target_operations::effects::provider_plan::ProviderPlan,
    Diagnostic,
> {
    selected_use_plan(checked, selected_provider_plans, requirement, origin).ok_or_else(|| {
        Diagnostic::error(format!(
            "{use_label} for requirement {requirement:?} has no exact selected ProviderPlan",
        ))
    })
}
