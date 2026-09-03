//! Atomic checked custody for two-predecessor dynamic descriptor joins.
//!
//! This owner consumes already-complete branch-local call plans and descriptor
//! transfer paths. It does not discover conformances or invent a joined table.

use super::*;

pub(super) fn promote_two_predecessor_dynamic_scalar_joins(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    plans: &mut psi_checked_trees::CheckedDynamicDispatchPlans,
) -> Option<()> {
    let inbound_counts = inbound_call_site_counts(program, facts);
    let mut consumed = Vec::new();
    let mut joined = Vec::new();
    for machine in program.machines() {
        let Some(control) = super::super::composed_control::admit_dynamic_join_control_topology(
            program, facts, shapes, machine,
        ) else {
            continue;
        };
        let branch_calls = control
            .successors
            .iter()
            .map(|successor| {
                let candidates = plans
                    .direct_scalar_calls
                    .iter()
                    .filter(|plan| {
                        plan.caller_machine == machine.symbol
                            && plan.caller_state == successor.target_state
                    })
                    .collect::<Vec<_>>();
                let [candidate] = candidates.as_slice() else {
                    return None;
                };
                Some((*candidate).clone())
            })
            .collect::<Option<Vec<_>>>();
        let Some(branch_calls) = branch_calls else {
            continue;
        };
        let [when_true_call, when_false_call] = branch_calls.as_slice() else {
            continue;
        };
        if !joined_scalar_branches_match(
            &control,
            when_true_call,
            when_false_call,
            &plans.transfers,
            &inbound_counts,
        ) {
            continue;
        }
        consumed.extend(branch_calls.iter().cloned());
        joined.push(psi_checked_trees::CheckedJoinedDynamicScalarCallPlan {
            caller_machine: machine.symbol,
            entry_state: control.entry_state,
            caller_attachment_type_identity: control.attachment_type_identity,
            scalar_parameters: control.scalar_parameters,
            guard: control.guard,
            when_true: psi_checked_trees::CheckedJoinedDynamicScalarCallBranchPlan {
                successor: control.successors[0].clone(),
                call: when_true_call.clone(),
            },
            when_false: psi_checked_trees::CheckedJoinedDynamicScalarCallBranchPlan {
                successor: control.successors[1].clone(),
                call: when_false_call.clone(),
            },
        });
    }
    plans
        .direct_scalar_calls
        .retain(|plan| !consumed.contains(plan));
    joined.sort_by_key(|plan| {
        (
            plan.caller_machine.arena_index(),
            plan.caller_machine.generation(),
            plan.entry_state.arena_index(),
            plan.entry_state.generation(),
        )
    });
    plans.joined_scalar_calls = joined;
    Some(())
}

pub(super) fn promote_two_predecessor_dynamic_unit_joins(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    plans: &mut psi_checked_trees::CheckedDynamicDispatchPlans,
) -> Option<()> {
    let inbound_counts = inbound_call_site_counts(program, facts);
    let mut consumed = Vec::new();
    let mut joined = Vec::new();
    for machine in program.machines() {
        let Some(control) = super::super::composed_control::admit_dynamic_join_control_topology(
            program, facts, shapes, machine,
        ) else {
            continue;
        };
        let branch_calls = control
            .successors
            .iter()
            .map(|successor| {
                let candidates = plans
                    .direct_unit_calls
                    .iter()
                    .filter(|plan| {
                        plan.caller_machine == machine.symbol
                            && plan.caller_state == successor.target_state
                    })
                    .collect::<Vec<_>>();
                let [candidate] = candidates.as_slice() else {
                    return None;
                };
                Some((*candidate).clone())
            })
            .collect::<Option<Vec<_>>>();
        let Some(branch_calls) = branch_calls else {
            continue;
        };
        let [when_true_call, when_false_call] = branch_calls.as_slice() else {
            continue;
        };
        if !joined_unit_branches_match(
            &control,
            when_true_call,
            when_false_call,
            &plans.transfers,
            &inbound_counts,
        ) {
            continue;
        }
        consumed.extend(branch_calls.iter().cloned());
        joined.push(psi_checked_trees::CheckedJoinedDynamicUnitCallPlan {
            caller_machine: machine.symbol,
            entry_state: control.entry_state,
            caller_attachment_type_identity: control.attachment_type_identity,
            scalar_parameters: control.scalar_parameters,
            guard: control.guard,
            when_true: psi_checked_trees::CheckedJoinedDynamicUnitCallBranchPlan {
                successor: control.successors[0].clone(),
                call: when_true_call.clone(),
            },
            when_false: psi_checked_trees::CheckedJoinedDynamicUnitCallBranchPlan {
                successor: control.successors[1].clone(),
                call: when_false_call.clone(),
            },
        });
    }
    plans
        .direct_unit_calls
        .retain(|plan| !consumed.contains(plan));
    joined.sort_by_key(|plan| {
        (
            plan.caller_machine.arena_index(),
            plan.caller_machine.generation(),
            plan.entry_state.arena_index(),
            plan.entry_state.generation(),
        )
    });
    plans.joined_unit_calls = joined;
    Some(())
}

