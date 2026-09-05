//! D29 closed generic checked-body realization joins.
//!
//! A checked use owns demand, ProviderPlan settlement owns selection, and
//! authoritative machine specialization owns the concrete realization. This
//! module rejoins those three compiler-private facts without issuing Terminal,
//! native, admission, or universal generic-family coverage.

use super::{
    CheckedOperatorAuthoredUseKind, exact_operator_definition,
    resolve_checked_adapter_for_operator, resolve_exact_selected_plan,
};
use checked_trees::{
    CheckedBoundaryOperatorApplicationArgument, CheckedBoundaryOperatorApplicationUseSite,
    CheckedOperatorRealizationContract, CheckedOperatorResolutionStatus, CheckedTrees,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

/// One nonempty closed operator application whose selected realization is an
/// authoritative specialization of a checked generic Omega body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedSpecializedOperatorApplicationRealization {
    pub application_site: CheckedBoundaryOperatorApplicationUseSite,
    pub application_arguments: Vec<CheckedBoundaryOperatorApplicationArgument>,
    pub authored_use_kind: CheckedOperatorAuthoredUseKind,
    pub requirement_operator: SymbolHandle,
    pub requirement_overload_identity: String,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub realization_template: SymbolHandle,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub specialization_commitment: typed_trees::typed_trees::MachineSpecializationCommitment,
    pub realization_contract_report_fingerprint: u64,
    pub realization_contract_commitment: checked_trees::MachineContractCommitment,
}

/// Independently rejoin every retained nonempty D29 demand to one exact
/// selected specialized checked body.
pub fn derive_checked_specialized_operator_application_realizations(
    checked: &CheckedTrees,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
) -> Result<Vec<CheckedSpecializedOperatorApplicationRealization>, Vec<Diagnostic>> {
    let independently_derived_realizations =
        typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(&checked.typed);
    let mut rows = Vec::new();
    let mut diagnostics = Vec::new();
    for application in &checked.facts.operators.boundary_applications {
        if application.arguments.is_empty() {
            continue;
        }
        match derive_one(
            checked,
            selected_provider_plans.plans(),
            &independently_derived_realizations,
            application,
        ) {
            Ok(row) => rows.push(row),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    if diagnostics.is_empty() {
        Ok(rows)
    } else {
        Err(diagnostics)
    }
}

fn derive_one(
    checked: &CheckedTrees,
    selected_provider_plans: &[effects::provider_plan::ProviderPlan],
    independently_derived_realizations: &[CheckedOperatorRealizationContract],
    application: &checked_trees::CheckedBoundaryOperatorApplicationDemand,
) -> Result<CheckedSpecializedOperatorApplicationRealization, Diagnostic> {
    let CheckedBoundaryOperatorApplicationUseSite::Expression { expression, origin } =
        application.site
    else {
        return Err(Diagnostic::error(
            "nonempty boundary application has no expression use site",
        ));
    };
    let operator = exact_operator_definition(checked, expression, application.requirement_symbol)?;
    if !operator.is_boundary || checked.typed.operator_type_parameters(operator).is_empty() {
        return Err(Diagnostic::error(format!(
            "nonempty boundary application at expression {expression:?} does not name a generic boundary operator",
        )));
    }

    let authored_uses =
        exact_authored_uses(checked, expression, origin, application.requirement_symbol);
    let [(authored_use_kind, plan_report, plan_commitment)] = authored_uses.as_slice() else {
        return Err(Diagnostic::error(format!(
            "nonempty boundary application at expression {expression:?} retains {} exact selected uses; expected one",
            authored_uses.len(),
        )));
    };
    let plan = resolve_exact_selected_plan(
        selected_provider_plans,
        *plan_report,
        *plan_commitment,
        "specialized operator application",
    )?;
    let Some((realization_machine, _, realization_state)) =
        resolve_checked_adapter_for_operator(checked, operator, plan, expression)?
    else {
        return Err(Diagnostic::error(format!(
            "nonempty boundary application at expression {expression:?} is not backed by a checked-adapter ProviderPlan row",
        )));
    };

    let specializations = checked
        .typed
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.instance == realization_machine)
        .collect::<Vec<_>>();
    let [specialization] = specializations.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected generic realization {realization_machine:?} retains {} exact machine specializations; expected one",
            specializations.len(),
        )));
    };
    if specialization.commitment.is_zero() {
        return Err(Diagnostic::error(
            "selected generic realization has an empty specialization commitment",
        ));
    }
    let replayed = validation::recompute_checked_machine_specialization_commitment(
        checked,
        realization_machine,
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "selected generic realization specialization does not replay: {error}"
        ))
    })?;
    if replayed != specialization.commitment.as_bytes() {
        return Err(Diagnostic::error(
            "selected generic realization has a stale specialization commitment",
        ));
    }
    let retained = specialization
        .operator_realizations
        .iter()
        .filter(|row| row.requirement_symbol == application.requirement_symbol)
        .collect::<Vec<_>>();
    let [retained] = retained.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected generic realization retains {} exact applications for its demanded operator; expected one",
            retained.len(),
        )));
    };
    if !applications_match(checked, application, retained) {
        return Err(Diagnostic::error(
            "selected generic realization application differs from the exact checked demand",
        ));
    }

    let realization = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == realization_machine)
        .ok_or_else(|| Diagnostic::error("selected generic realization machine is missing"))?;
    let entry = checked
        .typed
        .machine_states(realization)
        .first()
        .ok_or_else(|| Diagnostic::error("selected generic realization entry is missing"))?;
    if entry.symbol != realization_state {
        return Err(Diagnostic::error(
            "selected generic realization state differs from its concrete entry",
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
            "selected generic realization retains {} exact machine contracts; expected one",
            contracts.len(),
        )));
    };
    if contract.commitment.is_zero() {
        return Err(Diagnostic::error(
            "selected generic realization has an empty machine contract commitment",
        ));
    }

    let retained_contracts = checked
        .facts
        .operators
        .operator_realization_contracts
        .iter()
        .filter(|contract| {
            contract.machine_symbol() == realization_machine
                && contract.operator_symbol() == application.requirement_symbol
        })
        .collect::<Vec<_>>();
    let derived_contracts = independently_derived_realizations
        .iter()
        .filter(|contract| {
            contract.machine_symbol() == realization_machine
                && contract.operator_symbol() == application.requirement_symbol
        })
        .collect::<Vec<_>>();
    let ([retained_contract], [derived_contract]) =
        (retained_contracts.as_slice(), derived_contracts.as_slice())
    else {
        return Err(Diagnostic::error(
            "selected generic realization lacks one independently replayable operator contract",
        ));
    };
    if *retained_contract != *derived_contract {
        return Err(Diagnostic::error(
            "selected generic realization contract differs from independent compiler rederivation",
        ));
    }

    let requirement_overload_identity =
        typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator);
    if requirement_overload_identity.is_empty() {
        return Err(Diagnostic::error(
            "selected generic realization has an empty requirement identity",
        ));
    }
    Ok(CheckedSpecializedOperatorApplicationRealization {
        application_site: application.site,
        application_arguments: application.arguments.clone(),
        authored_use_kind: *authored_use_kind,
        requirement_operator: application.requirement_symbol,
        requirement_overload_identity,
        provider_plan_report_fingerprint: *plan_report,
        provider_plan_commitment: *plan_commitment,
        realization_template: specialization.template,
        realization_machine,
        realization_state,
        specialization_commitment: specialization.commitment,
        realization_contract_report_fingerprint: contract.report_fingerprint,
        realization_contract_commitment: contract.commitment,
    })
}

