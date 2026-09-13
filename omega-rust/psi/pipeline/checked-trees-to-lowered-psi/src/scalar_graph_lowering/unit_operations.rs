//! Rejoin ordered Unit statements and lower their existing checked call rows.

use super::*;
use crate::attached_unit::bodies::UnitBody;
use checked_trees::{CheckedUnitCallCoordinate, CheckedUnitEffectOperationPlan};

pub(super) struct Prepared {
    pub(super) coordinate: CheckedUnitCallCoordinate,
    pub(super) arguments: Vec<checked_trees::CheckedCallScalarArgument>,
    pub(super) argument_types: Vec<QualifiedScalarType>,
    pub(super) call: LoweredUnitCall,
}

pub(super) fn statement_index(
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<u32, LoweringError> {
    match operation {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => {
            Ok(result.statement_index)
        }
        _ => Ok(coordinate(operation)?.statement_index),
    }
}

pub(super) fn coordinate(
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<CheckedUnitCallCoordinate, LoweringError> {
    match operation {
        CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. } => Ok(*coordinate),
        _ => unsupported("scalar graph retained an unsupported Unit operation"),
    }
}

pub(super) fn prepare(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedScalarStateGraph,
    operation: &CheckedUnitEffectOperationPlan,
    bindings: &storage::ScalarBindings,
    value_types: &[QualifiedScalarType],
) -> Result<Prepared, LoweringError> {
    let CheckedUnitEffectOperationPlan::CallUnit {
        coordinate,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        scalar_arguments,
        structural_arguments,
        claim_transfers,
    } = operation
    else {
        return unsupported("scalar graph requires an ordinary Unit operation");
    };
    let body = UnitBody::find(&checked.facts.flow.terminal_unit_effects, *target_machine)?;
    let target = body.entry()?;
    if body.result()? != checked_trees::CheckedControlResultPlan::Unit
        || target.state != *target_state
        || target.contract_report_fingerprint != *target_contract_report_fingerprint
        || !crate::attached_unit::catalog::checked_unit_target_reach_matches(
            *service_reach,
            target.contract_service_reach,
        )
        || !target.entry_claims.is_empty()
        || !claim_transfers.is_empty()
        || structural_arguments.len() != target.structural_parameters.len()
        || scalar_arguments.len() != target.scalar_parameters.len()
    {
        return unsupported("scalar Unit call disagrees with its exact target custody");
    }
    let flow_call = crate::attached_unit::retain_exact_flow_call(
        checked,
        machine,
        state.state,
        *coordinate,
        *target_state,
    )?;
    crate::call_source_custody::validate_operation(
        checked,
        machine,
        state.state,
        operation,
        &state.structural_parameters,
    )?;
    let authored =
        crate::call_source_custody::authored::locate_source(checked, state.state, *coordinate)?;
    let (_, caller_state) = source_custody::authored_state(checked, state.state)?;
    let (_, callee_state) = source_custody::authored_state(checked, *target_state)?;
    let Some(checked_trees::statement::StatementNode::Call(source_call)) = checked
        .statement_table
        .statements(caller_state.statement_nodes)
        .get(coordinate.statement_index as usize)
    else {
        return unsupported("scalar Unit operation lost its authored statement call");
    };
    if flow_call.has_receiver
        || flow_call.receiver_symbol != source_call.receiver_symbol
        || flow_call.authored_expression.is_valid()
        || flow_call.service_reach != *service_reach
    {
        return unsupported("scalar Unit operation substituted its captured source call metadata");
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
        return unsupported("scalar Unit calls with services require scalar service lowering");
    }
    let borrow = crate::attached_unit::primitive_locals::borrows::call(
        checked,
        machine,
        state.state,
        *coordinate,
        authored.source_target,
    )?;
    if borrow.has_receiver != flow_call.has_receiver
        || borrow.receiver_symbol != flow_call.receiver_symbol
        || borrow.accesses != flow_call.accesses
    {
        return unsupported("scalar Unit operation substituted its captured borrow call rows");
    }
    let authored_arguments = checked
        .statement_table
        .expression_handles(source_call.arguments);
    let access_positions = crate::scalar_source_custody::computation_calls::rejoin_call_accesses(
        checked,
        borrow,
        authored_arguments,
    )?;
    let contract = checked
        .facts
        .contract_plans
        .for_machine(*target_machine)
        .ok_or(LoweringError::Unsupported(
            "scalar Unit target has no checked contract",
        ))?;
    // General structural contract substitution belongs to the existing Unit
    // assembler. This graph lane admits only the already plain primitive call
    // contract, and does not silently erase authored requirements or exits.
    if !checked.state_contracts(callee_state).is_empty()
        || !contract.closed_scalar_values.requires().is_empty()
        || !contract.closed_scalar_values.ensures().is_empty()
        || !contract.crash.published().is_empty()
        || contract
            .crash
            .structural_runtime_requirements()
            .is_some_and(|requirements| !requirements.is_empty())
        || checked
            .state_parameters(callee_state)
            .iter()
            .any(|parameter| {
                let reference = checked
                    .type_reference_table
                    .type_reference(parameter.type_reference);
                if checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
                {
                    !matches!(
                        reference,
                        checked_trees::types::TypeReferenceNode::Named { .. }
                    )
                } else {
                    false
                }
            })
    {
        return unsupported("scalar Unit calls require an empty authored value and crash contract");
    }
    let mut lowered_arguments = Vec::new();
    for (argument, parameter) in structural_arguments
        .iter()
        .zip(target.structural_parameters)
    {
        if parameter.multiplicity != Multiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || parameter.fused_service_erasure.is_some()
            || !argument.path.is_empty()
            || parameter.access != argument.access
            || parameter.type_identity != argument.type_identity
        {
            return unsupported("scalar Unit argument does not retain a plain primitive borrow");
        }
        let source_parameter = checked
            .state_parameters(callee_state)
            .get(parameter.position as usize)
            .ok_or(LoweringError::Unsupported(
                "scalar Unit formal position is absent",
            ))?;
        let expression = authored
            .structural_arguments
            .iter()
            .find_map(|(position, expression)| {
                (*position == parameter.position).then_some(*expression)
            })
            .ok_or(LoweringError::Unsupported(
                "scalar Unit call lost its authored structural actual",
            ))?;
        let access_position = *access_positions.get(parameter.position as usize).ok_or(
            LoweringError::Unsupported("scalar Unit argument has no authored access position"),
        )?;
        let actual_position =
            crate::scalar_source_custody::computation_calls::primitive_arguments::validate(
                checked,
                caller_state,
                coordinate.statement_index,
                source_parameter.type_reference,
                expression,
                argument,
                borrow,
                Some(access_position),
            )?;
        if actual_position != access_position {
            return unsupported("scalar Unit call reordered its structural access occurrences");
        }
        let checked_trees::types::TypeReferenceNode::Reference { referee, .. } = checked
            .type_reference_table
            .type_reference(source_parameter.type_reference)
        else {
            return unsupported("scalar Unit structural argument is not a primitive borrow");
        };
        let primitive =
            checked
                .primitive_type_reference(*referee)
                .ok_or(LoweringError::Unsupported(
                    "scalar Unit borrow is not primitive",
                ))?;
        lowered_arguments
            .push(bindings.primitive_borrow(argument, terminal_scalar_type(primitive)?)?);
    }
    let argument_types = target
        .scalar_parameters
        .iter()
        .map(|parameter| terminal_scalar_type(parameter.primitive_type))
        .collect::<Result<Vec<_>, _>>()?;
    for ((_, actual), expected) in authored.scalar_arguments.iter().zip(&argument_types) {
        if terminal_scalar_type(*actual)? != *expected {
            return unsupported("scalar Unit argument substituted its formal carrier");
        }
    }
    Ok(Prepared {
        coordinate: *coordinate,
        arguments: scalar_arguments.clone(),
        call: LoweredUnitCall {
            source_coordinate: SourceCallCoordinate {
                state: state.state,
                statement_index: coordinate.statement_index as usize,
                call_ordinal: coordinate.call_ordinal as usize,
            },
            target_machine: *target_machine,
            target_state: *target_state,
            arguments: argument_types
                .iter()
                .enumerate()
                .map(
                    |(ordinal, scalar_type)| LoweredDirectExpression::Parameter {
                        position: value_types.len() + ordinal,
                        scalar_type: *scalar_type,
                    },
                )
                .collect(),
            structural_arguments: lowered_arguments,
            crash_routes: contract.crash.published().to_vec(),
        },
        argument_types: argument_types.into_iter().map(Into::into).collect(),
    })
}
