//! D29 canonical-empty-application realization joins.
//!
//! This module derives review-projector input from facts already retained by
//! checking and selected dispatch. It creates no authority or execution
//! evidence: every returned identity is either a compiler-private checked
//! coordinate or an existing strong commitment.

use super::{
    exact_operator_definition, resolve_checked_adapter_for_operator, resolve_exact_selected_plan,
};
use psi_checked_trees::{
    CheckedBoundaryOperatorApplicationUseSite, CheckedOperatorRealizationContract,
    CheckedOperatorResolutionStatus, CheckedTrees, CheckedUnitCallCoordinate,
    CheckedUnitEffectOperationPlan, CheckedValueOrigin, CheckedValueStatementRole,
};
use psi_diagnostics::Diagnostic;
use psi_language_core::operator_spelling::OperatorSpelling;
use psi_symbols::SymbolHandle;

/// The retained authored-use lane rejoined to one selected attached-Unit
/// occurrence. The expression and origin live in `application_site`; this
/// discriminant prevents a downstream projector from conflating named and
/// fixed-token applications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedOperatorAuthoredUseKind {
    Named,
    FixedToken(OperatorSpelling),
}

/// One canonical-empty operator application whose selected realization is a
/// nongeneric checked body.
///
/// This is read-only compiler custody for a downstream review projector. It
/// does not claim generic-family coverage, native execution, or admission.
/// The retained realization contract's private snapshots are equality-checked
/// here and intentionally are not exposed as stable projector input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedNongenericOperatorApplicationRealization {
    pub source_machine: SymbolHandle,
    pub source_state: SymbolHandle,
    pub call_coordinate: CheckedUnitCallCoordinate,
    pub application_site: CheckedBoundaryOperatorApplicationUseSite,
    pub authored_use_kind: CheckedOperatorAuthoredUseKind,
    pub requirement_operator: SymbolHandle,
    pub requirement_overload_identity: String,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: psi_checked_trees::CheckedProviderPlanCommitment,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_contract_report_fingerprint: u64,
    pub realization_contract_commitment: psi_checked_trees::MachineContractCommitment,
}

/// Rejoin actual selected scalar calls retained for attached Unit machines to
/// their canonical empty application and exact nongeneric checked body.
///
/// Generic operator applications and applications outside attached Unit
/// machines are deliberately outside this derivation. Once an attached-Unit
/// selected call names a nongeneric operator, every demanded join is
/// fail-closed: absence, ambiguity, or substitution rejects the derivation.
pub fn derive_checked_nongeneric_operator_application_realizations(
    checked: &CheckedTrees,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
) -> Result<Vec<CheckedNongenericOperatorApplicationRealization>, Vec<Diagnostic>> {
    let independently_derived_realizations =
        psi_typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &checked.typed,
        );
    let mut rows = Vec::new();
    let mut diagnostics = Vec::new();

    for unit_machine in &checked.facts.flow.terminal_unit_effects.machines {
        if !unit_machine.operations.iter().any(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. }
            )
        }) {
            continue;
        }
        let source_machines = checked
            .typed
            .machines()
            .iter()
            .filter(|machine| machine.symbol == unit_machine.machine)
            .collect::<Vec<_>>();
        let [source_machine] = source_machines.as_slice() else {
            diagnostics.push(Diagnostic::error(format!(
                "selected Unit plan resolves source symbol {:?} to {} typed machines",
                unit_machine.machine,
                source_machines.len(),
            )));
            continue;
        };
        if source_machine.attached_data.is_none() {
            continue;
        }
        let source_states = checked
            .typed
            .machine_states(source_machine)
            .iter()
            .filter(|state| state.symbol == unit_machine.state)
            .collect::<Vec<_>>();
        let [source_state] = source_states.as_slice() else {
            diagnostics.push(Diagnostic::error(format!(
                "selected attached-Unit plan resolves source state symbol {:?} to {} typed states",
                unit_machine.state,
                source_states.len(),
            )));
            continue;
        };
        if !is_unit_return(&checked.typed, source_state.return_type) {
            diagnostics.push(Diagnostic::error(format!(
                "selected attached-Unit plan at {:?}/{:?} no longer has a Unit entry state",
                unit_machine.machine, unit_machine.state,
            )));
            continue;
        }

        for operation in &unit_machine.operations {
            let CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                coordinate,
                requirement_operator,
                provider_plan_report_fingerprint,
                provider_plan_commitment,
                realization_machine,
                realization_state,
                realization_contract_report_fingerprint,
                realization_contract_commitment,
                ..
            } = operation
            else {
                continue;
            };
            match derive_one(
                checked,
                selected_provider_plans.plans(),
                &independently_derived_realizations,
                unit_machine.machine,
                unit_machine.state,
                *coordinate,
                *requirement_operator,
                *provider_plan_report_fingerprint,
                *provider_plan_commitment,
                *realization_machine,
                *realization_state,
                *realization_contract_report_fingerprint,
                *realization_contract_commitment,
            ) {
                Ok(Some(row)) => rows.push(row),
                Ok(None) => {}
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(rows)
    } else {
        Err(diagnostics)
    }
}

