//! Planning and resolving reborrow disposition events.

use crate::checks::borrows::resources::lifecycle::span_handle;
use crate::checks::borrows::resources::lifecycle::{
    EphemeralResourceStatus, EphemeralStatuses, LifecycleBoundaryKey, LifecycleEvent,
    LifecycleEventKind, LifecyclePhase, activation_boundary, apply_activation_batch,
    apply_weakening_batch, exact_loan_lifecycle_rows, weakening_event_boundary,
};
use crate::checks::borrows::resources::reborrow_drafts::{
    CheckedReborrowDispositionEventDraft, CheckedReborrowLoanResourceDraft, DispositionTargetIndex,
    ParentResourceIndex,
};
use crate::checks::borrows::resources::{
    reborrow_disposition_drift, reborrow_restored_call_use_drift,
};
use checked_trees::{
    BorrowFacts, CheckedBorrowResourceLifecyclePhase, CheckedDirectBorrowLoanResource,
    CheckedReborrowAccessEffect, CheckedReborrowResourceDisposition, FlowFacts,
    FlowInvalidationSource, ParentLexicalStatusAtChildEnd,
};
use diagnostics::Diagnostic;

pub(crate) fn plan_reborrow_disposition_events(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    direct: &[CheckedDirectBorrowLoanResource],
    reborrows: &[CheckedReborrowLoanResourceDraft],
    installation: &[ParentResourceIndex],
) -> Result<Vec<CheckedReborrowDispositionEventDraft>, Vec<Diagnostic>> {
    let mut states = Vec::new();
    for (machine, state) in direct
        .iter()
        .map(|resource| (resource.machine_symbol, resource.state_symbol))
        .chain(
            reborrows
                .iter()
                .map(|resource| (resource.machine_symbol, resource.state_symbol)),
        )
    {
        if !states.contains(&(machine, state)) {
            states.push((machine, state));
        }
    }

    let mut statuses = EphemeralStatuses::new(direct.len(), reborrows.len());
    let mut dispositions = Vec::new();
    for (machine_symbol, state_symbol) in states {
        let mut events = Vec::new();
        let Some(flow_state) = flow.control.state(machine_symbol, state_symbol) else {
            return Err(reborrow_disposition_drift());
        };
        for (index, resource) in direct.iter().enumerate().filter(|(_, resource)| {
            resource.machine_symbol == machine_symbol && resource.state_symbol == state_symbol
        }) {
            // The lifecycle ledger is joined by its one reader; a drifted
            // ledger is a disposition failure, not a second reconstruction.
            let Ok((activation, weakening)) = exact_loan_lifecycle_rows(
                flow,
                flow_state,
                resource.loan,
                borrow.loans.get(resource.loan).statement_index,
            ) else {
                return Err(reborrow_disposition_drift());
            };
            events.push(LifecycleEvent {
                boundary: activation_boundary(
                    flow.borrow_lifetimes.activations.get(activation).source,
                )?,
                resource: ParentResourceIndex::Direct(index),
                loan_order: resource.loan.arena_index(),
                kind: LifecycleEventKind::Activate { activation },
            });
            let weakening_fact = flow.borrow_lifetimes.weakenings.get(weakening);
            events.push(LifecycleEvent {
                boundary: weakening_event_boundary(weakening_fact.source, weakening_fact.reason)?,
                resource: ParentResourceIndex::Direct(index),
                loan_order: resource.loan.arena_index(),
                kind: LifecycleEventKind::Weaken { weakening },
            });
        }
        for (index, resource) in reborrows.iter().enumerate().filter(|(_, resource)| {
            resource.machine_symbol == machine_symbol && resource.state_symbol == state_symbol
        }) {
            events.push(LifecycleEvent {
                boundary: activation_boundary(resource.activation_source)?,
                resource: ParentResourceIndex::Reborrow(index),
                loan_order: resource.loan.arena_index(),
                kind: LifecycleEventKind::Activate {
                    activation: resource.child_activation,
                },
            });
            events.push(LifecycleEvent {
                boundary: weakening_event_boundary(
                    resource.weakening_source,
                    resource.weakening_reason,
                )?,
                resource: ParentResourceIndex::Reborrow(index),
                loan_order: resource.loan.arena_index(),
                kind: LifecycleEventKind::Weaken {
                    weakening: resource.child_weakening,
                },
            });
        }
        events.sort_by_key(|event| (event.boundary, event.loan_order));

        let mut start = 0usize;
        while start < events.len() {
            let boundary = events[start].boundary;
            let mut end = start + 1;
            while end < events.len() && events[end].boundary == boundary {
                end += 1;
            }
            let batch = &events[start..end];
            if boundary.phase == LifecyclePhase::Activation {
                apply_activation_batch(&mut statuses, batch, installation, reborrows)?;
            } else {
                let completed = apply_weakening_batch(&mut statuses, batch)?;
                let snapshot = statuses.clone();
                for (child_index, child_weakening) in completed {
                    let draft = resolve_disposition_event(
                        flow,
                        child_index,
                        child_weakening,
                        boundary,
                        &mut statuses,
                        &snapshot,
                        reborrows,
                        installation,
                    )?;
                    dispositions.push(draft);
                }
            }
            start = end;
        }
    }
    Ok(dispositions)
}

