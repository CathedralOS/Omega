//! Canonical-empty D29 applications outside the attached-`Unit` plan lane.

use super::super::{
    exact_operator_definition, resolve_checked_adapter_for_operator, resolve_exact_selected_plan,
};
use super::{CheckedNongenericOperatorApplicationRealization, CheckedOperatorAuthoredUseKind};
use checked_trees::{
    CheckedBoundaryOperatorApplicationUseSite, CheckedOperatorRealizationContract,
    CheckedOperatorResolutionStatus, CheckedTrees,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(super) fn derive(
    checked: &CheckedTrees,
    selected_provider_plans: &[effects::provider_plan::ProviderPlan],
    independently_derived_realizations: &[CheckedOperatorRealizationContract],
) -> Result<Vec<CheckedNongenericOperatorApplicationRealization>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    let mut diagnostics = Vec::new();
    for application in &checked.facts.operators.boundary_applications {
        if !application.arguments.is_empty() {
            continue;
        }
        match derive_checked_expression_operator_application_realization(
            checked,
            selected_provider_plans,
            independently_derived_realizations,
            application,
        ) {
            Ok(Some(row)) => rows.push(row),
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    if diagnostics.is_empty() {
        Ok(rows)
    } else {
        Err(diagnostics)
    }
}

fn derive_checked_expression_operator_application_realization(
    checked: &CheckedTrees,
    selected_provider_plans: &[effects::provider_plan::ProviderPlan],
    independently_derived_realizations: &[CheckedOperatorRealizationContract],
    application: &checked_trees::CheckedBoundaryOperatorApplicationDemand,
) -> Result<Option<CheckedNongenericOperatorApplicationRealization>, Diagnostic> {
    let CheckedBoundaryOperatorApplicationUseSite::Expression { expression, origin } =
        application.site
    else {
        return Err(Diagnostic::error(
            "canonical-empty boundary application has no expression use site",
        ));
    };
    let operator = exact_operator_definition(checked, expression, application.requirement_symbol)?;
    if !operator.is_boundary {
        return Err(Diagnostic::error(format!(
            "canonical-empty boundary application at expression {expression:?} does not name a boundary operator",
        )));
    }

    let authored_uses = checked
        .facts
        .operators
        .named_uses
        .iter()
        .filter_map(|(_, operator_use)| {
            (operator_use.expression == expression
                && operator_use.origin == origin
                && operator_use.selected_operator_symbol == application.requirement_symbol)
                .then_some((
                    CheckedOperatorAuthoredUseKind::Named,
                    operator_use.provider_plan_report_fingerprint,
                    operator_use.provider_plan_commitment,
                    true,
                ))
        })
        .chain(
            checked
                .facts
                .operators
                .uses
                .iter()
                .filter_map(|(_, operator_use)| {
                    (operator_use.expression == expression
                        && operator_use.origin == origin
                        && operator_use.selected_operator_symbol == application.requirement_symbol)
                        .then_some((
                            CheckedOperatorAuthoredUseKind::FixedToken(operator_use.spelling),
                            operator_use.provider_plan_report_fingerprint,
                            operator_use.provider_plan_commitment,
                            operator_use.status == CheckedOperatorResolutionStatus::Resolved
                                && operator.spelling == Some(operator_use.spelling)
                                && checked
                                    .facts
                                    .operators
                                    .selected_candidate(operator_use)
                                    .is_some_and(|candidate| {
                                        candidate.operator_symbol == application.requirement_symbol
                                            && candidate.is_boundary
                                    }),
                        ))
                }),
        )
        .collect::<Vec<_>>();
    let [(authored_use_kind, plan_report, plan_commitment, exact_authored_use)] =
        authored_uses.as_slice()
    else {
        return Err(Diagnostic::error(format!(
            "canonical-empty boundary application at expression {expression:?} retains {} exact authored uses; expected one",
            authored_uses.len(),
        )));
    };
    if !exact_authored_use {
        return Err(Diagnostic::error(format!(
            "canonical-empty boundary application at expression {expression:?} does not retain an exact resolved authored use",
        )));
    }
    if !has_exact_authored_selection(
        checked,
        expression,
        *authored_use_kind,
        application.requirement_symbol,
    ) {
        return Ok(None);
    }
    if *plan_report == 0 && plan_commitment.is_empty() {
        // Checking also retains declaration/conformance application shapes.
        // Without selected-plan custody they are not executable D29 uses and
        // cannot publish realization coverage.
        return Ok(None);
    }
    if !operator.lifetime_parameters.is_empty()
        || !checked.typed.operator_type_parameters(operator).is_empty()
    {
        return Err(Diagnostic::error(format!(
            "selected canonical-empty boundary application at expression {expression:?} names a generic boundary operator",
        )));
    }

    let report_matches = selected_provider_plans
        .iter()
        .filter(|plan| plan.report_fingerprint() == *plan_report)
        .collect::<Vec<_>>();
    if let [plan] = report_matches.as_slice()
        && !matches!(
            plan.rows.as_slice(),
            [row]
                if matches!(
                    row.binding,
                    effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
                )
        )
    {
        // Compiler-intrinsic and admitted external roles have their own exact
        // realization projectors. This lane must not preempt their replay or
        // diagnostics merely because they share a checked application demand.
        return Ok(None);
    }

    let plan = resolve_exact_selected_plan(
        selected_provider_plans,
        *plan_report,
        *plan_commitment,
        "canonical-empty operator application",
    )?;
    let Some((realization_machine, _, realization_state)) =
        resolve_checked_adapter_for_operator(checked, operator, plan, expression)?
    else {
        return Ok(None);
    };
    let realization = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == realization_machine)
        .ok_or_else(|| Diagnostic::error("selected nongeneric realization machine is missing"))?;
    if !realization.lifetime_parameters.is_empty()
        || !checked
            .typed
            .machine_type_parameters(realization)
            .is_empty()
    {
        return Err(Diagnostic::error(
            "canonical-empty boundary application selected a generic realization",
        ));
    }

    let contracts = checked
        .facts
        .contract_plans
        .machines
        .iter()
        .filter(|contract| contract.machine == realization_machine)
        .collect::<Vec<_>>();
    let [contract] = contracts.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected nongeneric realization retains {} exact machine contracts; expected one",
            contracts.len(),
        )));
    };
    if contract.commitment.is_zero() {
        return Err(Diagnostic::error(
            "selected nongeneric realization has an empty machine contract commitment",
        ));
    }

    let retained = checked
        .facts
        .operators
        .operator_realization_contracts
        .iter()
        .filter(|contract| {
            contract.machine_symbol() == realization_machine
                && contract.operator_symbol() == application.requirement_symbol
        })
        .collect::<Vec<_>>();
    let derived = independently_derived_realizations
        .iter()
        .filter(|contract| {
            contract.machine_symbol() == realization_machine
                && contract.operator_symbol() == application.requirement_symbol
        })
        .collect::<Vec<_>>();
    let ([retained], [derived]) = (retained.as_slice(), derived.as_slice()) else {
        return Err(Diagnostic::error(
            "selected nongeneric realization lacks one independently replayable operator contract",
        ));
    };
    if *retained != *derived {
        return Err(Diagnostic::error(
            "selected nongeneric realization contract differs from independent compiler rederivation",
        ));
    }

    let requirement_overload_identity =
        typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator);
    if requirement_overload_identity.is_empty() {
        return Err(Diagnostic::error(
            "selected nongeneric realization has an empty requirement identity",
        ));
    }
    Ok(Some(CheckedNongenericOperatorApplicationRealization {
        application_site: application.site,
        authored_use_kind: *authored_use_kind,
        requirement_operator: application.requirement_symbol,
        requirement_overload_identity,
        provider_plan_report_fingerprint: *plan_report,
        provider_plan_commitment: *plan_commitment,
        realization_machine,
        realization_state,
        realization_contract_report_fingerprint: contract.report_fingerprint,
        realization_contract_commitment: contract.commitment,
    }))
}

fn has_exact_authored_selection(
    checked: &CheckedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    use_kind: CheckedOperatorAuthoredUseKind,
    requirement_operator: SymbolHandle,
) -> bool {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
    };

    let expected_kind = match use_kind {
        CheckedOperatorAuthoredUseKind::Named => AuthoredDeclarationSelectionKind::Call,
        CheckedOperatorAuthoredUseKind::FixedToken(_) => AuthoredDeclarationSelectionKind::Operator,
    };
    checked
        .typed
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            checked
                .typed
                .authored_declaration_selections()
                .get(occurrence)
        })
        .any(|selection| {
            selection.kind() == expected_kind
                && matches!(
                    selection.target(),
                    AuthoredDeclarationSelectionTarget::Resolved(target)
                        if target.selected_symbol() == requirement_operator
                )
        })
}
