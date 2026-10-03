//! Ordinary scalar calls retain an existing body plan, never a fabricated graph.
//! A missing legacy return row may use a complete ordinary scalar-result body.
//! A retained legacy row must match; its drift cannot select that fallback.
//! Availability closes dependencies, while the receiving lowerer independently
//! replays every retained operation and its authored source custody.
use super::{
    CarryPolicy, CheckFacts, CheckedStructuralScalarReturnMachinePlan,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, MachineSupplyMode, PrimitiveType,
    SignatureContractKind, SymbolHandle, TypeReferenceNode, TypedTrees, is_reference, returns,
};
use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::types::{ShapeCollector, machine_binders, state_flow};

use crate::execution::terminal_unit::calls::{
    ambient_self_scalar_graph_signature, call_claim_transfers, entry_claims,
    free_structural_scalar_signature, structural_scalar_signature, structural_signature,
};

use crate::execution::terminal_unit::structural_scalar_graph_signature;

#[cfg(test)]
mod tests;

/// Rejoin owned and borrowed graph signatures before ordinary Unit calls use them.
pub(super) fn registered_structural_graph_target<'facts>(
    program: &TypedTrees,
    facts: &'facts CheckFacts,
    scalar_callees: Option<ScalarCalleePlans<'_>>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    result: PrimitiveType,
) -> Option<&'facts checked_trees::CheckedScalarStateGraph> {
    let scalar_callees = scalar_callees?;
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
        || scalar_callees
            .structural_returns
            .for_machine(machine_symbol)
            .is_some()
        || scalar_callees
            .boundary_returns
            .machines
            .iter()
            .any(|plan| plan.machine == machine_symbol)
    {
        return None;
    }
    let machine = crate::lookup::machine_by_symbol(program, machine_symbol)?;
    let state = program.machine_states(machine).first()?;
    if state.symbol != state_symbol
        || program.primitive_type_reference(state.return_type) != Some(result)
    {
        return None;
    }
    let (structural, scalar, shapes) = if retained
        .structural_parameters
        .iter()
        .any(|parameter| parameter.is_self)
    {
        // A retained borrowed receiver needs the attached signature even when
        // the graph also carries explicit structural arguments. The free
        // signature would drop self and shift every following formal.
        ambient_self_scalar_graph_signature(program, machine, state)?
    } else {
        structural_scalar_graph_signature(program, state)?
    };
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