/// Close one leaf and then only those retired ancestors whose entire child
/// roster is empty. A sibling release never restores whole-parent authority.
/// The batch snapshot supplies the shared roster; evolving statuses decide
/// closure, so simultaneous grandchildren cannot close a parent twice.
fn resolve_disposition_event(
    flow: &FlowFacts,
    child_index: usize,
    child_weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    boundary: LifecycleBoundaryKey,
    statuses: &mut EphemeralStatuses,
    snapshot: &EphemeralStatuses,
    reborrows: &[CheckedReborrowLoanResourceDraft],
    installation: &[ParentResourceIndex],
) -> Result<CheckedReborrowDispositionEventDraft, Vec<Diagnostic>> {
    let Some(child) = reborrows.get(child_index) else {
        return Err(reborrow_disposition_drift());
    };
    if child.child_weakening != child_weakening {
        return Err(reborrow_disposition_drift());
    }
    let Some(immediate_parent) = installation.get(child_index).copied() else {
        return Err(reborrow_disposition_drift());
    };
    let shared_cohort = if child.access_effect == CheckedReborrowAccessEffect::SharedRelease {
        vec![child_index]
    } else if child.access_effect == CheckedReborrowAccessEffect::SharedFreeze {
        let children = match snapshot.get(immediate_parent) {
            Some(EphemeralResourceStatus::BorrowedBy { children })
            | Some(EphemeralResourceStatus::RetiredWhileBorrowed { children, .. }) => children,
            _ => return Err(reborrow_disposition_drift()),
        };
        children
            .into_iter()
            .filter_map(|resource| match resource {
                ParentResourceIndex::Reborrow(index)
                    if reborrows[index].access_effect
                        == CheckedReborrowAccessEffect::SharedFreeze =>
                {
                    Some(index)
                }
                _ => None,
            })
            .collect()
    } else {
        Vec::new()
    };
    let release = if child.access_effect == CheckedReborrowAccessEffect::ExclusiveSuspension {
        CheckedReborrowResourceDisposition::BranchRelease
    } else {
        CheckedReborrowResourceDisposition::SharedRelease
    };
    let mut closed_child = ParentResourceIndex::Reborrow(child_index);
    let mut parent = immediate_parent;
    let mut retired_parent_path = Vec::new();
    let (disposition, final_target) = loop {
        let (mut children, parent_weakening) = match statuses.get(parent) {
            Some(EphemeralResourceStatus::BorrowedBy { children }) => (children, None),
            Some(EphemeralResourceStatus::RetiredWhileBorrowed {
                children,
                weakening,
            }) => (children, Some(weakening)),
            _ => return Err(reborrow_disposition_drift()),
        };
        let Some(child_position) = children
            .iter()
            .position(|resource| *resource == closed_child)
        else {
            return Err(reborrow_disposition_drift());
        };
        children.remove(child_position);
        if !children.is_empty() {
            let status = if let Some(weakening) = parent_weakening {
                EphemeralResourceStatus::RetiredWhileBorrowed {
                    children,
                    weakening,
                }
            } else {
                EphemeralResourceStatus::BorrowedBy { children }
            };
            if !statuses.set(parent, status) {
                return Err(reborrow_disposition_drift());
            }
            break (release, DispositionTargetIndex::ParentResource(parent));
        }
        let Some(weakening) = parent_weakening else {
            if !statuses.set(parent, EphemeralResourceStatus::Available) {
                return Err(reborrow_disposition_drift());
            }
            let disposition = if parent != immediate_parent {
                CheckedReborrowResourceDisposition::CascadeThroughRetiredParent
            } else {
                match child.access_effect {
                    CheckedReborrowAccessEffect::SharedRelease => {
                        CheckedReborrowResourceDisposition::SharedRelease
                    }
                    CheckedReborrowAccessEffect::SharedFreeze => {
                        CheckedReborrowResourceDisposition::RestoreSharedCohort
                    }
                    CheckedReborrowAccessEffect::ExclusiveSuspension => {
                        CheckedReborrowResourceDisposition::Reactivate
                    }
                }
            };
            break (disposition, DispositionTargetIndex::ParentResource(parent));
        };
        let retired_fact = flow.borrow_lifetimes.weakenings.get(weakening);
        let retired_boundary = weakening_event_boundary(retired_fact.source, retired_fact.reason)?;
        if retired_boundary > boundary || !statuses.set(parent, EphemeralResourceStatus::Retired) {
            return Err(reborrow_disposition_drift());
        }
        retired_parent_path.push((parent, weakening));
        match parent {
            ParentResourceIndex::Direct(index) => {
                let final_target = DispositionTargetIndex::DirectRootLifetime(index);
                let disposition = if boundary.phase == LifecyclePhase::StateExit {
                    CheckedReborrowResourceDisposition::StateExitDirectRootHandoff
                } else if retired_boundary == boundary {
                    CheckedReborrowResourceDisposition::SameBoundaryLineageClosure
                } else {
                    CheckedReborrowResourceDisposition::CascadeThroughRetiredParent
                };
                break (disposition, final_target);
            }
            ParentResourceIndex::Reborrow(index) => {
                closed_child = parent;
                parent = *installation
                    .get(index)
                    .ok_or_else(reborrow_disposition_drift)?;
            }
        }
    };
    Ok(CheckedReborrowDispositionEventDraft {
        machine_symbol: child.machine_symbol,
        state_symbol: child.state_symbol,
        child_loan: child.loan,
        child_resource: child_index,
        child_activation: child.child_activation,
        child_weakening,
        parent_loan: child.parent_loan,
        parent_resource: immediate_parent,
        boundary_source: child.weakening_source,
        boundary_phase: boundary.phase,
        shared_cohort,
        retired_parent_path,
        final_target,
        disposition,
        restored_call: Default::default(),
        restored_borrow_call: Default::default(),
        restored_call_access: Default::default(),
    })
}

