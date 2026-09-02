//! Recheck exact selected boundary-operator custody before Unit lowering.

use super::*;

mod structural_realizations;
pub(super) use structural_realizations::lower_selected_structural_scalar_realizations;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_selected_operator_scalar_call(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
    coordinate: psi_checked_trees::CheckedUnitCallCoordinate,
    result: psi_checked_trees::CheckedUnitScalarResultBindingPlan,
    requirement_operator: psi_symbols::SymbolHandle,
    provider_plan_report_fingerprint: u64,
    provider_plan_commitment: psi_checked_trees::CheckedProviderPlanCommitment,
    realization_machine: psi_symbols::SymbolHandle,
    realization_state: psi_symbols::SymbolHandle,
    realization_contract_report_fingerprint: u64,
    realization_contract_commitment: psi_checked_trees::MachineContractCommitment,
    service_reach: psi_language_semantics::ServiceReachSummary,
    scalar_argument_count: usize,
) -> Result<psi_checked_trees::expression::ExpressionHandle, LoweringError> {
    let origin = psi_checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol: machine.machine,
        state_symbol: machine.state,
        statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
            LoweringError::Unsupported("selected operator statement coordinate exceeds usize")
        })?,
        role: psi_checked_trees::CheckedValueStatementRole::LocalInitializer,
    };
    let exact_uses = checked
        .facts
        .operators
        .named_uses
        .iter()
        .filter(|(_, operator_use)| {
            operator_use.origin == origin
                && operator_use.selected_operator_symbol == requirement_operator
                && operator_use.provider_plan_report_fingerprint == provider_plan_report_fingerprint
                && operator_use.provider_plan_commitment == provider_plan_commitment
        })
        .map(|(_, operator_use)| operator_use.expression)
        .chain(
            checked
                .facts
                .operators
                .uses
                .iter()
                .filter(|(_, operator_use)| {
                    operator_use.origin == origin
                        && operator_use.selected_operator_symbol == requirement_operator
                        && operator_use.provider_plan_report_fingerprint
                            == provider_plan_report_fingerprint
                        && operator_use.provider_plan_commitment == provider_plan_commitment
                })
                .map(|(_, operator_use)| operator_use.expression),
        )
        .collect::<Vec<_>>();
    if coordinate.call_ordinal != 0
        || provider_plan_report_fingerprint == 0
        || provider_plan_commitment.is_empty()
        || exact_uses.len() != 1
    {
        return unsupported(
            "selected Unit operator application does not rejoin one exact checked authored use",
        );
    }

    let realization = checked
        .typed
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == realization_machine)
        .and_then(|realization| {
            checked
                .typed
                .machine_states(realization)
                .iter()
                .find(|state| state.symbol == realization_state)
        })
        .ok_or(LoweringError::Unsupported(
            "selected Unit operator realization lost its exact checked entry",
        ))?;
    let contract = checked
        .facts
        .contract_plans
        .for_machine(realization_machine)
        .ok_or(LoweringError::Unsupported(
            "selected Unit operator realization has no checked contract",
        ))?;
    let realization_reaches = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| {
            state.machine_symbol == realization_machine && state.state_symbol == realization_state
        })
        .map(|(_, state)| state.service_reach)
        .collect::<Vec<_>>();
    let realization_contract_rows = checked
        .facts
        .operators
        .operator_realization_contracts
        .iter()
        .filter(|row| {
            row.machine_symbol() == realization_machine
                && row.operator_symbol() == requirement_operator
        })
        .count();
    if contract.report_fingerprint != realization_contract_report_fingerprint
        || contract.commitment != realization_contract_commitment
        || realization_contract_rows != 1
        || checked
            .typed
            .primitive_type_reference(realization.return_type)
            != Some(result.primitive_type)
        || checked.typed.state_parameters(realization).len() != scalar_argument_count
        || realization_reaches.as_slice() != [service_reach]
    {
        return unsupported(
            "selected Unit operator realization drifted from its checked machine, state, contract, result, arguments, or reach",
        );
    }
    if !checked
        .facts
        .service_reaches
        .rows
        .services(service_reach.direct)
        .is_empty()
        || !checked
            .facts
            .service_reaches
            .rows
            .services(service_reach.transitive)
            .is_empty()
    {
        return unsupported(
            "selected scalar realization with services requires terminal scalar service lowering",
        );
    }
    Ok(exact_uses[0])
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_selected_operator_structural_scalar_call(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
    coordinate: psi_checked_trees::CheckedUnitCallCoordinate,
    result: psi_checked_trees::CheckedUnitScalarResultBindingPlan,
    requirement_operator: psi_symbols::SymbolHandle,
    provider_plan_report_fingerprint: u64,
    provider_plan_commitment: psi_checked_trees::CheckedProviderPlanCommitment,
    realization_machine: psi_symbols::SymbolHandle,
    realization_state: psi_symbols::SymbolHandle,
    realization_contract_report_fingerprint: u64,
    realization_contract_commitment: psi_checked_trees::MachineContractCommitment,
    service_reach: psi_language_semantics::ServiceReachSummary,
    scalar_arguments: &[psi_checked_trees::CheckedScalarExpression],
    structural_arguments: &[psi_checked_trees::CheckedUnitStructuralArgumentPlan],
) -> Result<(), LoweringError> {
    let authored_expression = validate_selected_operator_scalar_call(
        checked,
        machine,
        coordinate,
        result,
        requirement_operator,
        provider_plan_report_fingerprint,
        provider_plan_commitment,
        realization_machine,
        realization_state,
        realization_contract_report_fingerprint,
        realization_contract_commitment,
        service_reach,
        scalar_arguments.len() + structural_arguments.len(),
    )?;
    let realizations = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .iter()
        .filter(|plan| plan.machine == realization_machine && plan.state == realization_state)
        .collect::<Vec<_>>();
    let [realization] = realizations.as_slice() else {
        return unsupported(
            "selected structural Unit operator does not rejoin one checked realization",
        );
    };
    if realization.result_type != result.primitive_type
        || realization.scalar_parameters.len() != scalar_arguments.len()
        || realization.structural_parameters.len() != structural_arguments.len()
        || !machine.entry_claims.is_empty()
        || machine.structural_parameters.len() != structural_arguments.len()
        || machine.structural_parameters.iter().any(|parameter| {
            parameter.is_self
                || parameter.multiplicity != Multiplicity::Affine
                || parameter.access != psi_checked_trees::CheckedStructuralAccess::Owned
                || !parameter.qualifications.is_empty()
                || parameter.fused_service_erasure.is_some()
        })
    {
        return unsupported(
            "selected structural Unit operator exceeds claim-free owned affine custody",
        );
    }
    for (argument, target) in scalar_arguments.iter().zip(&realization.scalar_parameters) {
        if lower_checked_scalar_expression(argument)?.scalar_type()
            != terminal_scalar_type(target.primitive_type)?
        {
            return unsupported(
                "selected structural Unit operator scalar operands drifted from their realization",
            );
        }
    }
    let origin = psi_checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol: machine.machine,
        state_symbol: machine.state,
        statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
            LoweringError::Unsupported("selected operator statement coordinate exceeds usize")
        })?,
        role: psi_checked_trees::CheckedValueStatementRole::LocalInitializer,
    };
    let rederived =
        psi_typed_trees_to_checked_trees::rederive_selected_operator_structural_scalar_arguments(
            checked,
            authored_expression,
            origin,
            realization_machine,
            realization_state,
        )
        .ok_or(LoweringError::Unsupported(
            "selected structural Unit operator arguments cannot be rederived from authored source",
        ))?;
    let retained_structural_source_positions = structural_arguments
        .iter()
        .map(|argument| {
            let index = usize::try_from(argument.source_parameter_index()?).ok()?;
            machine
                .structural_parameters
                .get(index)
                .map(|parameter| parameter.position)
        })
        .collect::<Option<Vec<_>>>();
    if rederived.scalar_arguments != scalar_arguments
        || retained_structural_source_positions.as_deref()
            != Some(rederived.structural_source_parameter_positions.as_slice())
    {
        return unsupported(
            "selected structural Unit operator operands drifted from authored source",
        );
    }
    let mut sources = BTreeSet::new();
    for (argument, target) in structural_arguments
        .iter()
        .zip(&realization.structural_parameters)
    {
        if argument.byte_sequence_literal().is_some()
            || !argument.path.is_empty()
            || argument.access != psi_checked_trees::CheckedStructuralAccess::Owned
            || argument.type_identity != target.type_identity
            || target.is_self
            || target.multiplicity != Multiplicity::Affine
            || target.access != psi_checked_trees::CheckedStructuralAccess::Owned
            || !target.qualifications.is_empty()
            || target.fused_service_erasure.is_some()
            || !sources.insert(argument.source_parameter_index().ok_or(
                LoweringError::Unsupported(
                    "selected operator argument is not a caller structural parameter",
                ),
            )?)
        {
            return unsupported(
                "selected structural Unit operator operands drifted from whole affine roots",
            );
        }
    }
    if sources.len() != machine.structural_parameters.len()
        || sources
            .iter()
            .copied()
            .enumerate()
            .any(|(index, source)| u32::try_from(index).ok() != Some(source))
    {
        return unsupported(
            "selected structural Unit operator operands are not an exact root permutation",
        );
    }
    Ok(())
}