#[allow(clippy::too_many_arguments)]
fn derive_one(
    checked: &CheckedTrees,
    selected_provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    independently_derived_realizations: &[CheckedOperatorRealizationContract],
    source_machine: SymbolHandle,
    source_state: SymbolHandle,
    call_coordinate: CheckedUnitCallCoordinate,
    requirement_operator: SymbolHandle,
    provider_plan_report_fingerprint: u64,
    provider_plan_commitment: psi_checked_trees::CheckedProviderPlanCommitment,
    realization_machine: SymbolHandle,
    realization_state: SymbolHandle,
    realization_contract_report_fingerprint: u64,
    realization_contract_commitment: psi_checked_trees::MachineContractCommitment,
) -> Result<Option<CheckedNongenericOperatorApplicationRealization>, Diagnostic> {
    let statement_index = usize::try_from(call_coordinate.statement_index).map_err(|_| {
        Diagnostic::error("selected attached-Unit operator statement coordinate exceeds usize")
    })?;
    let origin = CheckedValueOrigin::StateStatement {
        machine_symbol: source_machine,
        state_symbol: source_state,
        statement_index,
        role: CheckedValueStatementRole::LocalInitializer,
    };

    let operator = exact_operator_definition(
        checked,
        psi_typed_trees::expression::ExpressionHandle::invalid(),
        requirement_operator,
    )?;
    if !operator.is_boundary {
        return Err(Diagnostic::error(format!(
            "selected attached-Unit operator call at {origin:?} names a non-boundary operator",
        )));
    }
    if !operator.lifetime_parameters.is_empty()
        || !checked.typed.operator_type_parameters(operator).is_empty()
    {
        return Ok(None);
    }

    let authored_uses = checked
        .facts
        .operators
        .named_uses
        .iter()
        .filter_map(|(_, operator_use)| {
            (operator_use.origin == origin
                && operator_use.selected_operator_symbol == requirement_operator)
                .then_some((
                    operator_use.expression,
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
                    (operator_use.origin == origin
                        && operator_use.selected_operator_symbol == requirement_operator)
                        .then_some((
                            operator_use.expression,
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
                                        candidate.operator_symbol == requirement_operator
                                            && candidate.is_boundary
                                    }),
                        ))
                }),
        )
        .collect::<Vec<_>>();
    let [
        (
            expression,
            authored_use_kind,
            authored_plan_report,
            authored_plan_commitment,
            exact_authored_use,
        ),
    ] = authored_uses.as_slice()
    else {
        return Err(Diagnostic::error(format!(
            "selected attached-Unit operator call at {origin:?} retained {} authored named/fixed-token uses for its requirement; expected exactly one",
            authored_uses.len(),
        )));
    };
    if !exact_authored_use
        || *authored_plan_report != provider_plan_report_fingerprint
        || *authored_plan_commitment != provider_plan_commitment
    {
        return Err(Diagnostic::error(format!(
            "selected attached-Unit operator call at {origin:?} does not rejoin its exact authored use and selected ProviderPlan identity",
        )));
    }
    let application_site = CheckedBoundaryOperatorApplicationUseSite::Expression {
        expression: *expression,
        origin,
    };
    let applications = checked
        .facts
        .operators
        .boundary_applications
        .iter()
        .filter(|application| application.site == application_site)
        .collect::<Vec<_>>();
    let [application] = applications.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected attached-Unit operator call at {origin:?} retained {} exact application demands; expected exactly one canonical empty application",
            applications.len(),
        )));
    };
    if application.requirement_symbol != requirement_operator || !application.arguments.is_empty() {
        return Err(Diagnostic::error(format!(
            "nongeneric selected attached-Unit operator call at {origin:?} retained a substituted requirement or nonempty application",
        )));
    }

    let plan = resolve_exact_selected_plan(
        selected_provider_plans,
        provider_plan_report_fingerprint,
        provider_plan_commitment,
        "selected attached-Unit operator application",
    )?;
    let Some((expected_machine, _, expected_state)) =
        resolve_checked_adapter_for_operator(checked, operator, plan, *expression)?
    else {
        return Err(Diagnostic::error(format!(
            "selected attached-Unit operator application at {origin:?} is not backed by a checked-adapter ProviderPlan row",
        )));
    };
    if expected_machine != realization_machine || expected_state != realization_state {
        return Err(Diagnostic::error(format!(
            "selected attached-Unit operator application at {origin:?} retains realization {realization_machine:?}/{realization_state:?}, but its exact ProviderPlan selects {expected_machine:?}/{expected_state:?}",
        )));
    }

    let realization_machines = checked
        .typed
        .machines()
        .iter()
        .filter(|machine| machine.symbol == realization_machine)
        .collect::<Vec<_>>();
    let [realization] = realization_machines.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected attached-Unit realization symbol {realization_machine:?} resolves to {} typed machines",
            realization_machines.len(),
        )));
    };
    if !realization.lifetime_parameters.is_empty()
        || !checked
            .typed
            .machine_type_parameters(realization)
            .is_empty()
    {
        return Err(Diagnostic::error(format!(
            "selected attached-Unit realization `{}` is generic",
            realization.name,
        )));
    }

    let machine_contracts = checked
        .facts
        .contract_plans
        .machines
        .iter()
        .filter(|contract| contract.machine == realization_machine)
        .collect::<Vec<_>>();
    let [machine_contract] = machine_contracts.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected attached-Unit realization `{}` retained {} exact machine contracts; expected one",
            realization.name,
            machine_contracts.len(),
        )));
    };
    if realization_contract_commitment.is_zero()
        || machine_contract.report_fingerprint != realization_contract_report_fingerprint
        || machine_contract.commitment != realization_contract_commitment
    {
        return Err(Diagnostic::error(format!(
            "selected attached-Unit realization `{}` does not rejoin its exact machine contract commitment",
            realization.name,
        )));
    }

    let retained_realizations = checked
        .facts
        .operators
        .operator_realization_contracts
        .iter()
        .filter(|contract| {
            contract.machine_symbol() == realization_machine
                && contract.operator_symbol() == requirement_operator
        })
        .collect::<Vec<_>>();
    let [retained_realization] = retained_realizations.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected attached-Unit realization `{}` retained {} exact operator-realization contracts; expected one",
            realization.name,
            retained_realizations.len(),
        )));
    };
    let derived_realizations = independently_derived_realizations
        .iter()
        .filter(|contract| {
            contract.machine_symbol() == realization_machine
                && contract.operator_symbol() == requirement_operator
        })
        .collect::<Vec<_>>();
    let [derived_realization] = derived_realizations.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected attached-Unit realization `{}` independently derives {} exact operator-realization contracts; expected one",
            realization.name,
            derived_realizations.len(),
        )));
    };
    if *retained_realization != *derived_realization {
        return Err(Diagnostic::error(format!(
            "selected attached-Unit realization `{}` retained an operator-realization contract that differs from independent compiler rederivation",
            realization.name,
        )));
    }

    let requirement_overload_identity =
        psi_typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator);
    if requirement_overload_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected attached-Unit operator call at {origin:?} has an empty canonical overload identity",
        )));
    }

    Ok(Some(CheckedNongenericOperatorApplicationRealization {
        source_machine,
        source_state,
        call_coordinate,
        application_site,
        authored_use_kind: *authored_use_kind,
        requirement_operator,
        requirement_overload_identity,
        provider_plan_report_fingerprint,
        provider_plan_commitment,
        realization_machine,
        realization_state,
        realization_contract_report_fingerprint,
        realization_contract_commitment,
    }))
}

fn is_unit_return(
    typed: &psi_typed_trees::TypedTrees,
    mut type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> bool {
    loop {
        match typed.type_reference_table.type_reference(type_reference) {
            psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference = *base_type;
            }
            psi_typed_trees::types::TypeReferenceNode::Unit => return true,
            _ => return false,
        }
    }
}
