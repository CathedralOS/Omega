//! Exact admission for one source-visible routed-Service forwarding hop.

use super::*;

/// This check is deliberately independent from the ordinary empty-path fast
/// path: routed authority must retain one exact carrier, requirement, and
/// selection across the edge, and the target must terminate the route in
/// direct boundary calls rather than forming an unbounded forwarding graph.
#[allow(clippy::too_many_arguments)]
pub(super) fn exact_single_fused_service_forward_is_supported(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    target_machine: &typed_trees::machine::Machine,
    target_state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    arguments: &[CheckedUnitStructuralArgumentPlan],
) -> bool {
    if caller_machine.attached_data.is_some()
        || target_machine.attached_data.is_some()
        || !caller_machine.lifetime_parameters.is_empty()
        || !target_machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(caller_machine).is_empty()
        || !program.machine_type_parameters(target_machine).is_empty()
        || !matches!(program.machine_states(caller_machine), [only] if only.symbol == caller_state.symbol)
        || !matches!(program.machine_states(target_machine), [only] if only.symbol == target_state.symbol)
    {
        return false;
    }
    let [caller_source] = program.state_parameters(caller_state) else {
        return false;
    };
    let [target_source] = program.state_parameters(target_state) else {
        return false;
    };
    let [caller_parameter] = caller_parameters else {
        return false;
    };
    let [argument] = arguments else {
        return false;
    };
    let Some(caller_receipt) = caller_parameter.fused_service_erasure.as_ref() else {
        return false;
    };
    let Ok(Some(caller_carrier)) = typed_trees::service::classify_exact_bound_service_carrier(
        program,
        caller_source.type_reference,
    ) else {
        return false;
    };
    let Ok(Some(target_carrier)) = typed_trees::service::classify_exact_bound_service_carrier(
        program,
        target_source.type_reference,
    ) else {
        return false;
    };
    let Some(target_authorization) = program.fused_service_erasure(target_carrier.requirement)
    else {
        return false;
    };
    if caller_source.is_self
        || caller_source.is_const
        || caller_source.is_mutable
        || target_source.is_self
        || target_source.is_const
        || target_source.is_mutable
        || caller_parameter.position != 0
        || caller_parameter.is_self
        || caller_parameter.multiplicity != Multiplicity::Affine
        || caller_parameter.access != CheckedStructuralAccess::Owned
        || caller_parameter.qualifications.len() != 1
        || caller_receipt.source_parameter != caller_source.symbol
        || caller_receipt.requirement != caller_carrier.requirement
        || caller_receipt.requirement != target_carrier.requirement
        || caller_receipt.provider_plan_digest != target_authorization.provider_plan_digest
        || caller_receipt.carrier_type_identity
            != program
                .normalized_type_identity(caller_source.type_reference)
                .into_string()
        || caller_receipt.carrier_type_identity
            != program
                .normalized_type_identity(target_source.type_reference)
                .into_string()
        || crate::checks::type_multiplicity(program, target_source.type_reference)
            != Multiplicity::Affine
        || structural_access_for_type_reference(program, target_source.type_reference)
            != Some(CheckedStructuralAccess::Owned)
        || argument.source_parameter_index() != Some(0)
        || !argument.path.is_empty()
        || argument.type_identity != caller_parameter.type_identity
        || argument.access != CheckedStructuralAccess::Owned
        || argument.byte_sequence_literal().is_some()
    {
        return false;
    }

    let Some(caller_flow) = state_flow(facts, caller_machine.symbol, caller_state.symbol) else {
        return false;
    };
    let caller_calls = facts.flow.control.calls.span_or_empty(caller_flow.calls);
    let caller_statements = program
        .statement_table
        .statements(caller_state.statement_nodes);
    if !matches!(caller_calls, [source_call]
        if source_call.statement_index == call.statement_index
            && source_call.call_ordinal == call.call_ordinal
            && source_call.target_symbol == target_state.symbol
            && !source_call.has_receiver)
        || !matches!(caller_statements, [StatementNode::Call(_)])
    {
        return false;
    }

    let Some(requirement) = program.traits().iter().find(|definition| {
        definition.is_boundary && definition.symbol == target_carrier.requirement
    }) else {
        return false;
    };
    let requirement_states = program.trait_machine_signatures(requirement);
    let Some(target_flow) = state_flow(facts, target_machine.symbol, target_state.symbol) else {
        return false;
    };
    let target_calls = facts.flow.control.calls.span_or_empty(target_flow.calls);
    let target_statements = program
        .statement_table
        .statements(target_state.statement_nodes);
    !target_calls.is_empty()
        && target_calls.len() == target_statements.len()
        && target_statements
            .iter()
            .all(|statement| matches!(statement, StatementNode::Call(_)))
        && target_calls.iter().enumerate().all(|(index, direct)| {
            direct.statement_index == index
                && direct.call_ordinal == 0
                && direct.has_receiver
                && direct.receiver_symbol == target_source.symbol
                && requirement_states.iter().any(|signature| {
                    signature.symbol == direct.target_symbol
                        && is_unit(program, signature.return_type)
                        && program
                            .state_signature_parameters(signature)
                            .iter()
                            .all(|parameter| parameter.is_self)
                })
        })
}
