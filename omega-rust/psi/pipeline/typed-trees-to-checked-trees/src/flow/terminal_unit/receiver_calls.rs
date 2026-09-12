//! Reconcile implicit receiver operands against completed call signatures.
//!
//! Attachment specialization can erase borrowed self. A retained callee self
//! instead requires the caller's actual loan, including through forwarding
//! methods whose own provisional plan erased self.

use super::*;

mod observations;
pub(super) use observations::reads_receiver;

pub(super) fn reconcile(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    candidates: &mut Vec<CheckedUnitEffectMachinePlan>,
    composed: &mut Vec<CheckedComposedUnitControlMachinePlan>,
    selected_operators: &[crate::SelectedOperatorApplication],
    selected_float_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
) {
    // Receiver retention grows along the already-checked call graph. Rebuild
    // a caller with the ordinary planner rather than shifting its parameter
    // indices, stores, claims, and provider operands by hand.
    loop {
        let retained = candidates
            .iter()
            .filter(|plan| borrowed_self(plan).is_some())
            .map(|plan| plan.state)
            .chain(composed.iter().filter_map(|plan| {
                let entry = plan.states.first()?;
                entry
                    .structural_parameters
                    .iter()
                    .any(|parameter| {
                        parameter.is_self && parameter.access != CheckedStructuralAccess::Owned
                    })
                    .then_some(entry.state)
            }))
            .chain(
                facts
                    .flow
                    .terminal_structural_scalar_returns
                    .machines
                    .iter()
                    .filter(|plan| {
                        plan.structural_parameters.iter().any(|parameter| {
                            parameter.is_self && parameter.access != CheckedStructuralAccess::Owned
                        })
                    })
                    .map(|plan| plan.state),
            )
            .chain(
                facts
                    .flow
                    .terminal_scalar_graphs
                    .machines
                    .iter()
                    .filter_map(|plan| {
                        let state = plan.states.first()?;
                        state
                            .structural_parameters
                            .iter()
                            .any(|parameter| {
                                parameter.is_self
                                    && parameter.access != CheckedStructuralAccess::Owned
                            })
                            .then_some(state.state)
                    }),
            )
            .collect::<Vec<_>>();
        let demanded = candidates
            .iter()
            .filter(|plan| borrowed_self(plan).is_none())
            .filter(|plan| {
                plan.operations.iter().any(|operation| {
                    let (CheckedUnitEffectOperationPlan::CallUnit {
                        coordinate,
                        target_state,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::StructuralCall {
                        coordinate,
                        target_state,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::ScalarCall {
                        coordinate,
                        target_state,
                        ..
                    }) = operation
                    else {
                        return false;
                    };
                    retained.contains(target_state)
                        && receiver_place(
                            program,
                            facts,
                            plan.machine,
                            plan.state,
                            *coordinate,
                            *target_state,
                        )
                        .is_some_and(|place| {
                            is_self_root(program, plan.machine, plan.state, &place)
                        })
                })
            })
            .map(|plan| plan.machine)
            .collect::<Vec<_>>();
        if demanded.is_empty() {
            break;
        }
        candidates.retain_mut(|plan| {
            if !demanded.contains(&plan.machine) {
                return true;
            }
            let Some(machine) = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == plan.machine)
            else {
                return false;
            };
            let Some(rebuilt) = control::build_checked_machine_with(
                program,
                facts,
                shapes,
                machine,
                selected_operators,
                selected_float_applications,
                true,
            ) else {
                return false;
            };
            if borrowed_self(&rebuilt).is_none() {
                return false;
            }
            *plan = rebuilt;
            true
        });
    }

    reconcile_operands(program, facts, candidates, composed);
}

fn borrowed_self(
    plan: &CheckedUnitEffectMachinePlan,
) -> Option<(usize, &CheckedUnitStructuralParameterPlan)> {
    plan.structural_parameters
        .iter()
        .enumerate()
        .find(|(_, parameter)| {
            parameter.is_self && parameter.access != CheckedStructuralAccess::Owned
        })
}

/// Rejoin receiver operands after ordinary and graph entry signatures exist.
fn reconcile_operands(
    program: &TypedTrees,
    facts: &CheckFacts,
    candidates: &mut Vec<CheckedUnitEffectMachinePlan>,
    composed: &mut Vec<CheckedComposedUnitControlMachinePlan>,
) {
    let retained =
        candidates
            .iter()
            .filter_map(|plan| {
                let (index, receiver) = borrowed_self(plan)?;
                Some((
                    plan.machine,
                    plan.state,
                    index,
                    receiver.clone(),
                    plan.structural_parameters.len(),
                ))
            })
            .chain(composed.iter().filter_map(|plan| {
                let entry = plan.states.first()?;
                let (index, receiver) =
                    entry
                        .structural_parameters
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| {
                            parameter.is_self && parameter.access != CheckedStructuralAccess::Owned
                        })?;
                Some((
                    plan.machine,
                    entry.state,
                    index,
                    receiver.clone(),
                    entry.structural_parameters.len(),
                ))
            }))
            .chain(
                facts
                    .flow
                    .terminal_structural_scalar_returns
                    .machines
                    .iter()
                    .filter_map(|plan| {
                        let (index, receiver) =
                            plan.structural_parameters.iter().enumerate().find(
                                |(_, parameter)| {
                                    parameter.is_self
                                        && parameter.access != CheckedStructuralAccess::Owned
                                },
                            )?;
                        Some((
                            plan.machine,
                            plan.state,
                            index,
                            receiver.clone(),
                            plan.structural_parameters.len(),
                        ))
                    }),
            )
            .chain(
                facts
                    .flow
                    .terminal_scalar_graphs
                    .machines
                    .iter()
                    .filter_map(|plan| {
                        let state = plan.states.first()?;
                        let (index, receiver) =
                            state.structural_parameters.iter().enumerate().find(
                                |(_, parameter)| {
                                    parameter.is_self
                                        && parameter.access != CheckedStructuralAccess::Owned
                                },
                            )?;
                        Some((
                            plan.machine,
                            state.state,
                            index,
                            receiver.clone(),
                            state.structural_parameters.len(),
                        ))
                    }),
            )
            .collect::<Vec<_>>();
    let reconcile_state =
        |machine,
         state,
         parameters: &[CheckedUnitStructuralParameterPlan],
         operations: &mut [CheckedUnitEffectOperationPlan]| {
            let results = operations
                .iter()
                .filter_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
                    | CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
                    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. } => {
                        Some(result.clone())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            for operation in operations {
                let (
                    coordinate,
                    target_machine,
                    target_state,
                    structural_arguments,
                    claim_transfers,
                ) = match operation {
                    CheckedUnitEffectOperationPlan::CallUnit {
                        coordinate,
                        target_machine,
                        target_state,
                        structural_arguments,
                        claim_transfers,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::ScalarCall {
                        coordinate,
                        target_machine,
                        target_state,
                        structural_arguments,
                        claim_transfers,
                        ..
                    } => (
                        coordinate,
                        target_machine,
                        target_state,
                        structural_arguments,
                        Some(claim_transfers),
                    ),
                    CheckedUnitEffectOperationPlan::StructuralCall {
                        coordinate,
                        target_machine,
                        target_state,
                        structural_arguments,
                        ..
                    } => (
                        coordinate,
                        target_machine,
                        target_state,
                        structural_arguments,
                        None,
                    ),
                    _ => continue,
                };
                let Some((_, _, receiver_index, target, count)) =
                    retained.iter().find(|(machine, state, ..)| {
                        machine == target_machine && state == target_state
                    })
                else {
                    continue;
                };
                let Some(place) =
                    receiver_place(program, facts, machine, state, *coordinate, *target_state)
                else {
                    return false;
                };
                let Some(argument) = receiver_argument(
                    program,
                    machine,
                    state,
                    *coordinate,
                    parameters,
                    &place,
                    target,
                )
                .or_else(|| {
                    result_receiver_argument(
                        program,
                        facts,
                        machine,
                        state,
                        *coordinate,
                        *target_state,
                        &results,
                        &place,
                        target,
                    )
                }) else {
                    return false;
                };
                if structural_arguments.len().checked_add(1) != Some(*count)
                    || *receiver_index > structural_arguments.len()
                {
                    return false;
                }
                structural_arguments.insert(*receiver_index, argument);
                // A retained receiver is a loan. Preserve the indices of any
                // ownership transfers belonging to the other arguments.
                for transfer in claim_transfers.into_iter().flatten() {
                    if transfer.argument_index as usize >= *receiver_index {
                        let Some(position) = transfer.argument_index.checked_add(1) else {
                            return false;
                        };
                        transfer.argument_index = position;
                    }
                }
            }
            true
        };
    candidates.retain_mut(|plan| {
        reconcile_state(
            plan.machine,
            plan.state,
            &plan.structural_parameters,
            &mut plan.operations,
        )
    });
    composed.retain_mut(|plan| {
        plan.states.iter_mut().all(|state| {
            reconcile_state(
                plan.machine,
                state.state,
                &state.structural_parameters,
                &mut state.operations,
            )
        })
    });
}

#[allow(clippy::too_many_arguments)]
fn result_receiver_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    target_state: SymbolHandle,
    results: &[CheckedUnitStructuralResultBindingPlan],
    place: &crate::flow::CanonicalPlace,
    target: &CheckedUnitStructuralParameterPlan,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    if !matches!(
        target.access,
        CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
    ) || !target.qualifications.is_empty()
    {
        return None;
    }
    let source = crate::find_state(program, state)?;
    let mut matching = results.iter().filter_map(|result| {
        if result.statement_index >= coordinate.statement_index {
            return None;
        }
        let StatementNode::LocalData(local) = program
            .statement_table
            .statements(source.statement_nodes)
            .get(result.statement_index as usize)?
        else {
            return None;
        };
        (local.symbol == symbol).then_some((result, local))
    });
    let (result, local) = matching.next()?;
    if matching.next().is_some()
        || !local.initial_value.is_valid()
        || !validation::has_plain_owned_contents_with_numeric_constraints(
            program,
            local.type_reference,
        )
        || program
            .normalized_type_identity(local.type_reference)
            .as_str()
            != result.type_identity
        || program.type_multiplicity(local.type_reference) != result.multiplicity
        || result.multiplicity == Multiplicity::Linear
    {
        return None;
    }
    let flow = state_flow(facts, machine, state)?;
    let mut calls = facts
        .flow
        .control
        .calls
        .span(flow.calls)?
        .iter()
        .filter(|call| {
            call.statement_index == coordinate.statement_index as usize
                && call.call_ordinal == coordinate.call_ordinal as usize
                && call.target_symbol == target_state
        });
    let call = calls.next()?;
    let borrow_state = facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|candidate| candidate.machine_symbol == machine && candidate.state_symbol == state)?;
    let mut borrow_calls = facts
        .borrow
        .calls
        .span_or_empty(borrow_state.calls)
        .iter()
        .filter(|candidate| {
            candidate.statement_index == call.statement_index
                && candidate.call_ordinal == call.call_ordinal
                && candidate.target_symbol == call.target_symbol
        });
    let borrow_call = borrow_calls.next()?;
    // Implicit receivers have their own capture. The argument access roster
    // contains explicit operands and must not be used to invent a receiver loan.
    if calls.next().is_some()
        || borrow_calls.next().is_some()
        || !call.has_receiver
        || !borrow_call.has_receiver
        || borrow_call.receiver_symbol != call.receiver_symbol
        || (target.access == CheckedStructuralAccess::MutableBorrow && !local.is_mutable)
    {
        return None;
    }
    let (reference, path) =
        calls::projected_argument_path(program, state, coordinate.statement_index as usize, place)?;
    if base_type_identity(program, reference, &[])? != target.type_identity {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path,
        type_identity: target.type_identity.clone(),
        access: target.access,
    })
}

