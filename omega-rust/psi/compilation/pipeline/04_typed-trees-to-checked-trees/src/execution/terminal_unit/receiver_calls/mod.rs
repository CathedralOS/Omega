//! Reconcile implicit receiver operands against completed call signatures.
//!
//! Attachment specialization can erase borrowed self. A retained callee self
//! instead requires the caller's actual loan, including through forwarding
//! methods whose own provisional plan erased self.
//! Local receivers are joined to their exact initializer producer. Operand
//! copies and nested calls can produce other values in that same statement;
//! sharing a statement or a type does not make those values the local's result.
use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::types::{ShapeCollector, base_type_identity, state_flow};

use crate::execution::terminal_unit::calls;
use crate::execution::terminal_unit::control;
use crate::execution::terminal_unit::receiver_aliases;

use super::{
    CheckFacts, CheckedComposedUnitControlMachinePlan, CheckedStructuralAccess,
    CheckedUnitCallCoordinate, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, Multiplicity,
    StatementNode, SymbolHandle, TypedTrees,
};
mod observations;
pub(in crate::execution::terminal_unit) use observations::uses_receiver_storage;

pub(super) fn reconcile(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    candidates: &mut Vec<CheckedUnitEffectMachinePlan>,
    composed: &mut Vec<CheckedComposedUnitControlMachinePlan>,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
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
                scalar_callees
                    .structural_returns
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
                    // Receiver retention reconciles ordinary calls only.
                    let Some(call) = operation.call().filter(|call| !call.is_boundary()) else {
                        return false;
                    };
                    retained.contains(&call.target.state)
                        && receiver_place(
                            program,
                            facts,
                            plan.machine,
                            plan.state,
                            call.coordinate,
                            call.target.state,
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
            let Some(machine) = crate::lookup::machine_by_symbol(program, plan.machine) else {
                return false;
            };
            let Some(rebuilt) = control::build_checked_machine_with(
                program,
                facts,
                scalar_callees,
                shapes,
                machine,
                true,
                call_frames,
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

    reconcile_operands(program, facts, scalar_callees, candidates, composed);
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
    scalar_callees: ScalarCalleePlans<'_>,
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
                // The builder overlap below drops a Unit-result graph whose
                // result-free ordinary sibling covers the same entry state;
                // a graph that cannot survive may not decide the receiver
                // roster either — the emitted signature follows the ordinary
                // plan then, and it keeps no borrowed-self slot.
                if matches!(plan.result, checked_trees::CheckedControlResultPlan::Unit)
                    && candidates.iter().any(|ordinary| {
                        ordinary.machine == plan.machine
                            && entry.state == ordinary.state
                            && ordinary.structural_result.is_none()
                            && ordinary.scalar_result.is_none()
                            && ordinary.scalar_control.is_none()
                    })
                {
                    return None;
                }
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
                scalar_callees
                    .structural_returns
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
    // `prior` holds operations that ran before `operations` in the same
    // state without being reconciled here: their results may still own a
    // receiver place.
    let reconcile_state =
        |machine,
         state,
         parameters: &[CheckedUnitStructuralParameterPlan],
         prior: &[CheckedUnitEffectOperationPlan],
         operations: &mut [CheckedUnitEffectOperationPlan]| {
            let results = prior
                .iter()
                .chain(operations.iter())
                .filter_map(|operation| local_initializer_result(program, facts, state, operation))
                .cloned()
                .collect::<Vec<_>>();
            for operation in operations {
                // Only ordinary calls retain a receiver operand.
                let Some(call) = operation.call_mut().filter(|call| !call.is_boundary()) else {
                    continue;
                };
                let coordinate = call.coordinate;
                let target_machine = call.target.machine;
                let target_state = call.target.state;
                let is_scalar = call.scalar_result().is_some();
                // A structural call's custody claims keep their argument indices.
                let shifts_claim_transfers = call.structural_result().is_none();
                // A borrowed `self` stays ambient on the attachment carrier
                // for a scalar-graph callee: its graph keeps no receiver
                // slot, so calls dispatched there take no receiver operand.
                if is_scalar
                    && facts
                        .flow
                        .terminal_scalar_graphs
                        .for_machine(target_machine)
                        .is_some_and(|graph| {
                            graph.states.iter().any(|state| {
                                state.state == target_state
                                    && state
                                        .structural_parameters
                                        .iter()
                                        .all(|parameter| !parameter.is_self)
                            })
                        })
                {
                    continue;
                }
                let Some((_, _, receiver_index, target, count)) =
                    retained.iter().find(|(machine, state, ..)| {
                        *machine == target_machine && *state == target_state
                    })
                else {
                    continue;
                };
                let Some(place) =
                    receiver_place(program, facts, machine, state, coordinate, target_state)
                else {
                    return false;
                };
                let Some(argument) = receiver_argument(
                    program, machine, state, coordinate, parameters, &place, target,
                )
                .or_else(|| {
                    result_receiver_argument(
                        program,
                        facts,
                        machine,
                        state,
                        coordinate,
                        target_state,
                        &results,
                        &place,
                        target,
                    )
                }) else {
                    return false;
                };
                if call.structural_arguments.len().checked_add(1) != Some(*count)
                    || *receiver_index > call.structural_arguments.len()
                {
                    return false;
                }
                call.structural_arguments.insert(*receiver_index, argument);
                if !shifts_claim_transfers {
                    continue;
                }
                // A retained receiver is a loan. Preserve the indices of any
                // ownership transfers belonging to the other arguments.
                for transfer in call.claim_transfers_mut() {
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
            &[],
            &mut plan.operations,
        )
    });
    composed.retain_mut(|plan| {
        plan.states.iter_mut().all(|state| {
            // A terminal call arm runs after the state's whole sequence, so
            // that sequence's results precede it.
            reconcile_state(
                plan.machine,
                state.state,
                &state.structural_parameters,
                &[],
                &mut state.operations,
            ) && state.terminator.arm_calls_mut().into_iter().all(|arm| {
                reconcile_state(
                    plan.machine,
                    state.state,
                    &state.structural_parameters,
                    &state.operations,
                    std::slice::from_mut(arm),
                )
            })
        })
    });
}

fn local_initializer_result<'operation>(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: SymbolHandle,
    operation: &'operation CheckedUnitEffectOperationPlan,
) -> Option<&'operation CheckedUnitStructuralResultBindingPlan> {
    let result = match operation {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => result,
        CheckedUnitEffectOperationPlan::Call(call) => call.structural_result()?,
        _ => return None,
    };
    let source = crate::semantic::calls::find_state(program, state)?;
    let StatementNode::LocalData(local) = program
        .statement_table
        .statements(source.statement_nodes)
        .get(result.statement_index as usize)?
    else {
        return None;
    };
    // Retain the producer's authored identity, not its position among values
    // sequenced for this statement. In particular a copied call operand can
    // even have the same type as the local being initialized.
    let is_initializer = match operation {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            value,
            operand_source,
            ..
        } => {
            operand_source.is_none()
                && facts
                    .values
                    .structural_values
                    .root_for_expression(state, result.statement_index, local.initial_value)
                    .is_some_and(|root| root.root == *value)
        }
        CheckedUnitEffectOperationPlan::Call(call) => {
            call.source_site().flatten()
                == Some(checked_trees::NominalMachineUseSite::Expression(
                    local.initial_value,
                ))
        }
        _ => false,
    };
    is_initializer.then_some(result)
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
    let source = crate::semantic::calls::find_state(program, state)?;
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
        || program
            .normalized_type_identity(local.type_reference)
            .as_str()
            != result.type_identity
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
    {
        return None;
    }
    if let Some(argument) = calls::reference_result_receiver_argument(
        program,
        facts,
        machine,
        source,
        call,
        place,
        local,
        result,
        target_state,
        target,
    ) {
        return Some(argument);
    }
    if !validation::has_plain_owned_contents_with_numeric_constraints(program, local.type_reference)
        || program.type_multiplicity(local.type_reference) != result.multiplicity
        || result.multiplicity == Multiplicity::Linear
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
    let site = crate::semantic::calls::find_call_site(
        program,
        machine,
        state,
        statement_index,
        call_ordinal,
    )?;
    let mut place = crate::flow::canonical_receiver_place_for_call_site(
        program,
        machine,
        state,
        &site,
        statement_index,
    )?;
    let authored_machine = crate::lookup::machine_by_symbol(program, machine)?;
    let authored_state = crate::semantic::calls::find_state_in_machine(program, machine, state)?;
    if let Some(aliases) =
        receiver_aliases::aliases(program, facts, authored_machine, authored_state)
        && let Some(resolved) = receiver_aliases::resolve(&aliases, &place)
    {
        place = resolved;
    }
    Some(place)
}

