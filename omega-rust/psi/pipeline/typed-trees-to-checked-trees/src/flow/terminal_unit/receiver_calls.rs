//! Reconcile implicit receiver operands against completed Unit signatures.
//!
//! Attachment specialization can erase borrowed self. A retained callee self
//! instead requires the caller's actual loan, including through forwarding
//! methods whose own provisional plan erased self.

use super::*;

pub(super) fn reconcile(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    candidates: &mut Vec<CheckedUnitEffectMachinePlan>,
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
            .collect::<Vec<_>>();
        let demanded = candidates
            .iter()
            .filter(|plan| borrowed_self(plan).is_none())
            .filter(|plan| {
                plan.operations.iter().any(|operation| {
                    let CheckedUnitEffectOperationPlan::CallUnit {
                        coordinate,
                        target_state,
                        ..
                    } = operation
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

    let retained = candidates
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
        .collect::<Vec<_>>();
    candidates.retain_mut(|plan| {
        for operation in &mut plan.operations {
            let CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                target_machine,
                target_state,
                structural_arguments,
                claim_transfers,
                ..
            } = operation
            else {
                continue;
            };
            let Some((_, _, receiver_index, target, parameter_count)) = retained
                .iter()
                .find(|(machine, state, ..)| machine == target_machine && state == target_state)
            else {
                continue;
            };
            if structural_arguments.len().checked_add(1) != Some(*parameter_count) {
                return false;
            }
            let Some(place) = receiver_place(
                program,
                facts,
                plan.machine,
                plan.state,
                *coordinate,
                *target_state,
            ) else {
                return false;
            };
            let Some(argument) = receiver_argument(
                program,
                plan.machine,
                plan.state,
                *coordinate,
                &plan.structural_parameters,
                &place,
                target,
            ) else {
                return false;
            };
            if *receiver_index > structural_arguments.len() {
                return false;
            }
            structural_arguments.insert(*receiver_index, argument);
            // The new operand is a loan, never an ownership transfer. Existing
            // transfers still name their original operands after insertion.
            for transfer in claim_transfers {
                if usize::try_from(transfer.argument_index)
                    .ok()
                    .is_some_and(|index| index >= *receiver_index)
                {
                    let Some(index) = transfer.argument_index.checked_add(1) else {
                        return false;
                    };
                    transfer.argument_index = index;
                }
            }
        }
        true
    });
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
    crate::flow::canonical_receiver_place_for_call_site(program, machine, state, &site)
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
        // field-path exclusive subloan contract. The root keeps its container
        // type; the operand names the leaf, without transferring ownership.
        if !matches!(
            (parameter.access, target.access),
            (MutableBorrow, MutableBorrow | WriteOnlyBorrow) | (WriteOnlyBorrow, WriteOnlyBorrow)
        ) || parameter.multiplicity != Multiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !place
                .segments
                .iter()
                .all(|segment| matches!(segment, facts::PlaceSegment::Field { .. }))
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