fn exact_shared_cohort_observation(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    flow_state: &checked_trees::FlowStateFact,
    borrow_state: &checked_trees::StateBorrowFact,
    cohort: &[(usize, &CheckedReborrowLoanResourceDraft)],
    mutation_statement_index: usize,
) -> bool {
    if mutation_statement_index != cohort.len() + 2
        || !cohort.iter().enumerate().all(|(offset, (_, member))| {
            member.activation_source
                == FlowInvalidationSource::Statement {
                    statement_index: offset + 1,
                }
        })
    {
        return false;
    }
    let Some(observation_statement_index) = mutation_statement_index.checked_sub(1) else {
        return false;
    };
    let observation_calls = flow
        .control
        .calls
        .span_or_empty(flow_state.calls)
        .iter()
        .filter(|call| {
            call.statement_index == observation_statement_index && call.call_ordinal == 0
        })
        .collect::<Vec<_>>();
    let [observation] = observation_calls.as_slice() else {
        return false;
    };
    let Some(target_state) = program.state_by_symbol(observation.target_symbol) else {
        return false;
    };
    let Some(target_machine) = program.machines().iter().find(|machine| {
        typed_trees::state::state_by_symbol(program.machine_states(machine), target_state.symbol)
            .is_some()
    }) else {
        return false;
    };
    if target_machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !program
            .statement_table
            .statements(target_state.statement_nodes)
            .is_empty()
        || program.state_parameters(target_state).len() != cohort.len()
        || program
            .state_parameters(target_state)
            .iter()
            .any(|parameter| {
                !matches!(
                    program
                        .type_reference_table
                        .type_reference(parameter.type_reference),
                    typed_trees::types::TypeReferenceNode::Reference {
                        access: language_semantics::ReferenceAccess::Shared,
                        ..
                    }
                )
            })
        || (observation.has_receiver
            && target_machine.attached_data_symbol != observation.receiver_symbol)
    {
        return false;
    }
    let matching_borrow_calls = borrow
        .calls
        .span_or_empty(borrow_state.calls)
        .iter()
        .filter(|borrow_call| {
            borrow_call.statement_index == observation.statement_index
                && borrow_call.call_ordinal == observation.call_ordinal
                && borrow_call.target_symbol == observation.target_symbol
                && borrow_call.receiver_symbol == observation.receiver_symbol
                && borrow_call.has_receiver == observation.has_receiver
                && borrow_call.accesses == observation.accesses
        })
        .collect::<Vec<_>>();
    let [borrow_call] = matching_borrow_calls.as_slice() else {
        return false;
    };
    let accesses = borrow.argument_accesses.span_or_empty(borrow_call.accesses);
    matches!(cohort.len(), 2 | 3)
        && accesses.len() == cohort.len()
        && accesses.iter().zip(cohort).all(|(access, (_, member))| {
            access.root == facts::PlaceRoot::Symbol(member.owner_symbol)
                && borrow.access_segments(access).is_empty()
                && access.kind == checked_trees::BorrowAccessKind::Read
        })
}

