//! Exact checked boundary-operator realization joins.
//!
//! Application coverage and specialization rejoin the authored requirement,
//! selected plan and checked body here. Selection never rebuilds checked Psi
//! or guesses a missing execution route from the selected provider.

mod application_realization;
mod specialized_application_realization;
#[cfg(test)]
mod tests;

pub use application_realization::{
    CheckedNongenericOperatorApplicationRealization, CheckedOperatorAuthoredUseKind,
    derive_checked_nongeneric_operator_application_realizations,
};
pub use specialized_application_realization::{
    CheckedSpecializedOperatorApplicationRealization,
    derive_checked_specialized_operator_application_realizations,
};

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use typed_trees::expression::ExpressionHandle;

/// A selected operator's crash invocation enters Terminal production only
/// through a covered carrier: a spelled use replays through the
/// `TerminalOperationCrashContract` row stage 05 installs beside the emitted
/// comparison, and a named call's contract stays on its `BoundaryCall`
/// operation's boundary declaration. Admission therefore requires every
/// crash-qualified use to name a checked crash site on its owning machine
/// whose selected operator's signature is all scalar, and — for a spelled
/// use — a comparison spelling, the only spelled emission that records the
/// `operator_use` join a site lowers through. A use outside that shape still
/// has no carrier and cannot enter production; a carried site whose exact
/// join cannot be reconstructed keeps failing closed at retention.
pub fn validate_selected_operator_terminal_custody(
    checked: &CheckedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let program = &checked.typed;
    let diagnostics = checked
        .facts
        .operators
        .crash_qualified_uses(program)
        .filter_map(|crash_use| carried_crash_site_shape(checked, crash_use).err())
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// The covered shapes a checked crash site lowers through. Everything else —
/// a site stage 04 could not admit, a signature whose parameters have no
/// positional scalar telescope, or a spelled use no comparison occurrence can
/// join — keeps the refusal this gate has always owned.
fn carried_crash_site_shape(
    checked: &CheckedTrees,
    crash_use: checked_trees::CheckedCrashQualifiedUse,
) -> Result<(), Diagnostic> {
    let program = &checked.typed;
    let operators = &checked.facts.operators;
    let (origin, spelling) = match crash_use {
        checked_trees::CheckedCrashQualifiedUse::Spelled(handle) => {
            let operator_use = operators.uses.get(handle);
            (operator_use.origin, Some(operator_use.spelling))
        }
        checked_trees::CheckedCrashQualifiedUse::Named(handle) => {
            (operators.named_uses.get(handle).origin, None)
        }
    };
    let checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, .. } = origin else {
        return Err(Diagnostic::error(
            "selected operator crash invocation has no machine-local checked origin",
        ));
    };
    let site = checked
        .facts
        .contract_plans
        .for_machine(machine_symbol)
        .and_then(|plan| {
            plan.crash
                .checked_operators()
                .iter()
                .find(|site| match crash_use {
                    checked_trees::CheckedCrashQualifiedUse::Spelled(handle) => {
                        site.operator_use == handle
                    }
                    checked_trees::CheckedCrashQualifiedUse::Named(handle) => {
                        site.named_use == handle
                    }
                })
        })
        .ok_or_else(|| {
            Diagnostic::error(
                "selected operator crash invocation has no checked crash site to carry into Terminal production",
            )
        })?;
    let Some(operator) =
        typed_trees::operator::declaration_by_symbol(program, site.selected_operator)
    else {
        return Err(Diagnostic::error(
            "selected operator crash invocation's site names no operator declaration",
        ));
    };
    if !program
        .operator_parameters(operator)
        .iter()
        .all(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
    {
        return Err(Diagnostic::error(
            "selected operator crash invocation's signature is not all-scalar",
        ));
    }
    if let Some(spelling) = spelling
        && !matches!(
            spelling,
            language_core::operator_spelling::OperatorSpelling::Equal
                | language_core::operator_spelling::OperatorSpelling::NotEqual
                | language_core::operator_spelling::OperatorSpelling::Less
                | language_core::operator_spelling::OperatorSpelling::LessEqual
                | language_core::operator_spelling::OperatorSpelling::Greater
                | language_core::operator_spelling::OperatorSpelling::GreaterEqual
        )
    {
        return Err(Diagnostic::error(
            "selected operator crash invocation is not a selected comparison; no emitted operation carries its contract",
        ));
    }
    Ok(())
}

pub(super) fn resolve_checked_adapter_for_operator(
    checked: &CheckedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
    plan: &effects::provider_plan::ProviderPlan,
    expression: ExpressionHandle,
) -> Result<Option<(symbols::SymbolHandle, String, symbols::SymbolHandle)>, Diagnostic> {
    let overload_identity =
        typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator);
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
    let direct_provider = provider_planning::exact_checked_adapter(
        provider_planning::ProviderPlanningContext::new(
            &checked.typed,
            source::DependencyScope::Product,
        ),
        plan,
        row,
    );
    let specialized = checked
        .typed
        .machine_specializations
        .iter()
        .filter(|specialization| {
            validation::machine_specialization_matches_template_identity(
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
                && (typed_trees::operator::resolve_satisfied_checked_operator_for_conformance(
                    &checked.typed,
                    provider,
                    conformance,
                )
                .is_some_and(|resolved| resolved.symbol == operator.symbol)
                    || typed_trees::operator::resolve_specialized_checked_operator_application(
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
    provider_planning::selected_use_plan(checked, selected_provider_plans, requirement, origin)
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