fn joined_scalar_branches_match(
    control: &super::super::composed_control::DynamicJoinControlTopology,
    when_true: &psi_checked_trees::CheckedDynamicScalarCallPlan,
    when_false: &psi_checked_trees::CheckedDynamicScalarCallPlan,
    transfers: &[psi_checked_trees::CheckedDynamicDescriptorTransferPlan],
    inbound_counts: &BTreeMap<(u32, u32), usize>,
) -> bool {
    if when_true.result.primitive_type != when_false.result.primitive_type
        || when_true.caller_structural_scalar_field_store.is_some()
        || when_false.caller_structural_scalar_field_store.is_some()
        || when_true.unit_continuation.is_some()
        || when_false.unit_continuation.is_some()
    {
        return false;
    }
    joined_branches_match(
        control,
        JoinBranchView::scalar(when_true),
        JoinBranchView::scalar(when_false),
        transfers,
        inbound_counts,
    )
}

fn joined_unit_branches_match(
    control: &super::super::composed_control::DynamicJoinControlTopology,
    when_true: &psi_checked_trees::CheckedDynamicUnitCallPlan,
    when_false: &psi_checked_trees::CheckedDynamicUnitCallPlan,
    transfers: &[psi_checked_trees::CheckedDynamicDescriptorTransferPlan],
    inbound_counts: &BTreeMap<(u32, u32), usize>,
) -> bool {
    joined_branches_match(
        control,
        JoinBranchView::unit(when_true),
        JoinBranchView::unit(when_false),
        transfers,
        inbound_counts,
    )
}

struct JoinBranchView<'a> {
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    caller_attachment_type_identity: &'a str,
    caller_multiplicity: psi_language_semantics::Multiplicity,
    caller_parameter_access: CheckedStructuralAccess,
    caller_contract_report_fingerprint: u64,
    caller_contract_commitment: &'a psi_checked_trees::MachineContractCommitment,
    caller_service_reach: &'a ServiceReachSummary,
    coordinate: CheckedUnitCallCoordinate,
    receiver_binding: SymbolHandle,
    selection: &'a psi_checked_trees::DynamicConformanceBindingFact,
    target_trait: SymbolHandle,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
    requirement_identity: &'a str,
    checked_call_service_reach: &'a ServiceReachSummary,
    forwarded_origin: Option<(SymbolHandle, SymbolHandle, SymbolHandle)>,
    forwarding_transfers: &'a [psi_checked_trees::CheckedDynamicDescriptorTransferPlan],
}

impl<'a> JoinBranchView<'a> {
    fn scalar(plan: &'a psi_checked_trees::CheckedDynamicScalarCallPlan) -> Self {
        let forwarded_origin = match plan.origin {
            psi_checked_trees::CheckedDynamicScalarCallOrigin::Local => None,
            psi_checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
                machine,
                state,
                parameter,
                ..
            } => Some((machine, state, parameter)),
        };
        Self {
            caller_machine: plan.caller_machine,
            caller_state: plan.caller_state,
            caller_attachment_type_identity: &plan.caller_attachment_type_identity,
            caller_multiplicity: plan.caller_multiplicity,
            caller_parameter_access: plan.caller_parameter_access,
            caller_contract_report_fingerprint: plan.caller_contract_report_fingerprint,
            caller_contract_commitment: &plan.caller_contract_commitment,
            caller_service_reach: &plan.caller_service_reach,
            coordinate: plan.coordinate,
            receiver_binding: plan.receiver_binding,
            selection: &plan.selection,
            target_trait: plan.target_trait,
            declaring_trait: plan.declaring_trait,
            requirement: plan.requirement,
            requirement_identity: &plan.requirement_identity,
            checked_call_service_reach: &plan.checked_call_service_reach,
            forwarded_origin,
            forwarding_transfers: &plan.forwarding_transfers,
        }
    }

    fn unit(plan: &'a psi_checked_trees::CheckedDynamicUnitCallPlan) -> Self {
        let forwarded_origin = match plan.origin {
            psi_checked_trees::CheckedDynamicUnitCallOrigin::Local => None,
            psi_checked_trees::CheckedDynamicUnitCallOrigin::Forwarded {
                machine,
                state,
                parameter,
                ..
            } => Some((machine, state, parameter)),
        };
        Self {
            caller_machine: plan.caller_machine,
            caller_state: plan.caller_state,
            caller_attachment_type_identity: &plan.caller_attachment_type_identity,
            caller_multiplicity: plan.caller_multiplicity,
            caller_parameter_access: plan.caller_parameter_access,
            caller_contract_report_fingerprint: plan.caller_contract_report_fingerprint,
            caller_contract_commitment: &plan.caller_contract_commitment,
            caller_service_reach: &plan.caller_service_reach,
            coordinate: plan.coordinate,
            receiver_binding: plan.receiver_binding,
            selection: &plan.selection,
            target_trait: plan.target_trait,
            declaring_trait: plan.declaring_trait,
            requirement: plan.requirement,
            requirement_identity: &plan.requirement_identity,
            checked_call_service_reach: &plan.checked_call_service_reach,
            forwarded_origin,
            forwarding_transfers: &plan.forwarding_transfers,
        }
    }
}