fn receiver_place(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    target: SymbolHandle,
) -> Option<crate::flow::CanonicalPlace> {
    let statement_index = usize::try_from(coordinate.statement_index).ok()?;
    let call_ordinal = usize::try_from(coordinate.call_ordinal).ok()?;
    let flow = state_flow(facts, machine, state)?;
    let mut calls = facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .iter()
        .filter(|call| {
            call.statement_index == statement_index
                && call.call_ordinal == call_ordinal
                && call.target_symbol == target
                && call.has_receiver
        });
    calls.next()?;
    if calls.next().is_some() {
        return None;
    }
    let site = crate::find_call_site(program, machine, state, statement_index, call_ordinal)?;
    let mut place =
        crate::flow::canonical_receiver_place_for_call_site(program, machine, state, &site)?;
    let authored_machine = program
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine)?;
    let authored_state = crate::find_state_in_machine(program, machine, state)?;
    if let Some(aliases) =
        receiver_aliases::prefix(program, facts, authored_machine, authored_state)
        && let Some(alias) = aliases
            .iter()
            .find(|alias| place.root == facts::PlaceRoot::Symbol(alias.owner))
    {
        place.root = facts::PlaceRoot::Symbol(alias.root);
        let mut segments = alias.segments.clone();
        segments.extend_from_slice(&place.segments);
        place.segments = segments;
    }
    Some(place)
}