/// Bind the checked post-restoration mutating call onto each qualifying
/// disposition event. A Reactivate or RestoreSharedCohort event already names
/// the restored parent; this pass additionally requires the boundary's exact
/// receiver-free, single-argument mutating call, the whole-root mutable access
/// onto the parent, the exact constraint roster, and the call's one mutated
/// place. Unsupported shapes stay unbound rather than receiving inferred
/// authority.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bind_restored_call_uses(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    flow: &checked_trees::FlowFacts,
    direct: &[CheckedDirectBorrowLoanResource],
    reborrows: &[CheckedReborrowLoanResourceDraft],
    dispositions: &mut [CheckedReborrowDispositionEventDraft],
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    for event in dispositions.iter_mut() {
        // The event's disposition selects the exact shape: Reactivate
        // requires an exclusive suspension, RestoreSharedCohort a bounded
        // shared-freeze cohort carrying every child of the parent.
        let restored = match event.disposition {
            CheckedReborrowResourceDisposition::Reactivate => true,
            CheckedReborrowResourceDisposition::RestoreSharedCohort => false,
            _ => continue,
        };
        let ParentResourceIndex::Direct(parent_index) = event.parent_resource else {
            continue;
        };
        let Some(parent) = direct.get(parent_index) else {
            return Err(reborrow_restored_call_use_drift());
        };
        let Some(child) = reborrows.get(event.child_resource) else {
            return Err(reborrow_restored_call_use_drift());
        };
        let exclusive_reactivation = matches!(
            child.access,
            checked_trees::BorrowAccessKind::Mutable | checked_trees::BorrowAccessKind::WriteOnly
        ) && child.access_effect
            == CheckedReborrowAccessEffect::ExclusiveSuspension;
        let shared_cohort = reborrows
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.parent_loan == parent.loan)
            .collect::<Vec<_>>();
        let bounded_shared_freeze = matches!(shared_cohort.len(), 1..=3)
            && shared_cohort.iter().all(|(_, member)| {
                member.access == checked_trees::BorrowAccessKind::Read
                    && member.access_effect == CheckedReborrowAccessEffect::SharedFreeze
                    && member.machine_symbol == child.machine_symbol
                    && member.state_symbol == child.state_symbol
                    && member.parent_lexical_status == ParentLexicalStatusAtChildEnd::LivePastChild
                    && member.weakening_reason
                        == checked_trees::FlowBorrowWeakeningReason::LastUseExpired
                    && member.weakening_source == child.weakening_source
                    && !reborrows
                        .iter()
                        .any(|candidate| candidate.parent_loan == member.loan)
            });
        if restored != exclusive_reactivation || !restored && !bounded_shared_freeze {
            continue;
        }
        if parent.loan != child.parent_loan
            || parent.machine_symbol != child.machine_symbol
            || parent.state_symbol != child.state_symbol
            || parent.access != checked_trees::BorrowAccessKind::Mutable
            || !parent.owner_path.is_empty()
            || (!exclusive_reactivation && !bounded_shared_freeze)
            || child.parent_lexical_status != ParentLexicalStatusAtChildEnd::LivePastChild
            || child.weakening_reason != checked_trees::FlowBorrowWeakeningReason::LastUseExpired
            || reborrows
                .iter()
                .any(|candidate| candidate.parent_loan == child.loan)
            || event.boundary_phase != CheckedBorrowResourceLifecyclePhase::LastUseExpired
            || event.boundary_source != child.weakening_source
            || event.parent_loan != parent.loan
            || !event.retired_parent_path.is_empty()
            || (restored && !event.shared_cohort.is_empty())
            || (!restored
                && event.shared_cohort
                    != shared_cohort
                        .iter()
                        .map(|(index, _)| *index)
                        .collect::<Vec<_>>())
            || event.final_target != DispositionTargetIndex::ParentResource(event.parent_resource)
        {
            continue;
        }
        let FlowInvalidationSource::Statement { statement_index } = child.weakening_source else {
            continue;
        };

        let Some(flow_state) = flow
            .control
            .unique_state(child.machine_symbol, child.state_symbol)
        else {
            continue;
        };
        let calls = flow
            .control
            .calls
            .span_or_empty(flow_state.calls)
            .iter()
            .enumerate()
            .filter(|(_, call)| call.statement_index == statement_index)
            .filter_map(|(offset, call)| {
                span_handle(flow_state.calls, offset).map(|handle| (handle, call))
            })
            .collect::<Vec<_>>();
        let [(call_handle, call)] = calls.as_slice() else {
            continue;
        };
        let Some(target_state) = program.state_by_symbol(call.target_symbol) else {
            continue;
        };
        let Some(target_machine) = program.machines().iter().find(|machine| {
            typed_trees::state::state_by_symbol(
                program.machine_states(machine),
                target_state.symbol,
            )
            .is_some()
        }) else {
            continue;
        };
        // A nominal type qualifier is source lookup, not a runtime receiver.
        // Keep true instance-receiver calls outside this binding.
        if call.has_receiver && target_machine.attached_data_symbol != call.receiver_symbol {
            continue;
        }

        let matching_borrow_states = borrow
            .states
            .iter()
            .filter(|(_, state)| {
                state.machine_symbol == child.machine_symbol
                    && state.state_symbol == child.state_symbol
            })
            .map(|(_, state)| state)
            .collect::<Vec<_>>();
        let [borrow_state] = matching_borrow_states.as_slice() else {
            continue;
        };
        if bounded_shared_freeze
            && shared_cohort.len() > 1
            && !exact_shared_cohort_observation(
                program,
                borrow,
                flow,
                flow_state,
                borrow_state,
                &shared_cohort,
                statement_index,
            )
        {
            continue;
        }
        let borrow_calls = borrow
            .calls
            .span_or_empty(borrow_state.calls)
            .iter()
            .enumerate()
            .filter(|(_, borrow_call)| {
                borrow_call.statement_index == call.statement_index
                    && borrow_call.call_ordinal == call.call_ordinal
                    && borrow_call.target_symbol == call.target_symbol
                    && borrow_call.receiver_symbol == call.receiver_symbol
                    && borrow_call.has_receiver == call.has_receiver
                    && borrow_call.accesses == call.accesses
            })
            .filter_map(|(offset, borrow_call)| {
                span_handle(borrow_state.calls, offset).map(|handle| (handle, borrow_call))
            })
            .collect::<Vec<_>>();
        let [(borrow_call_handle, borrow_call)] = borrow_calls.as_slice() else {
            continue;
        };

        let parameters = program
            .state_parameters(target_state)
            .iter()
            .collect::<Vec<_>>();
        let [parameter] = parameters.as_slice() else {
            continue;
        };
        if !parameter.is_mutable {
            continue;
        }
        let typed_trees::types::TypeReferenceNode::Reference {
            access: language_core::ReferenceAccess::Mutable,
            ..
        } = program
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            continue;
        };

        let accesses = borrow.argument_accesses.span_or_empty(call.accesses);
        let [access] = accesses else {
            continue;
        };
        let Some(access_handle) = span_handle(call.accesses, 0) else {
            continue;
        };
        if access.root != facts::PlaceRoot::Symbol(parent.owner_symbol)
            || !borrow.access_segments(access).is_empty()
            || access.kind != checked_trees::BorrowAccessKind::Mutable
            || access.end_before_access.is_some()
        {
            continue;
        }

        let constraints = flow
            .contexts
            .constraint_refs
            .span_or_empty(call.entry_constraints);
        let borrow_call_constraints = constraints
            .iter()
            .filter(|constraint| {
                matches!(
                    constraint.kind,
                    checked_trees::FlowConstraintKind::BorrowCall { .. }
                )
            })
            .collect::<Vec<_>>();
        let access_constraints = constraints
            .iter()
            .filter(|constraint| {
                matches!(
                    constraint.kind,
                    checked_trees::FlowConstraintKind::BorrowAccess { .. }
                )
            })
            .collect::<Vec<_>>();
        let parent_constraints = constraints
            .iter()
            .filter(|constraint| {
                constraint.kind
                    == checked_trees::FlowConstraintKind::BorrowLoan { loan: parent.loan }
            })
            .collect::<Vec<_>>();
        let child_constraint_count = constraints
            .iter()
            .filter(|constraint| {
                shared_cohort.iter().any(|(_, member)| {
                    constraint.kind
                        == checked_trees::FlowConstraintKind::BorrowLoan { loan: member.loan }
                })
            })
            .count();
        let [borrow_call_constraint] = borrow_call_constraints.as_slice() else {
            continue;
        };
        let [access_constraint] = access_constraints.as_slice() else {
            continue;
        };
        let [_parent_entry_constraint] = parent_constraints.as_slice() else {
            continue;
        };
        if borrow_call_constraint.kind
            != (checked_trees::FlowConstraintKind::BorrowCall {
                call: *borrow_call_handle,
            })
            || access_constraint.kind
                != (checked_trees::FlowConstraintKind::BorrowAccess {
                    access: access_handle,
                })
            || child_constraint_count != 0
        {
            continue;
        }

        let mutated_places = crate::flow::call_write_accesses(
            program,
            child.machine_symbol,
            child.state_symbol,
            borrow,
            borrow_call,
            mutation_summaries,
            None,
        );
        let [mutated_place] = mutated_places.as_slice() else {
            continue;
        };
        if mutated_place.root != facts::PlaceRoot::Symbol(parent.owner_symbol)
            || !mutated_place.segments.is_empty()
        {
            continue;
        }

        event.restored_call = *call_handle;
        event.restored_borrow_call = *borrow_call_handle;
        event.restored_call_access = access_handle;
    }
    Ok(())
}