fn joined_branches_match(
    control: &super::super::composed_control::DynamicJoinControlTopology,
    when_true: JoinBranchView<'_>,
    when_false: JoinBranchView<'_>,
    transfers: &[psi_checked_trees::CheckedDynamicDescriptorTransferPlan],
    inbound_counts: &BTreeMap<(u32, u32), usize>,
) -> bool {
    if when_true.caller_machine != when_false.caller_machine
        || when_true.caller_attachment_type_identity != control.attachment_type_identity
        || when_false.caller_attachment_type_identity != control.attachment_type_identity
        || when_true.caller_multiplicity != when_false.caller_multiplicity
        || when_true.caller_parameter_access != when_false.caller_parameter_access
        || when_true.caller_contract_report_fingerprint
            != when_false.caller_contract_report_fingerprint
        || when_true.caller_contract_commitment != when_false.caller_contract_commitment
        || when_true.caller_service_reach != when_false.caller_service_reach
        || when_true.target_trait != when_false.target_trait
        || when_true.declaring_trait != when_false.declaring_trait
        || when_true.requirement != when_false.requirement
        || when_true.requirement_identity != when_false.requirement_identity
        || when_true.checked_call_service_reach != when_false.checked_call_service_reach
        || when_true.forwarded_origin != when_false.forwarded_origin
        || when_true.forwarding_transfers != when_false.forwarding_transfers
        || when_true.selection.machine != when_true.caller_machine
        || when_true.selection.state != when_true.caller_state
        || when_false.selection.machine != when_false.caller_machine
        || when_false.selection.state != when_false.caller_state
        || when_true.selection == when_false.selection
    {
        return false;
    }
    let Some((dispatch_machine, dispatch_state, dispatch_parameter)) = when_true.forwarded_origin
    else {
        return false;
    };
    let (target_machine, target_state, parameter) = match when_true.forwarding_transfers {
        [] => (dispatch_machine, dispatch_state, dispatch_parameter),
        forwarding @ [first, ..]
            if first.source
                == psi_checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                    parameter_position: 0,
                }
                && first.source_predecessor_count == 2
                && first.source_paths.len() == 2
                && forwarding.iter().all(|transfer| {
                    transfer.source
                        == psi_checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                            parameter_position: 0,
                        }
                        && transfer.source_paths.len() == 2
                        && transfer.has_complete_source_custody(transfers)
                })
                && forwarding.windows(2).all(|pair| {
                    pair[0].target_machine == pair[1].caller_machine
                        && pair[0].target_state == pair[1].caller_state
                        && pair[0].parameter == pair[1].source_binding
                        && pair[1].source_predecessor_count == 1
                })
                && forwarding.last().is_some_and(|last| {
                    last.target_machine == dispatch_machine
                        && last.target_state == dispatch_state
                        && last.parameter == dispatch_parameter
                }) =>
        {
            (
                first.caller_machine,
                first.caller_state,
                first.source_binding,
            )
        }
        _ => return false,
    };
    if inbound_counts.get(&(target_state.arena_index(), target_state.generation())) != Some(&2) {
        return false;
    }
    let exact_root = |plan: &JoinBranchView<'_>| {
        let roots = transfers
            .iter()
            .filter(|transfer| {
                transfer.caller_machine == plan.caller_machine
                    && transfer.caller_state == plan.caller_state
                    && transfer.coordinate == plan.coordinate
                    && transfer.target_machine == target_machine
                    && transfer.target_state == target_state
                    && transfer.parameter_position == 0
                    && transfer.parameter == parameter
                    && transfer.target_trait == plan.target_trait
                    && transfer.source_binding == plan.receiver_binding
                    && transfer.source
                        == psi_checked_trees::CheckedDynamicDescriptorTransferSource::Selection
                    && transfer.sole_selection() == Some(&plan.selection)
                    && transfer.has_complete_source_custody(transfers)
            })
            .collect::<Vec<_>>();
        let [root] = roots.as_slice() else {
            return None;
        };
        Some(root.edge())
    };
    let (Some(true_root), Some(false_root)) = (exact_root(&when_true), exact_root(&when_false))
    else {
        return false;
    };
    true_root != false_root
        && transfers
            .iter()
            .filter(|transfer| {
                transfer.target_machine == target_machine
                    && transfer.target_state == target_state
                    && transfer.parameter == parameter
                    && transfer.target_trait == when_true.target_trait
            })
            .count()
            == 2
}