fn is_self_root(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    place: &crate::flow::CanonicalPlace,
) -> bool {
    crate::semantic::calls::find_state(program, state).is_some_and(|state| {
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
    let source = program.state_parameters(crate::semantic::calls::find_state(program, state)?);
    let (position, _) = source.iter().enumerate().find(|(_, parameter)| {
        parameter.symbol == root || (parameter.is_self && root == machine)
    })?;
    let (index, parameter) = parameters
        .iter()
        .enumerate()
        .find(|(_, parameter)| usize::try_from(parameter.position).ok() == Some(position))?;
    use CheckedStructuralAccess::{MutableBorrow, Owned, SharedBorrow, WriteOnlyBorrow};
    // A whole shared view is unrestricted while its source remains affine.
    // Retain the original parameter as the owner; this agrees with Terminal's
    // shared-affine-loan check and grants no move or projected custody.
    let shared_affine_loan = place.segments.is_empty()
        && parameter.access == Owned
        && parameter.multiplicity == Multiplicity::Affine
        && target.access == SharedBorrow
        && target.multiplicity == Multiplicity::Unrestricted;
    if parameter.qualifications != target.qualifications
        || (parameter.multiplicity != target.multiplicity && !shared_affine_loan)
    {
        return None;
    }
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
        // An owned parameter can lend that subtree just as a mutable parent
        // can; source checking has already established writable receiver use.
        if !matches!(
            (parameter.access, target.access),
            (
                MutableBorrow,
                SharedBorrow | MutableBorrow | WriteOnlyBorrow
            ) | (Owned, MutableBorrow | WriteOnlyBorrow)
                | (SharedBorrow, SharedBorrow)
                | (WriteOnlyBorrow, WriteOnlyBorrow)
        ) || !matches!(
            parameter.multiplicity,
            // An affine root lends the projected subtree under the same
            // loan a shared view grants; its custody stays with the place.
            Multiplicity::Unrestricted | Multiplicity::Affine
        ) || !parameter.qualifications.is_empty()
            || !place.segments.iter().all(|segment| {
                matches!(segment, facts::PlaceSegment::Field { .. })
                    || (matches!(
                        target.access,
                        SharedBorrow | MutableBorrow | WriteOnlyBorrow
                    ) && matches!(segment, facts::PlaceSegment::FixedIndex { .. }))
                    // A runtime index may borrow one shared or mutable
                    // element: the checked `RuntimeIndex` segment retains the
                    // scalar selector and terminal verification replays its
                    // bound rather than trusting this admission. A mutable
                    // receiver keeps the same operand shape — the loan is
                    // exclusive, not static, and still names one element.
                    || (matches!(
                        target.access,
                        SharedBorrow | MutableBorrow
                    ) && matches!(segment, facts::PlaceSegment::Index { .. }))
            })
        {
            return None;
        }
        let (projected, path) = calls::projected_borrowed_path(
            program,
            machine,
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
