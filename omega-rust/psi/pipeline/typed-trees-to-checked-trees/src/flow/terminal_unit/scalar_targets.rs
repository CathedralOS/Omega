//! Ordinary scalar calls retain an existing body plan, never a fabricated graph.
//! A missing legacy return row may use a complete ordinary scalar-result body.
//! A retained legacy row must match; its drift cannot select that fallback.
//! Availability closes dependencies, while the receiving lowerer independently
//! replays every retained operation and its authored source custody.

use super::*;

#[cfg(test)]
mod tests;

/// Rejoin owned and borrowed graph signatures before ordinary Unit calls use them.
pub(super) fn registered_structural_graph_target<'facts>(
    program: &TypedTrees,
    facts: &'facts CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    result: PrimitiveType,
) -> Option<&'facts checked_trees::CheckedScalarStateGraph> {
    let mut graphs = facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter()
        .filter(|graph| graph.machine == machine_symbol);
    let graph = graphs.next()?;
    let [retained] = graph.states.as_slice() else {
        return None;
    };
    if graphs.next().is_some()
        || retained.state != state_symbol
        || retained.result_type != result
        || retained.structural_parameters.is_empty()
        || !retained.parameter_storage.is_empty()
        || facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_symbol)
            .is_some()
        || facts
            .flow
            .terminal_boundary_scalar_returns
            .machines
            .iter()
            .any(|plan| plan.machine == machine_symbol)
    {
        return None;
    }
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if state.symbol != state_symbol
        || program.primitive_type_reference(state.return_type) != Some(result)
    {
        return None;
    }
    let (structural, scalar, shapes) = structural_scalar_graph_signature(program, state)?;
    if structural != retained.structural_parameters
        || scalar != retained.scalar_parameters
        || retained.parameter_types
            != scalar
                .iter()
                .map(|parameter| parameter.primitive_type)
                .collect::<Vec<_>>()
        || shapes.iter().any(|shape| {
            let mut matching = facts
                .flow
                .terminal_scalar_graphs
                .structural_types
                .iter()
                .filter(|candidate| candidate.identity == shape.identity);
            matching.next() != Some(shape) || matching.next().is_some()
        })
    {
        return None;
    }
    Some(retained)
}

pub(super) fn registered_primitive_store_target<'facts>(
    program: &TypedTrees,
    facts: &'facts CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    result: PrimitiveType,
) -> Option<&'facts CheckedStructuralScalarReturnMachinePlan> {
    let mut targets = facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .iter()
        .filter(|plan| plan.machine == machine_symbol);
    let plan = targets.next()?;
    if targets.next().is_some()
        || plan.state != state_symbol
        || plan.result_type != result
        || facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine_symbol)
            .is_some()
        || facts
            .flow
            .terminal_boundary_scalar_returns
            .machines
            .iter()
            .any(|target| target.machine == machine_symbol)
    {
        return None;
    }
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let mut shapes = ShapeCollector::new(program);
    let expected = returns::primitive_effects::build_machine(program, facts, &mut shapes, machine)?;
    (expected == *plan).then_some(plan)
}

/// A registered scalar producer survives ordinary candidate pruning. Only the
/// ordinary-body fallback carries an edge into that changing roster.
pub(super) enum AvailableScalarTarget {
    Registered,
    OrdinaryBody(usize),
}

#[cfg(test)]
fn is_available(
    program: &TypedTrees,
    facts: &CheckFacts,
    candidates: &[CheckedUnitEffectMachinePlan],
    caller: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
) -> bool {
    available_target(program, facts, candidates, caller, operation).is_some()
}