fn is_self_root(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    place: &crate::flow::CanonicalPlace,
) -> bool {
    crate::find_state(program, state).is_some_and(|state| {
        program.state_parameters(state).iter().any(|parameter| {
            parameter.is_self
                && matches!(place.root, facts::PlaceRoot::Symbol(root)
                if root == machine || root == parameter.symbol)
        })
    })
}

fn receiver_argument(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    parameters: &[CheckedUnitStructuralParameterPlan],
    place: &crate::flow::CanonicalPlace,
    target: &CheckedUnitStructuralParameterPlan,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let facts::PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    let source = program.state_parameters(crate::find_state(program, state)?);
    let (position, _) = source.iter().enumerate().find(|(_, parameter)| {
        parameter.symbol == root || (parameter.is_self && root == machine)
    })?;
    let (index, parameter) = parameters
        .iter()
        .enumerate()
        .find(|(_, parameter)| usize::try_from(parameter.position).ok() == Some(position))?;
    if parameter.qualifications != target.qualifications
        || parameter.multiplicity != target.multiplicity
    {
        return None;
    }
    use CheckedStructuralAccess::{MutableBorrow, Owned, SharedBorrow, WriteOnlyBorrow};
    if !matches!(
        (parameter.access, target.access),
        (
            Owned | MutableBorrow,
            SharedBorrow | MutableBorrow | WriteOnlyBorrow
        ) | (SharedBorrow, SharedBorrow)
            | (WriteOnlyBorrow, WriteOnlyBorrow)
    ) {
        return None;
    }
    let path = if place.segments.is_empty() {
        if parameter.type_identity != target.type_identity {
            return None;
        }
        Vec::new()
    } else {
        // Reuse the ordinary exact-place resolver and the existing Terminal
        // structural-path exclusive subloan contract. The root keeps its container
        // type; the operand names the leaf, without transferring ownership.
        if !matches!(
            (parameter.access, target.access),
            (
                MutableBorrow,
                SharedBorrow | MutableBorrow | WriteOnlyBorrow
            ) | (SharedBorrow, SharedBorrow)
                | (WriteOnlyBorrow, WriteOnlyBorrow)
        ) || parameter.multiplicity != Multiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !place.segments.iter().all(|segment| {
                matches!(segment, facts::PlaceSegment::Field { .. })
                    || (target.access == WriteOnlyBorrow
                        && matches!(segment, facts::PlaceSegment::FixedIndex { .. }))
            })
        {
            return None;
        }
        let (projected, path) = calls::projected_argument_path(
            program,
            state,
            usize::try_from(coordinate.statement_index).ok()?,
            place,
        )?;
        if base_type_identity(program, projected, &[])? != target.type_identity {
            return None;
        }
        path
    };
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(index).ok()?,
        },
        path,
        type_identity: target.type_identity.clone(),
        access: target.access,
    })
}
