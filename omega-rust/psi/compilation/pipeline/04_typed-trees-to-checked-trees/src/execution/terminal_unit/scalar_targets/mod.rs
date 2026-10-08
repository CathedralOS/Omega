//! Ordinary scalar calls retain an existing body plan, never a fabricated graph:
//! the callee's scalar graph, its scalar-result state graph, or its ordinary
//! scalar-result body. Availability closes dependencies, while the receiving
//! lowerer independently replays every retained operation and its authored
//! source custody.
use super::{
    CarryPolicy, CheckFacts, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    MachineSupplyMode, PrimitiveType, SignatureContractKind, SymbolHandle,
};
use crate::execution::terminal_unit::ScalarCalleeClosure;
use crate::execution::terminal_unit::types::{ShapeCollector, machine_binders, state_flow};

use crate::execution::terminal_unit::calls::{
    call_claim_transfers, entry_claims, free_structural_scalar_signature,
};

use crate::execution::terminal_unit::structural_scalar_graph_signature;

/// Rejoin owned and borrowed graph signatures before ordinary Unit calls use them.
pub(super) fn registered_structural_graph_target<'facts>(
    program: &dyn ::validation::CheckingInput,
    facts: &'facts CheckFacts,
    scalar_callees: Option<ScalarCalleeClosure>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    result: PrimitiveType,
) -> Option<&'facts checked_trees::CheckedScalarStateGraph> {
    scalar_callees?;
    let mut graphs = facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter()
        .filter(|graph| graph.machine == machine_symbol);
    let graph = graphs.next()?;
    // The call binds the graph's entry signature. Later state rosters belong
    // to their incoming edges and must not constrain the caller's arity.
    let retained = graph.states.first()?;
    if graphs.next().is_some()
        || retained.state != state_symbol
        || retained.result_type != Some(result)
        || retained.structural_parameters.is_empty()
        || !retained.parameter_storage.is_empty()
    {
        return None;
    }
    let machine = program.machine_by_symbol(machine_symbol)?;
    let state = program.machine_states(machine).first()?;
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

/// A registered scalar producer survives ordinary candidate pruning. Only the
/// body fallbacks carry an edge into that changing roster: an ordinary
/// single-state body by its index among the ordinary candidates, and a
/// multi-state state graph by its index among the composed candidates.
pub(super) enum AvailableScalarTarget {
    Registered,
    OrdinaryBody(usize),
    ComposedBody(usize),
}

/// The call-site half of a scalar-call availability check. Ordinary machine
/// plans own their fields directly; composed-control bodies hold the same
/// vocabulary on each retained state, so both route through this view.
pub(super) struct ScalarCallSite<'a> {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub operations: Vec<&'a CheckedUnitEffectOperationPlan>,
    pub structural_parameters: &'a [checked_trees::CheckedUnitStructuralParameterPlan],
    pub entry_claims: &'a [checked_trees::CheckedUnitEntryClaimPlan],
}

impl ScalarCallSite<'_> {
    pub(super) fn of_plan(plan: &CheckedUnitEffectMachinePlan) -> ScalarCallSite<'_> {
        ScalarCallSite {
            machine: plan.machine,
            state: plan.state,
            operations: plan.operations.iter().collect(),
            structural_parameters: &plan.structural_parameters,
            entry_claims: &plan.entry_claims,
        }
    }

    pub(super) fn of_composed_operation<'a>(
        machine: SymbolHandle,
        state: &'a checked_trees::CheckedComposedUnitControlStatePlan,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Option<ScalarCallSite<'a>> {
        let owns = |operations: &[&CheckedUnitEffectOperationPlan]| {
            operations
                .iter()
                .flat_map(|operation| operation.with_value_calls())
                .any(|candidate| std::ptr::eq(candidate, operation))
        };
        let mut operations = state.operations.iter().collect::<Vec<_>>();
        if !owns(&operations) {
            // Selected arms share the predecessor namespace, but their own
            // preparation bindings may reuse ordinals. Only the selected
            // operation's scope can supply its argument sources.
            let mut selected = None;
            for arm in state.terminator.return_arms() {
                let dependencies = arm.operation_dependencies().collect::<Vec<_>>();
                if owns(&dependencies) && selected.replace(dependencies).is_some() {
                    return None;
                }
            }
            if let checked_trees::CheckedComposedUnitControlTerminatorPlan::Guarded {
                return_values,
                ..
            } = &state.terminator
            {
                for returned in return_values {
                    let dependencies = vec![returned];
                    if owns(&dependencies) && selected.replace(dependencies).is_some() {
                        return None;
                    }
                }
            }
            for successor in state.successors() {
                for transfer in &successor.transfers {
                    if let checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                        operands,
                        establishment,
                    } = &transfer.source {
                        let dependencies = operands.iter()
                            .chain(std::iter::once(establishment.as_ref()))
                            .collect::<Vec<_>>();
                        if owns(&dependencies) && selected.replace(dependencies).is_some() {
                            return None;
                        }
                    }
                }
            }
            operations.extend(selected?);
        }
        Some(ScalarCallSite {
            machine,
            state: state.state,
            operations,
            structural_parameters: &state.structural_parameters,
            entry_claims: &state.entry_claims,
        })
    }
}

