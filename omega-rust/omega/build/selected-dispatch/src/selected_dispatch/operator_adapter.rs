//! Checked boundary-operator ProviderPlan execution bridge.
//!
//! Semantic checking and retained facts continue to name the public boundary
//! operator. After selection, a named or fixed-token use whose exact plan row
//! is a checked adapter redirects execution to that ordinary Omega machine
//! body. This is the operator analogue of boundary-trait adapter dispatch;
//! compiler intrinsics remain in `float_intrinsic_dispatch`.

mod application_realization;
mod specialized_application_realization;
mod spelled;
mod unit;

pub use application_realization::{
    CheckedNongenericOperatorApplicationRealization, CheckedOperatorAuthoredUseKind,
    derive_checked_nongeneric_operator_application_realizations,
};
pub use specialized_application_realization::{
    CheckedSpecializedOperatorApplicationRealization,
    derive_checked_specialized_operator_application_realizations,
};

use checked_trees::{
    CheckedTrees, CheckedUnitEffectOperationPlan, CheckedValueOrigin, CheckedValueStatementRole,
};
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use language_core::CallOperationalAcknowledgementOrigin;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OperatorAdapterRewrite {
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    requirement_operator: symbols::SymbolHandle,
    provider_plan_report_fingerprint: u64,
    provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    machine_symbol: symbols::SymbolHandle,
    machine: String,
    entry_symbol: symbols::SymbolHandle,
    source: OperatorAdapterSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OperatorAdapterSource {
    NamedCall,
    Spelled(Box<[ExpressionHandle]>),
}

/// Rejoin every retained selected Unit call to the exact ProviderPlan still
/// owned by the compiler before Terminal production. Checked Psi deliberately
/// carries only the selected join; Omega retains the complete plan whose
/// strong identity authorizes that join.
pub fn validate_selected_operator_terminal_custody(
    checked: &CheckedTrees,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
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
    let mut diagnostics = Vec::new();
    for machine in &checked.facts.flow.terminal_unit_effects.machines {
        for operation in &machine.operations {
            let (
                coordinate,
                requirement_operator,
                provider_plan_report_fingerprint,
                provider_plan_commitment,
                realization_machine,
                realization_state,
            ) = match operation {
                CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                    coordinate,
                    requirement_operator,
                    provider_plan_report_fingerprint,
                    provider_plan_commitment,
                    realization_machine,
                    realization_state,
                    ..
                }
                | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                    coordinate,
                    requirement_operator,
                    provider_plan_report_fingerprint,
                    provider_plan_commitment,
                    realization_machine,
                    realization_state,
                    ..
                }
                | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                    coordinate,
                    requirement_operator,
                    provider_plan_report_fingerprint,
                    provider_plan_commitment,
                    realization_machine,
                    realization_state,
                    ..
                } => (
                    coordinate,
                    requirement_operator,
                    provider_plan_report_fingerprint,
                    provider_plan_commitment,
                    realization_machine,
                    realization_state,
                ),
                _ => continue,
            };
            let origin = CheckedValueOrigin::StateStatement {
                machine_symbol: machine.machine,
                state_symbol: machine.state,
                statement_index: match usize::try_from(coordinate.statement_index) {
                    Ok(statement_index) => statement_index,
                    Err(_) => {
                        diagnostics.push(Diagnostic::error(
                            "selected Unit operator statement coordinate exceeds usize",
                        ));
                        continue;
                    }
                },
                role: CheckedValueStatementRole::LocalInitializer,
            };
            let uses = checked
                .facts
                .operators
                .named_uses
                .iter()
                .map(|(_, operator_use)| {
                    (
                        operator_use.expression,
                        operator_use.origin,
                        operator_use.selected_operator_symbol,
                        operator_use.provider_plan_report_fingerprint,
                        operator_use.provider_plan_commitment,
                    )
                })
                .chain(
                    checked
                        .facts
                        .operators
                        .uses
                        .iter()
                        .map(|(_, operator_use)| {
                            (
                                operator_use.expression,
                                operator_use.origin,
                                operator_use.selected_operator_symbol,
                                operator_use.provider_plan_report_fingerprint,
                                operator_use.provider_plan_commitment,
                            )
                        }),
                )
                .filter(|(_, use_origin, operator, report, commitment)| {
                    *use_origin == origin
                        && *operator == *requirement_operator
                        && *report == *provider_plan_report_fingerprint
                        && *commitment == *provider_plan_commitment
                })
                .collect::<Vec<_>>();
            let [(expression, _, _, _, _)] = uses.as_slice() else {
                diagnostics.push(Diagnostic::error(format!(
                    "selected Unit operator call at {:?} retained {} matching authored uses; expected exactly one",
                    origin,
                    uses.len(),
                )));
                continue;
            };
            let plan = match resolve_exact_selected_plan(
                selected_provider_plans.plans(),
                *provider_plan_report_fingerprint,
                *provider_plan_commitment,
                "selected Unit operator call",
            ) {
                Ok(plan) => plan,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            let operator =
                match exact_operator_definition(checked, *expression, *requirement_operator) {
                    Ok(operator) => operator,
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic);
                        continue;
                    }
                };
            match resolve_checked_adapter_for_operator(checked, operator, plan, *expression) {
                Ok(Some((expected_machine, _, expected_state)))
                    if expected_machine == *realization_machine
                        && expected_state == *realization_state => {}
                Ok(Some((expected_machine, expected_name, expected_state))) => {
                    diagnostics.push(Diagnostic::error(format!(
                        "selected Unit operator call at {:?} retains realization {:?}/{:?}, but its exact ProviderPlan selects `{expected_name}` at {:?}/{:?}",
                        origin,
                        realization_machine,
                        realization_state,
                        expected_machine,
                        expected_state,
                    )));
                }
                Ok(None) => diagnostics.push(Diagnostic::error(format!(
                    "selected Unit operator call at {:?} is not backed by a checked-adapter ProviderPlan row",
                    origin,
                ))),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
    }
    for machine in &checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .selected_operator_machines
    {
        let origin = CheckedValueOrigin::StateStatement {
            machine_symbol: machine.machine,
            state_symbol: machine.state,
            statement_index: match usize::try_from(machine.return_statement_ordinal) {
                Ok(statement_index) => statement_index,
                Err(_) => {
                    diagnostics.push(Diagnostic::error(
                        "selected structural operator statement coordinate exceeds usize",
                    ));
                    continue;
                }
            },
            role: CheckedValueStatementRole::Expression,
        };
        let uses = checked
            .facts
            .operators
            .named_uses
            .iter()
            .map(|(_, operator_use)| {
                (
                    operator_use.expression,
                    operator_use.origin,
                    operator_use.selected_operator_symbol,
                    operator_use.provider_plan_report_fingerprint,
                    operator_use.provider_plan_commitment,
                )
            })
            .chain(
                checked
                    .facts
                    .operators
                    .uses
                    .iter()
                    .map(|(_, operator_use)| {
                        (
                            operator_use.expression,
                            operator_use.origin,
                            operator_use.selected_operator_symbol,
                            operator_use.provider_plan_report_fingerprint,
                            operator_use.provider_plan_commitment,
                        )
                    }),
            )
            .filter(|(_, use_origin, operator, report, commitment)| {
                *use_origin == origin
                    && *operator == machine.requirement_operator
                    && *report == machine.provider_plan_report_fingerprint
                    && *commitment == machine.provider_plan_commitment
            })
            .collect::<Vec<_>>();
        let [(expression, _, _, _, _)] = uses.as_slice() else {
            diagnostics.push(Diagnostic::error(format!(
                "selected structural operator at {origin:?} retained {} matching authored uses; expected exactly one",
                uses.len(),
            )));
            continue;
        };
        let plan = match resolve_exact_selected_plan(
            selected_provider_plans.plans(),
            machine.provider_plan_report_fingerprint,
            machine.provider_plan_commitment,
            "selected structural operator call",
        ) {
            Ok(plan) => plan,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        let operator =
            match exact_operator_definition(checked, *expression, machine.requirement_operator) {
                Ok(operator) => operator,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
        match resolve_checked_adapter_for_operator(checked, operator, plan, *expression) {
            Ok(Some((expected_machine, _, expected_state)))
                if expected_machine == machine.realization_machine
                    && expected_state == machine.realization_state => {}
            Ok(Some((expected_machine, expected_name, expected_state))) => {
                diagnostics.push(Diagnostic::error(format!(
                    "selected structural operator at {origin:?} retains realization {:?}/{:?}, but its exact ProviderPlan selects `{expected_name}` at {:?}/{:?}",
                    machine.realization_machine,
                    machine.realization_state,
                    expected_machine,
                    expected_state,
                )));
            }
            Ok(None) => diagnostics.push(Diagnostic::error(format!(
                "selected structural operator at {origin:?} is not backed by a checked-adapter ProviderPlan row",
            ))),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(super) fn plan_selected_operator_adapter_rewrites(
    checked: &CheckedTrees,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
) -> Result<Vec<OperatorAdapterRewrite>, Vec<Diagnostic>> {
    let mut rewrites = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, operator_use) in checked.facts.operators.named_uses.iter() {
        if operator_use.provider_plan_report_fingerprint == 0
            && operator_use.provider_plan_commitment.is_empty()
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
        if operator_use.occurrence != checked_trees::CheckedOperatorOccurrence::Expression {
            continue;
        }
        if operator_use.provider_plan_report_fingerprint == 0
            && operator_use.provider_plan_commitment.is_empty()
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

pub(super) fn apply_selected_operator_adapter_rewrites(
    checked: &mut CheckedTrees,
    rewrites: &[OperatorAdapterRewrite],
    source_edits: &mut crate::source_edits::SourceEditBuilder,
) {
    for rewrite in rewrites {
        source_edits.expression(&checked.typed, rewrite.expression);
        let replacement = match &rewrite.source {
            OperatorAdapterSource::NamedCall => {
                let ExpressionNode::Call(mut call) = checked
                    .typed
                    .expression_table
                    .expression(rewrite.expression)
                    .clone()
                else {
                    unreachable!("validated named operator rewrite ceased to be a call")
                };
                call.receiver = ExpressionHandle::invalid();
                call.target = typed_trees::name::Identifier::generated(rewrite.machine.clone());
                call.target_symbol = rewrite.entry_symbol;
                ExpressionNode::Call(call)
            }
            OperatorAdapterSource::Spelled(operands) => {
                let arguments = checked
                    .typed
                    .expression_table
                    .insert_expression_handles(operands.iter().copied());
                ExpressionNode::Call(TableCallExpression {
                    receiver: ExpressionHandle::invalid(),
                    target_symbol: rewrite.entry_symbol,
                    target: typed_trees::name::Identifier::generated(rewrite.machine.clone()),
                    static_machine_parameter: symbols::SymbolHandle::invalid(),
                    static_requirement_dispatch: None,
                    machine_arguments: Box::new([]),
                    quotient_operation: None,
                    private_layout_operation: None,
                    arguments,
                    evidence_arguments: Box::new([]),
                    operational_acknowledgement: language_core::CallOperationalAcknowledgement {
                        origin: CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                        acknowledges_suspend: false,
                        acknowledges_block: false,
                    },
                })
            }
        };
        *checked
            .typed
            .expression_table
            .expression_mut(rewrite.expression) = replacement;
    }
}

pub(super) fn selected_operator_applications(
    checked: &CheckedTrees,
    rewrites: &[OperatorAdapterRewrite],
) -> Result<Vec<typed_trees_to_checked_trees::SelectedOperatorApplication>, Diagnostic> {
    rewrites
        .iter()
        .map(|rewrite| unit::selected_application(checked, rewrite))
        .collect()
}

pub(super) fn validate_selected_unit_applications(
    checked: &CheckedTrees,
    rewrites: &[OperatorAdapterRewrite],
) -> Result<(), Diagnostic> {
    for rewrite in rewrites {
        unit::validate_selected_unit_application(checked, rewrite)?;
    }
    Ok(())
}

fn resolve_selected_operator_adapter_call(
    checked: &CheckedTrees,
    selected_provider_plans: &[effects::provider_plan::ProviderPlan],
    operator_use: &checked_trees::CheckedNamedOperatorUseFact,
) -> Result<Option<OperatorAdapterRewrite>, Diagnostic> {
    let plan = resolve_exact_selected_plan(
        selected_provider_plans,
        operator_use.provider_plan_report_fingerprint,
        operator_use.provider_plan_commitment,
        "named operator use",
    )?;

    resolve_operator_adapter_call(checked, operator_use, plan)
}

fn resolve_operator_adapter_call(
    checked: &CheckedTrees,
    operator_use: &checked_trees::CheckedNamedOperatorUseFact,
    plan: &effects::provider_plan::ProviderPlan,
) -> Result<Option<OperatorAdapterRewrite>, Diagnostic> {
    unit::validate_selected_unit_source_shape(
        checked,
        operator_use.expression,
        operator_use.origin,
    )?;
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
        typed_trees::operator::resolve_named_expression_call(&checked.typed, call)
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

    let Some((machine_symbol, machine, entry_symbol)) =
        resolve_checked_adapter_for_operator(checked, operator, plan, operator_use.expression)?
    else {
        return Ok(None);
    };

    Ok(Some(OperatorAdapterRewrite {
        expression: operator_use.expression,
        origin: operator_use.origin,
        requirement_operator: operator_use.selected_operator_symbol,
        provider_plan_report_fingerprint: operator_use.provider_plan_report_fingerprint,
        provider_plan_commitment: operator_use.provider_plan_commitment,
        machine_symbol,
        machine,
        entry_symbol,
        source: OperatorAdapterSource::NamedCall,
    }))
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
    let direct_provider = provider_planning::exact_checked_adapter(&checked.typed, plan, row);
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

pub(super) fn resolve_exact_selected_plan<'plans>(
    selected_provider_plans: &'plans [effects::provider_plan::ProviderPlan],
    report_fingerprint: u64,
    commitment: checked_trees::CheckedProviderPlanCommitment,
    use_label: &str,
) -> Result<&'plans effects::provider_plan::ProviderPlan, Diagnostic> {
    if commitment.is_empty() {
        return Err(Diagnostic::error(format!(
            "{use_label} carries ProviderPlan report fingerprint {report_fingerprint:#018x} without an exact commitment",
        )));
    }
    let report_matches = selected_provider_plans
        .iter()
        .filter(|plan| plan.report_fingerprint() == report_fingerprint)
        .collect::<Vec<_>>();
    let plans = report_matches
        .iter()
        .copied()
        .filter(|plan| plan.identity_digest().as_bytes() == commitment.as_bytes())
        .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        return Err(Diagnostic::error(
            match (report_matches.len(), plans.len()) {
                (1, 0) => format!(
                    "{use_label} ProviderPlan report fingerprint {report_fingerprint:#018x} has an exact commitment that does not match the selected plan",
                ),
                (0, _) => format!(
                    "{use_label} carries unknown ProviderPlan report fingerprint {report_fingerprint:#018x}",
                ),
                (_, count) => format!(
                    "{use_label} ProviderPlan report fingerprint {report_fingerprint:#018x} and exact commitment match {count} selected plans",
                ),
            },
        ));
    };
    Ok(*plan)
}

#[cfg(test)]
mod tests;