fn exact_authored_uses(
    checked: &CheckedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    origin: checked_trees::CheckedValueOrigin,
    requirement: SymbolHandle,
) -> Vec<(
    CheckedOperatorAuthoredUseKind,
    u64,
    checked_trees::CheckedProviderPlanCommitment,
)> {
    let operator = checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator.symbol == requirement);
    checked
        .facts
        .operators
        .named_uses
        .iter()
        .filter_map(|(_, operator_use)| {
            (operator_use.expression == expression
                && operator_use.origin == origin
                && operator_use.selected_operator_symbol == requirement)
                .then_some((
                    CheckedOperatorAuthoredUseKind::Named,
                    operator_use.provider_plan_report_fingerprint,
                    operator_use.provider_plan_commitment,
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
                        && operator_use.selected_operator_symbol == requirement
                        && operator_use.status == CheckedOperatorResolutionStatus::Resolved
                        && operator.is_some_and(|operator| {
                            operator.is_boundary
                                && operator.spelling == Some(operator_use.spelling)
                                && checked
                                    .facts
                                    .operators
                                    .selected_candidate(operator_use)
                                    .is_some_and(|candidate| {
                                        candidate.operator_symbol == requirement
                                            && candidate.is_boundary
                                    })
                        }))
                    .then_some((
                        CheckedOperatorAuthoredUseKind::FixedToken(operator_use.spelling),
                        operator_use.provider_plan_report_fingerprint,
                        operator_use.provider_plan_commitment,
                    ))
                }),
        )
        .collect()
}

pub(super) fn applications_match(
    checked: &CheckedTrees,
    demand_row: &checked_trees::CheckedBoundaryOperatorApplicationDemand,
    realization: &typed_trees::operator::ClosedOperatorRealizationApplication,
) -> bool {
    validation::checked_operator_application_matches_realization(
        &checked.typed,
        demand_row,
        realization,
    )
}