pub(super) fn available_target(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
    candidates: &[CheckedUnitEffectMachinePlan],
    composed_candidates: &[checked_trees::CheckedComposedUnitControlMachinePlan],
    caller: &ScalarCallSite<'_>,
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<AvailableScalarTarget> {
    // Only an ordinary scalar call resolves a scalar target.
    let planned = operation.call().filter(|call| call.is_module())?;
    let result = planned.scalar_result()?;
    let target_machine = &planned.target.machine;
    let target_state = &planned.target.state;
    let target_contract_report_fingerprint = &planned.target.contract_report_fingerprint;
    let target_contract_commitment = planned.contract_commitment();
    let scalar_arguments = &planned.scalar_arguments;
    let structural_arguments = &planned.structural_arguments;
    let claim_transfers = planned.claim_transfers();
    let coordinate = &planned.coordinate;
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
    let machine = program.machine_by_symbol(*target_machine)?;
    // A scalar-result state graph is entered at its first authored state; every
    // other body owner describes exactly one state.
    let composed = composed_candidates.iter().enumerate().find(|(_, plan)| {
        plan.machine == *target_machine
            && matches!(
                plan.result,
                checked_trees::CheckedControlResultPlan::Scalar { .. }
            )
    });
    let authored_states = program.machine_states(machine);
    let state = if composed.is_some()
        || facts
            .flow
            .terminal_scalar_graphs
            .for_machine(*target_machine)
            .is_some()
    {
        authored_states.first()?
    } else {
        let [state] = authored_states else {
            return None;
        };
        state
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
            Some(scalar_callees),
            *target_machine,
            *target_state,
            result.primitive_type,
        )?;
        (
            &plan.structural_parameters,
            &plan.scalar_parameters,
            &[][..],
            // A registered scalar-graph callee must return a scalar; a
            // structural result is never callable in scalar position.
            plan.result_type?,
        )
    } else if let Some((composed_index, plan)) = composed {
        // A state graph reaches callers through its entry state's signature,
        // exactly as its internal Unit calls do. Rebuild that signature so a
        // drifted plan cannot select this body.
        let checked_trees::CheckedControlResultPlan::Scalar { primitive_type } = plan.result else {
            return None;
        };
        let entry = plan.states.first()?;
        if entry.state != state.symbol
            || plan.contract_report_fingerprint != contract.report_fingerprint
            || plan.contract_commitment != contract.commitment
        {
            return None;
        }
        let mut shapes = ShapeCollector::new(program);
        let signature = free_structural_scalar_signature(program, &mut shapes, state, &[])
            .map(|(structural, scalar)| (None, structural, scalar));
        let (attachment, structural, scalar) = signature?;
        if attachment != plan.attachment_type_identity
            || structural != entry.structural_parameters
            || scalar != entry.scalar_parameters
        {
            return None;
        }
        availability = AvailableScalarTarget::ComposedBody(composed_index);
        (
            &entry.structural_parameters,
            &entry.scalar_parameters,
            entry.entry_claims.as_slice(),
            primitive_type,
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
            || !validation::scalar_result_contract_is_represented(program, state.return_type)
        {
            return None;
        }
        let mut shapes = ShapeCollector::new(program);
        let binders = machine_binders(program, machine);
        let signature = free_structural_scalar_signature(program, &mut shapes, state, &binders)
            .map(|(structural, scalar)| (None, structural, scalar));
        let (attachment, structural, scalar) = signature?;
        if attachment != plan.attachment_type_identity
            || structural != plan.structural_parameters
            || scalar != plan.scalar_parameters
            || plan.scalar_result.as_ref().is_some_and(|completion| {
                plan.operations
                    .iter()
                    .filter(|operation| match operation {
                        // Any call with a scalar result, ordinary or boundary.
                        CheckedUnitEffectOperationPlan::Call(call) => {
                            call.scalar_result() == Some(completion)
                        }
                        CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. } => {
                            result == completion
                        }
                        _ => false,
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
            // Claims and transfers share the dense structural namespace.
            // Authored signature positions also count scalar formals.
            let parameter_index = usize::try_from(claim.parameter_index).unwrap_or(usize::MAX);
            let expected_argument = (parameter_index < structural.len())
                .then_some(parameter_index)
                .and_then(|index| u32::try_from(index).ok());
            Some(transfer.argument_index) != expected_argument || claim.carry != CarryPolicy::STRICT
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
    // Rebuild the same roots statement sequencing gave its call arguments:
    // named locals retain their symbols, while inline constructed or copied
    // operands retain their exact authored expressions, not the destination
    // of the enclosing call-result assignment.
    let caller_state = program.state_by_symbol(caller.state)?;
    let caller_structural_results = caller
        .operations
        .iter()
        .copied()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                value,
                operand_source:
                    Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. }),
                ..
            } => facts
                .values
                .structural_values
                .nodes
                .is_valid(*value)
                .then(|| {
                    (
                        result,
                        Some(facts.values.structural_values.nodes.get(*value).expression),
                    )
                }),
            CheckedUnitEffectOperationPlan::EstablishScalarArray {
                result,
                source:
                    checked_trees::CheckedArrayConstructionSource::CallArgument {
                        call_ordinal,
                        parameter_position,
                    },
                ..
            } => {
                let site = validation::find_call_site(
                    program,
                    caller.machine,
                    caller.state,
                    result.statement_index as usize,
                    *call_ordinal as usize,
                )?;
                let arguments = validation::call_site_argument_expressions(program, &site);
                let target = match site {
                    validation::CallSite::Statement(call) => call.target_symbol,
                    validation::CallSite::Expression { call, .. } => {
                        validation::call_target_symbol(program, call)
                    }
                    validation::CallSite::TransitionNamed { path, .. } => path.symbol,
                };
                let parameters = validation::call_target_parameters(program, target)?;
                let mut actuals = parameters.iter().enumerate().zip(arguments);
                let expression = actuals.find_map(|((position, _), expression)| {
                    (position == *parameter_position as usize).then_some(*expression)
                })?;
                Some((result, Some(expression)))
            }
            CheckedUnitEffectOperationPlan::Call(call) => {
                call.structural_result().map(|result| (result, None))
            }
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. } => {
                Some((result, None))
            }
            _ => None,
        })
        .filter_map(|(result, expression)| {
            if let Some(expression) = expression {
                return Some((result.clone(), facts::PlaceRoot::Expression(expression)));
            }
            let crate::execution::terminal_unit::StatementNode::LocalData(local) = program
                .statement_table
                .statements(caller_state.statement_nodes)
                .get(usize::try_from(result.statement_index).ok()?)?
            else {
                return None;
            };
            Some((result.clone(), facts::PlaceRoot::Symbol(local.symbol)))
        })
        .collect::<Vec<_>>();
    if calls.next().is_some()
        || call_claim_transfers(
            facts,
            caller.machine,
            caller.state,
            call,
            caller.structural_parameters,
            caller.entry_claims,
            &caller_structural_results,
            structural_arguments,
        )
        .as_deref()
            != Some(claim_transfers)
    {
        return None;
    }
    // Authored positions partition into dense scalar and structural namespaces;
    // only erased bindings own no runtime plan entry.
    let planned_positions = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.relevance.is_erased())
        .count();
    (machine.supply_mode == MachineSupplyMode::CheckedBody
        && state.symbol == *target_state
        && planned_positions == scalar.len() + structural.len()
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
        && Some(contract.commitment) == target_contract_commitment)
        .then_some(availability)
}
