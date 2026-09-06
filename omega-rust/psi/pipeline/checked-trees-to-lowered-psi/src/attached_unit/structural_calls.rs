//! Ordinary whole-affine result calls within the shared Unit closure.

use super::*;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;

fn target(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
) -> Result<&checked_trees::CheckedClaimFreeAffineStructuralReturnMachinePlan, LoweringError> {
    let mut targets = checked
        .facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_machines
        .iter()
        .filter(|target| target.machine == machine);
    let target = targets.next().ok_or(LoweringError::Unsupported(
        "ordinary structural call has no checked affine identity target",
    ))?;
    if targets.next().is_some() || target.state != state {
        return unsupported("ordinary structural call target state is absent or ambiguous");
    }
    Ok(target)
}

pub(super) fn validate(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::StructuralCall {
        coordinate,
        result,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        target_contract_commitment,
        service_reach,
        scalar_arguments,
        structural_arguments,
        discard_result_on_return,
        ..
    } = operation
    else {
        return unsupported("ordinary structural validation requires a structural call");
    };
    retain_exact_checked_flow_call(checked, caller, *coordinate, *target_state)?;
    let target = target(checked, *target_machine, *target_state)?;
    let contract = checked
        .facts
        .contract_plans
        .for_machine(*target_machine)
        .ok_or(LoweringError::Unsupported(
            "ordinary structural call has no checked target contract",
        ))?;
    let reaches = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| {
            state.machine_symbol == *target_machine && state.state_symbol == *target_state
        })
        .map(|(_, state)| state.service_reach)
        .collect::<Vec<_>>();
    if target_contract_commitment.is_zero()
        || *target_contract_report_fingerprint == 0
        || contract.report_fingerprint != *target_contract_report_fingerprint
        || contract.commitment != *target_contract_commitment
        || reaches.as_slice() != [*service_reach]
        || !checked
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
        || coordinate.call_ordinal != 0
        || result.statement_index != coordinate.statement_index
        || result.binding_ordinal != 0
        || !discard_result_on_return
        || result.multiplicity != Multiplicity::Affine
        || result.type_identity != target.result.type_identity
        || target.result.multiplicity != Multiplicity::Affine
        || !target.result.qualifications.is_empty()
        || target.scalar_parameters.len() != scalar_arguments.len()
        || target.structural_parameter.type_identity != result.type_identity
        || target.structural_parameter.multiplicity != Multiplicity::Affine
        || target.structural_parameter.access != checked_trees::CheckedStructuralAccess::Owned
        || target.structural_parameter.is_self
        || !target.structural_parameter.qualifications.is_empty()
        || target.structural_parameter.fused_service_erasure.is_some()
    {
        return unsupported(
            "ordinary structural call disagrees with its checked result, contract, or reach",
        );
    }
    let [argument] = structural_arguments.as_slice() else {
        return unsupported("ordinary structural call requires one whole owned argument");
    };
    let source_index = argument
        .source_parameter_index()
        .ok_or(LoweringError::Unsupported(
            "ordinary structural call source must be a whole entry parameter",
        ))?;
    let source = caller
        .structural_parameters
        .get(source_index as usize)
        .ok_or(LoweringError::Unsupported(
            "ordinary structural call source parameter is absent",
        ))?;
    if !argument.path.is_empty()
        || argument.access != checked_trees::CheckedStructuralAccess::Owned
        || argument.type_identity != result.type_identity
        || source.type_identity != argument.type_identity
        || source.access != checked_trees::CheckedStructuralAccess::Owned
        || source.multiplicity != Multiplicity::Affine
        || source.is_self
        || !source.qualifications.is_empty()
        || source.fused_service_erasure.is_some()
        || caller
            .entry_claims
            .iter()
            .any(|claim| claim.parameter_index == source_index)
    {
        return unsupported(
            "ordinary structural call source is not an exact claim-free affine parameter",
        );
    }
    let (source_machine, source_state) =
        crate::scalar_source_custody::authored_state(checked, caller.state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(source_state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return unsupported("ordinary structural result has no authored immutable local");
    };
    // This outer result has its own checked target and custody slice. Retain
    // its captured occurrence without widening nested result-call admission.
    crate::call_source_custody::occurrences::validate(
        checked,
        caller.machine,
        caller.state,
        *coordinate,
        local.initial_value,
    )?;
    let ExpressionNode::Call(call) = checked.expression_table.expression(local.initial_value)
    else {
        return unsupported("ordinary structural result has no direct initializer call");
    };
    let source_parameter = checked
        .state_parameters(source_state)
        .get(source.position as usize)
        .ok_or(LoweringError::Unsupported(
            "ordinary structural source lost its authored position",
        ))?;
    let authored_argument = checked
        .expression_table
        .expression_handles(call.arguments)
        .get(target.structural_parameter.position as usize)
        .ok_or(LoweringError::Unsupported(
            "ordinary structural call lost its authored argument",
        ))?;
    if source_machine.symbol != caller.machine
        || local.is_mutable
        || checked
            .typed
            .normalized_type_identity(local.type_reference)
            .into_string()
            != result.type_identity
        || !matches!(checked.expression_table.expression(*authored_argument),
            ExpressionNode::Name(name) if name.symbol == source_parameter.symbol)
    {
        return unsupported(
            "ordinary structural result or source disagrees with its authored initializer",
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
    parameters: &[StructuralParameterDeclaration],
    evaluated: Option<&[ValueDeclaration]>,
    structural_types: &[StructuralTypeDeclaration],
    type_ids: &[(String, StructuralTypeId)],
    machine_ids: &[(symbols::SymbolHandle, MachineId)],
    result_count: usize,
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(StructuralPlaceDeclaration, bool), LoweringError> {
    let CheckedUnitEffectOperationPlan::StructuralCall {
        coordinate,
        source_site,
        result,
        target_machine,
        target_state,
        structural_arguments,
        discard_result_on_return,
        ..
    } = operation
    else {
        return unsupported("ordinary structural emission requires a structural call");
    };
    if usize::try_from(result.binding_ordinal).ok() != Some(result_count) {
        return unsupported("ordinary structural result binding ordinal drifted from source order");
    }
    let target = target(checked, *target_machine, *target_state)?;
    validate_transfer_shape(
        structural_arguments,
        &[],
        parameters,
        &[],
        &[],
        std::slice::from_ref(&target.structural_parameter),
        type_ids,
        structural_types,
        &[],
    )?;
    let scalar_types = target
        .scalar_parameters
        .iter()
        .map(|parameter| terminal_scalar_type(parameter.primitive_type))
        .collect::<Result<Vec<_>, _>>()?;
    if scalar_types.iter().any(|scalar_type| !matches!(scalar_type,
        ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
            && matches!(integer.bits(), 8 | 16 | 32 | 64))) {
        return unsupported("ordinary structural side arguments require fixed-width integers");
    }
    let arguments = argument_evaluation::validated_values(evaluated, &scalar_types)?
        .iter()
        .map(|value| value.id)
        .collect();
    let structural_arguments =
        lower_structural_arguments(structural_arguments, parameters, &[], &[], &[])?;
    let id = operations.allocate();
    let place = place_id(allocate_dense(next_place)?);
    let structural_type = lookup_type_id(type_ids, &result.type_identity)?;
    operations.record_source_call(
        SourceCallCoordinate {
            state: caller.state,
            statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                LoweringError::Unsupported("ordinary structural statement coordinate exceeds usize")
            })?,
            call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("ordinary structural call ordinal exceeds usize")
            })?,
        },
        *source_site,
        id,
        *target_machine,
    )?;
    operations.push(Operation {
        id,
        result: OperationResult::Structural(StructuralOperationResult {
            place,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::CallStructuralWithScalarArguments {
            callee: lookup_machine_id(machine_ids, *target_machine)?,
            arguments,
            structural_arguments,
            claim_transfers: Vec::new(),
            returned_claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    Ok((
        StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: id,
                structural_type,
            },
        },
        *discard_result_on_return,
    ))
}
