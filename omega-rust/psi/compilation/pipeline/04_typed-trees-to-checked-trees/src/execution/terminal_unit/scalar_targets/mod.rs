//! Ordinary scalar calls retain an existing body plan, never a fabricated one:
//! the callee's scalar-result state graph or its ordinary scalar-result body.
//! Availability closes dependencies, while the receiving lowerer independently
//! replays every retained operation and its authored source custody.
use super::{
    CarryPolicy, CheckFacts, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    MachineSupplyMode, SignatureContractKind, SymbolHandle,
};
use crate::execution::terminal_unit::types::{ShapeCollector, machine_binders, state_flow};

use crate::execution::terminal_unit::calls::{
    call_claim_transfers, entry_claims, free_structural_scalar_signature,
};

/// The body a scalar call depends on, as an edge into the changing candidate
/// roster: an ordinary single-state body by its index among the ordinary
/// candidates, and a multi-state state graph by its index among the composed
/// candidates. A caller survives pruning only while its callee's body does.
pub(super) enum AvailableScalarTarget {
    Ordinary(usize),
    Composed(usize),
    /// An ordinary single-state body published through the partial-affine
    /// carrier, by its index among those candidates.
    PartialAffine(usize),
}

/// The call-site half of a scalar-call availability check. Ordinary machine
/// plans own their fields directly; composed-control bodies hold the same
/// vocabulary on each retained state, so both route through this view.
pub(super) struct ScalarCallSite<'a> {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub operations: Vec<&'a CheckedUnitEffectOperationPlan>,
    pub structural_parameters: &'a [checked_trees::CheckedUnitStructuralParameterPlan],
}

impl ScalarCallSite<'_> {
    pub(super) fn of_plan(plan: &CheckedUnitEffectMachinePlan) -> ScalarCallSite<'_> {
        ScalarCallSite {
            machine: plan.machine,
            state: plan.state,
            operations: plan.operations.iter().collect(),
            structural_parameters: &plan.structural_parameters,
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
                        ..
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
        })
    }
}

pub(super) fn available_target(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    candidates: &[CheckedUnitEffectMachinePlan],
    composed_candidates: &[checked_trees::CheckedComposedUnitControlMachinePlan],
    partial_affine_candidates: &[checked_trees::CheckedPartialAffineUnitCleanupMachinePlan],
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
    let state = if composed.is_some() {
        authored_states.first()?
    } else {
        let [state] = authored_states else {
            return None;
        };
        state
    };
    let contract = facts.contract_plans.for_machine(*target_machine)?;
    let availability;
    let (structural, scalar, claims, result_type) = if let Some((composed_index, plan)) = composed {
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
        availability = AvailableScalarTarget::Composed(composed_index);
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
            .map(|(index, plan)| (AvailableScalarTarget::Ordinary(index), plan))
            .chain(
                partial_affine_candidates
                    .iter()
                    .enumerate()
                    .map(|(index, plan)| {
                        (AvailableScalarTarget::PartialAffine(index), &plan.machine)
                    }),
            )
            .filter(|(_, plan)| plan.machine == *target_machine);
        let (candidate, plan) = targets.next()?;
        availability = candidate;
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
                        CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::Scalar { result, .. },
                        ) => result == completion,
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
        || !claim_transfers_match_entry_catalogue(claim_transfers, claims, structural.len())
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
            CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                    result,
                    value,
                    operand_source:
                        Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. }),
                    ..
                },
            ) => facts
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
            CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::ScalarArray {
                    result,
                    source:
                        checked_trees::CheckedArrayConstructionSource::CallArgument {
                            call_ordinal,
                            parameter_position,
                        },
                    ..
                },
            ) => {
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
            // A nested operand's anonymous result is rooted at its authored
            // call; a bound result at its local.
            CheckedUnitEffectOperationPlan::Call(call) => {
                let result = call.structural_result()?;
                if result.local.symbol.is_valid() {
                    return Some((result, None));
                }
                let mut producers = facts
                    .flow
                    .control
                    .calls
                    .span_or_empty(flow.calls)
                    .iter()
                    .filter(|producer| {
                        u32::try_from(producer.statement_index).ok()
                            == Some(call.coordinate.statement_index)
                            && u32::try_from(producer.call_ordinal).ok()
                                == Some(call.coordinate.call_ordinal)
                    });
                let producer = producers.next()?;
                if producers.next().is_some() || !producer.authored_expression.is_valid() {
                    return None;
                }
                Some((result, Some(producer.authored_expression)))
            }
            CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { result, .. },
            )
            | CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { result, .. },
            ) => Some((result, None)),
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
            program,
            facts,
            caller.machine,
            caller.state,
            call,
            caller.structural_parameters,
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

/// Entry claims describe every possible payload; a known sum transfers only
/// its active payload. Match typed relative paths, not catalogue cardinality or
/// claim ordering. Unguarded entries always require a transfer. Source flow's
/// exact transfer roster is reconstructed above; consuming custody binds every
/// active callee claim against the argument's actual tag. Missing an inactive
/// entry is not permission to invent or omit a live claim.
fn claim_transfers_match_entry_catalogue(
    transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
    claims: &[checked_trees::CheckedUnitEntryClaimPlan],
    structural_parameter_count: usize,
) -> bool {
    let matches = |transfer: &checked_trees::CheckedUnitClaimTransferPlan,
                   claim: &checked_trees::CheckedUnitEntryClaimPlan| {
        transfer.argument_index == claim.parameter_index && transfer.path == claim.path
    };
    claims.iter().all(|claim| {
        usize::try_from(claim.parameter_index)
            .is_ok_and(|parameter_index| parameter_index < structural_parameter_count)
            && claim.carry == CarryPolicy::STRICT
            && (claim.path.iter().any(|segment| {
                matches!(
                    segment,
                    checked_trees::CheckedUnitStructuralPathSegment::Case(_)
                )
            }) || transfers.iter().any(|transfer| matches(transfer, claim)))
    }) && transfers
        .iter()
        .enumerate()
        .all(|(transfer_position, transfer)| {
            claims
                .iter()
                .filter(|claim| matches(transfer, claim))
                .count()
                == 1
                && !transfers[..transfer_position].iter().any(|previous| {
                    previous.argument_index == transfer.argument_index
                        && previous.path == transfer.path
                })
        })
}