pub(super) fn registered_primitive_store_target<'plans>(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: Option<ScalarCalleePlans<'plans>>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    result: PrimitiveType,
) -> Option<&'plans CheckedStructuralScalarReturnMachinePlan> {
    let scalar_callees = scalar_callees?;
    let mut targets = scalar_callees
        .structural_returns
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
        || scalar_callees
            .boundary_returns
            .machines
            .iter()
            .any(|target| target.machine == machine_symbol)
    {
        return None;
    }
    let machine = crate::lookup::machine_by_symbol(program, machine_symbol)?;
    let mut shapes = ShapeCollector::new(program);
    let expected = returns::primitive_effects::build_machine(program, facts, &mut shapes, machine)?;
    (expected == *plan).then_some(plan)
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
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    candidates: &[CheckedUnitEffectMachinePlan],
    composed_candidates: &[checked_trees::CheckedComposedUnitControlMachinePlan],
    caller: &ScalarCallSite<'_>,
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<AvailableScalarTarget> {
    // Only an ordinary scalar call resolves a scalar target.
    let planned = operation.call().filter(|call| !call.is_boundary())?;
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
    let machine = crate::lookup::machine_by_symbol(program, *target_machine)?;
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
    } else if scalar_callees
        .structural_returns
        .machines
        .iter()
        .any(|plan| plan.machine == *target_machine)
    {
        let plan = registered_primitive_store_target(
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
            plan.result_type,
        )
    } else if scalar_callees
        .boundary_returns
        .machines
        .iter()
        .any(|plan| plan.machine == *target_machine)
    {
        let mut targets = scalar_callees
            .boundary_returns
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
        // Same receiver pivot the mint applies: attachment storage exists
        // only when an authored `self` parameter retains it.
        let signature = if !program.machine_has_receiver(machine) {
            free_structural_scalar_signature(program, &mut shapes, state, &[])
                .map(|(structural, scalar)| (None, structural, scalar))
        } else {
            // The graph decides per state whether a borrowed `self` keeps its
            // structural place; rebuild with the entry's own decision so a
            // receiver the graph legitimately erased does not falsify the
            // comparison.
            structural_scalar_signature(
                program,
                &mut shapes,
                machine,
                state,
                &[],
                entry
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.is_self),
            )
        };
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
            || (matches!(
                program
                    .type_reference_table
                    .type_reference(state.return_type),
                TypeReferenceNode::Constrained { .. }
            ) && !validation::is_arithmetic_policy_only_integer(program, state.return_type)
                && validation::closed_scalar_result_range(program, state.return_type).is_none())
        {
            return None;
        }
        let mut shapes = ShapeCollector::new(program);
        let binders = machine_binders(program, machine);
        // The plan mints attachment storage only when some state parameter is
        // a receiver; a receiver-free attached machine carries no attachment
        // identity and must compare signatures through the free route.
        let signature = if !program.machine_has_receiver(machine) {
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
        };
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
    // A borrowed receiver the callee leaves ambient on its attachment carrier
    // still occupies its authored slot among the call's structural arguments;
    // it owns no plan entry, so forwarded operands compare around it. The gap
    // is real only when the arguments actually carry one extra slot — a
    // roster that forwards self (a `&mut self` callee whose plan parameter
    // does not flag it) keeps argument and parameter arity equal.
    let ambient_receiver = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.is_self && is_reference(program, parameter.type_reference))
        .filter(|_| !structural.iter().any(|parameter| parameter.is_self))
        .filter(|_| structural_arguments.len() == structural.len() + 1);
    let forwarded = structural_arguments
        .iter()
        .enumerate()
        .filter(|(index, _)| ambient_receiver != Some(*index))
        .map(|(_, argument)| argument)
        .collect::<Vec<_>>();
    if forwarded.len() != structural.len()
        || forwarded
            .iter()
            .zip(structural)
            .any(|(argument, parameter)| {
                argument.type_identity != parameter.type_identity
                    || argument.access != parameter.access
            })
        || claim_transfers.len() != claims.len()
        || claim_transfers.iter().zip(claims).any(|(transfer, claim)| {
            // A claim names a dense structural parameter; its transfer names
            // the dense call-argument slot forwarded to that parameter, one
            // later when the ambient receiver's slot precedes it. Authored
            // signature positions also count scalar formals, so they never
            // index this list.
            let parameter_index = usize::try_from(claim.parameter_index).unwrap_or(usize::MAX);
            let expected_argument = (parameter_index < structural.len())
                .then(|| {
                    parameter_index
                        + usize::from(ambient_receiver.is_some_and(|slot| slot <= parameter_index))
                })
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
    let caller_state = crate::semantic::calls::find_state(program, caller.state)?;
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
                let site = crate::semantic::calls::find_call_site(
                    program,
                    caller.machine,
                    caller.state,
                    result.statement_index as usize,
                    *call_ordinal as usize,
                )?;
                let arguments =
                    crate::semantic::calls::call_site_argument_expressions(program, &site);
                let target = match site {
                    crate::semantic::calls::CallSite::Statement(call) => call.target_symbol,
                    crate::semantic::calls::CallSite::Expression { call, .. } => {
                        crate::flow::call_target_symbol(program, call)
                    }
                    crate::semantic::calls::CallSite::TransitionNamed { path, .. } => path.symbol,
                };
                let parameters = crate::semantic::calls::call_target_parameters(program, target)?;
                let explicit_self = crate::semantic::calls::call_site_supplies_explicit_subject(
                    program, &site, parameters,
                );
                let mut actuals = parameters
                    .iter()
                    .enumerate()
                    .filter(|(_, parameter)| !parameter.is_self || explicit_self)
                    .zip(arguments);
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
    // no receiver or claim may disappear while selecting the real callee body.
    // Two authored positions legitimately own no plan entry here: an erased
    // binding, and a borrowed `self` the callee leaves ambient on the
    // attachment carrier instead of retaining as a structural parameter.
    let receiver_retained = structural.iter().any(|parameter| parameter.is_self);
    let planned_positions = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && !(parameter.is_self
                    && !receiver_retained
                    && is_reference(program, parameter.type_reference))
        })
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
