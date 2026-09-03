//! Recheck exact selected boundary-operator custody before Unit lowering.

use super::*;

mod structural_realizations;
mod structural_result_realizations;
pub(super) use structural_realizations::lower_selected_structural_scalar_realizations;
pub(super) use structural_result_realizations::lower_selected_structural_result_realizations;

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
) -> Result<(), LoweringError> {
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
        .count()
        + checked
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
            .count();
    if coordinate.call_ordinal != 0
        || provider_plan_report_fingerprint == 0
        || provider_plan_commitment.is_empty()
        || exact_uses != 1
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
    Ok(())
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
    validate_selected_operator_scalar_call(
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

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_selected_operator_structural_call(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
    coordinate: psi_checked_trees::CheckedUnitCallCoordinate,
    result: &psi_checked_trees::CheckedUnitStructuralResultBindingPlan,
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
    discard_result_on_return: bool,
) -> Result<(), LoweringError> {
    let origin = psi_checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol: machine.machine,
        state_symbol: machine.state,
        statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
            LoweringError::Unsupported(
                "selected structural-result statement coordinate exceeds usize",
            )
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
        .count()
        + checked
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
            .count();
    let typed_realization = checked
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
            "selected structural-result realization lost its exact checked entry",
        ))?;
    let contract = checked
        .facts
        .contract_plans
        .for_machine(realization_machine)
        .ok_or(LoweringError::Unsupported(
            "selected structural-result realization has no checked contract",
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
    let realizations = checked
        .facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_machines
        .iter()
        .filter(|plan| plan.machine == realization_machine && plan.state == realization_state)
        .collect::<Vec<_>>();
    let [realization] = realizations.as_slice() else {
        return unsupported(
            "selected structural-result Unit operator does not rejoin one checked realization",
        );
    };
    if coordinate.call_ordinal != 0
        || coordinate.statement_index != result.statement_index
        || result.binding_ordinal != 0
        || result.multiplicity != Multiplicity::Affine
        || !discard_result_on_return
        || provider_plan_report_fingerprint == 0
        || provider_plan_commitment.is_empty()
        || exact_uses != 1
        || contract.report_fingerprint != realization_contract_report_fingerprint
        || contract.commitment != realization_contract_commitment
        || realization_contract_rows != 1
        || checked.typed.state_parameters(typed_realization).len()
            != scalar_arguments.len() + structural_arguments.len()
        || realization_reaches.as_slice() != [service_reach]
        || realization.result.type_identity != result.type_identity
        || realization.result.multiplicity != Multiplicity::Affine
        || !realization.result.qualifications.is_empty()
        || realization.scalar_parameters.len() != scalar_arguments.len()
        || realization.scalar_parameters.len() != 1
        || realization.structural_parameter.position != 0
        || realization.scalar_parameters[0].source_position != 1
        || structural_arguments.len() != 1
        || !machine.entry_claims.is_empty()
        || machine.structural_parameters.len() != 1
    {
        return unsupported(
            "selected structural-result Unit operator drifted from its exact checked application",
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
            "selected structural-result realization with services requires a separate Terminal lane",
        );
    }
    for (argument, target) in scalar_arguments.iter().zip(&realization.scalar_parameters) {
        let scalar_type = lower_checked_scalar_expression(argument)?.scalar_type();
        if scalar_type != terminal_scalar_type(target.primitive_type)?
            || !matches!(scalar_type, ScalarType::Integer(integer)
                if integer.carrier() == psi_core::IntegerCarrier::Fixed
                    && matches!(integer.bits(), 8 | 16 | 32 | 64))
        {
            return unsupported(
                "selected structural-result scalar operand drifted from its fixed-width realization",
            );
        }
    }
    let [argument] = structural_arguments else {
        unreachable!("one structural argument was required above")
    };
    let source = argument
        .source_parameter_index()
        .ok_or(LoweringError::Unsupported(
            "selected structural-result argument is not a caller structural parameter",
        ))?;
    if source != 0
        || argument.byte_sequence_literal().is_some()
        || !argument.path.is_empty()
        || argument.access != psi_checked_trees::CheckedStructuralAccess::Owned
        || argument.type_identity != realization.structural_parameter.type_identity
        || argument.type_identity != result.type_identity
        || realization.structural_parameter.is_self
        || realization.structural_parameter.multiplicity != Multiplicity::Affine
        || realization.structural_parameter.access
            != psi_checked_trees::CheckedStructuralAccess::Owned
        || !realization.structural_parameter.qualifications.is_empty()
        || realization
            .structural_parameter
            .fused_service_erasure
            .is_some()
        || machine.structural_parameters[0].multiplicity != Multiplicity::Affine
        || machine.structural_parameters[0].access
            != psi_checked_trees::CheckedStructuralAccess::Owned
        || !machine.structural_parameters[0].qualifications.is_empty()
    {
        return unsupported(
            "selected structural-result operand drifted from one whole claim-free affine root",
        );
    }
    let result_shape = checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .iter()
        .find(|declaration| declaration.identity == result.type_identity)
        .ok_or(LoweringError::Unsupported(
            "selected structural-result type is absent from the Unit catalog",
        ))?;
    if !matches!(
        &result_shape.shape,
        psi_checked_trees::CheckedUnitStructuralTypeShape::Record { fields }
            if matches!(
                fields.as_slice(),
                [field]
                    if matches!(
                        &field.field_type,
                        psi_checked_trees::CheckedUnitStructuralFieldType::Scalar(
                            PrimitiveType::I64 | PrimitiveType::U64
                        )
                    )
            )
    ) {
        return unsupported(
            "selected structural-result type exceeds the direct 8-byte record lane",
        );
    }
    Ok(())
}