pub(super) fn available_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    candidates: &[CheckedUnitEffectMachinePlan],
    caller: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<AvailableScalarTarget> {
    let CheckedUnitEffectOperationPlan::ScalarCall {
        result,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        target_contract_commitment,
        scalar_arguments,
        structural_arguments,
        claim_transfers,
        coordinate,
        ..
    } = operation
    else {
        return None;
    };
    if let Some(graph) = facts
        .flow
        .terminal_scalar_graphs
        .for_machine(*target_machine)
        && graph
            .states
            .first()
            .is_some_and(|state| state.structural_parameters.is_empty())
    {
        return (structural_arguments.is_empty() && claim_transfers.is_empty())
            .then_some(AvailableScalarTarget::Registered);
    }
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == *target_machine)?;
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let contract = facts.contract_plans.for_machine(*target_machine)?;
    let mut availability = AvailableScalarTarget::Registered;
    let (structural, scalar, claims, result_type) = if facts
        .flow
        .terminal_scalar_graphs
        .for_machine(*target_machine)
        .is_some()
    {
        let plan = registered_structural_graph_target(
            program,
            facts,
            *target_machine,
            *target_state,
            result.primitive_type,
        )?;
        (
            &plan.structural_parameters,
            &plan.scalar_parameters,
            &[][..],
            plan.result_type,
        )
    } else if facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .iter()
        .any(|plan| plan.machine == *target_machine)
    {
        let plan = registered_primitive_store_target(
            program,
            facts,
            *target_machine,
            *target_state,
            result.primitive_type,
        )?;
        (
            &plan.structural_parameters,
            &plan.scalar_parameters,
            &[][..],
            plan.result_type,
        )
    } else if facts
        .flow
        .terminal_boundary_scalar_returns
        .machines
        .iter()
        .any(|plan| plan.machine == *target_machine)
    {
        let mut targets = facts
            .flow
            .terminal_boundary_scalar_returns
            .machines
            .iter()
            .filter(|plan| plan.machine == *target_machine);
        let plan = targets.next()?;
        if targets.next().is_some() || plan.state != *target_state {
            return None;
        }
        let mut shapes = ShapeCollector::new(program);
        let binders = machine_binders(program, machine);
        let signature = if plan.scalar_parameters.is_empty() {
            structural_signature(program, &mut shapes, machine, state, &binders, false)
                .map(|(attachment, structural)| (attachment, structural, Vec::new()))
        } else {
            structural_scalar_signature(program, &mut shapes, machine, state, &binders, false)
        };
        let (attachment, structural, scalar) = signature?;
        // Dependency pruning must recognize the scheduled final expression as
        // well as a direct call result. Both retain one exact completion owner;
        // otherwise an ordered helper disappears only when another helper calls it.
        if attachment != plan.attachment_type_identity
            || structural != plan.structural_parameters
            || scalar != plan.scalar_parameters
        {
            return None;
        }
        (
            &plan.structural_parameters,
            &plan.scalar_parameters,
            plan.entry_claims.as_slice(),
            plan.result_type,
        )
    } else {
        // Availability borrows the complete immutable ordinary body roster.
        // The returned ordinal carries its transitive pruning dependency.
        // A scalar completion does not turn its ordered operations into
        // a graph, nor allow a caller to forget their transitive dependencies.
        let mut targets = candidates
            .iter()
            .enumerate()
            .filter(|(_, plan)| plan.machine == *target_machine);
        let (candidate_index, plan) = targets.next()?;
        availability = AvailableScalarTarget::OrdinaryBody(candidate_index);
        let primitive_type = match (&plan.scalar_result, &plan.scalar_control) {
            (Some(completion), None) => completion.primitive_type,
            (None, Some(completion)) => completion.primitive_type,
            _ => return None,
        };
        if plan.scalar_control.as_ref().is_some_and(|completion| {
            super::control::statement_sequence::scalar_control(program, facts, machine, state)
                .as_ref()
                .map(|(expected, _)| expected)
                != Some(completion)
        }) {
            return None;
        }
        if targets.next().is_some()
            || plan.state != *target_state
            || plan.structural_result.is_some()
            || plan.contract_report_fingerprint != contract.report_fingerprint
            || plan.contract_commitment != contract.commitment
            || program
                .machine_contracts(machine)
                .iter()
                .chain(program.state_contracts(state))
                .any(|contract| {
                    !matches!(
                        contract.kind,
                        SignatureContractKind::Crashes { .. }
                            | SignatureContractKind::Requires
                            | SignatureContractKind::Ensures
                    ) || contract.binding.is_some()
                })
            || matches!(
                program
                    .type_reference_table
                    .type_reference(state.return_type),
                TypeReferenceNode::Constrained { .. }
            )
        {
            return None;
        }
        let mut shapes = ShapeCollector::new(program);
        let binders = machine_binders(program, machine);
        let signature = if machine.attached_data.is_none() {
            free_structural_scalar_signature(program, &mut shapes, state, &binders)
                .map(|(structural, scalar)| (None, structural, scalar))
        } else {
            structural_scalar_signature(
                program,
                &mut shapes,
                machine,
                state,
                &binders,
                plan.structural_parameters
                    .iter()
                    .any(|parameter| parameter.is_self),
            )
            .map(|(attachment, structural, scalar)| (Some(attachment), structural, scalar))
        };
        let (attachment, structural, scalar) = signature?;
        if attachment != plan.attachment_type_identity
            || structural != plan.structural_parameters
            || scalar != plan.scalar_parameters
            || plan.scalar_result.as_ref().is_some_and(|completion| {
                plan.operations
                    .iter()
                    .filter(|operation| {
                        matches!(operation,
                            CheckedUnitEffectOperationPlan::ScalarCall { result, .. }
                            | CheckedUnitEffectOperationPlan::BoundaryScalarCall { result, .. }
                            | CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. }
                                if result == completion
                        )
                    })
                    .count()
                    != 1
            })
        {
            return None;
        }
        (
            &plan.structural_parameters,
            &plan.scalar_parameters,
            plan.entry_claims.as_slice(),
            primitive_type,
        )
    };
    if structural_arguments.len() != structural.len()
        || structural_arguments
            .iter()
            .zip(structural)
            .any(|(argument, parameter)| {
                argument.type_identity != parameter.type_identity
                    || argument.access != parameter.access
            })
        || claim_transfers.len() != claims.len()
        || claim_transfers.iter().zip(claims).any(|(transfer, claim)| {
            transfer.argument_index != claim.parameter_index || claim.carry != CarryPolicy::STRICT
        })
    {
        return None;
    }
    if entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        structural,
        program.state_parameters(state),
    )
    .as_deref()
        != Some(claims)
    {
        return None;
    }
    let flow = state_flow(facts, caller.machine, caller.state)?;
    let mut calls = facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .iter()
        .filter(|call| {
            u32::try_from(call.statement_index).ok() == Some(coordinate.statement_index)
                && u32::try_from(call.call_ordinal).ok() == Some(coordinate.call_ordinal)
        });
    let call = calls.next()?;
    if calls.next().is_some()
        || call_claim_transfers(
            facts,
            caller.machine,
            caller.state,
            call,
            &caller.structural_parameters,
            &caller.entry_claims,
            structural_arguments,
            PermissionEventKind::Transfer,
        )
        .as_ref()
            != Some(claim_transfers)
    {
        return None;
    }
    // Authored positions partition into dense scalar and structural namespaces;
    // no receiver or claim may disappear while selecting the real callee body.
    (machine.supply_mode == MachineSupplyMode::CheckedBody
        && state.symbol == *target_state
        && program.state_parameters(state).len() == scalar.len() + structural.len()
        && scalar_arguments.len() == scalar.len()
        && scalar_arguments
            .iter()
            .zip(scalar)
            .all(|(argument, parameter)| {
                let primitive_type = match argument {
                    checked_trees::CheckedCallScalarArgument::Pure(expression) => {
                        crate::values::scalar_expression_type(expression)
                    }
                    checked_trees::CheckedCallScalarArgument::Computation(root) => {
                        let computations = &facts.values.scalar_computations;
                        computations
                            .nodes
                            .is_valid(*root)
                            .then(|| computations.nodes.get(*root).primitive_type)
                    }
                };
                primitive_type == Some(parameter.primitive_type)
            })
        && result_type == result.primitive_type
        && program.primitive_type_reference(state.return_type) == Some(result_type)
        && contract.report_fingerprint == *target_contract_report_fingerprint
        && contract.commitment == *target_contract_commitment)
        .then_some(availability)
}
