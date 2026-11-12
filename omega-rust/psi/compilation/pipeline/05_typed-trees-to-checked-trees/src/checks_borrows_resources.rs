/*
    Construct the resource rows consumed by execution planning and Psi 06.
    Formation and flow own loan ancestry and lifetime events. This owner joins
    those facts once, checks access attenuation, and builds the parent graph
    and disposition events required by lowering. Each disposition event whose
    boundary is a checked post-restoration mutating call carries that call's
    exact join. No second construction replays them.
*/

pub(in crate::checks::borrows) mod call_operands {
    /*
        Join a completed operand to its existing call access or stored loan.

        Call entry is too late to recover an earlier operand's descriptor contents.
        Resolve checked accesses in its completion constraints, preserving the original
        access occurrence and every retained ancestor. Constructed children use
        their own evaluations and resolved destination paths. Missing provenance
        produces no resource: carry must still establish complete leaf coverage.
        Lifetime retention keeps the complete enclosing owner, not a second copy
        of referent geometry; the evaluated actual already owns its captured address.
        A terminal region retains that exact footprint. It need not have one scalar
        type, but its collection element must have no unresolved permission carriers.
        These rows retain demands; they authorize no access, restoration or parking.
    */
    use super::permission_sources;
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;

    use crate::published::places::place_at;
    use arena::{Arena, Handle, HandleSpan};
    use checked_trees::{
        BorrowLoanFact, BorrowLoanOwnerSegment, CapturedPlace, CheckFacts,
        CheckedCallBorrowLoanResource, CheckedCallBorrowLoanSource,
        CheckedDirectBorrowParentLifetime, FlowCallFact, FlowOperandFact, FlowStateFact,
    };

    pub(super) struct ResourceDraft {
        pub(super) resource: CheckedCallBorrowLoanResource,
        pub(super) path: Vec<BorrowLoanOwnerSegment>,
        pub(super) backing: CapturedPlace,
    }

    pub(super) fn install(program: &dyn validation::CheckingInput, facts: &mut CheckFacts) {
        let prepared_loans = LoanOccurrences::new(program, facts);
        let loan_occurrences = &prepared_loans;
        let mut resources = Vec::new();
        let mut parents = Arena::new();
        for (_, state) in facts.flow.control.states.iter() {
            for call in state.calls.handles() {
                let row = facts.flow.control.calls.get(call);
                for operand in row.argument_operands().handles() {
                    collect(
                        program,
                        facts,
                        loan_occurrences,
                        state,
                        call,
                        operand,
                        operand,
                        &[],
                        &mut resources,
                        &mut parents,
                    );
                }
            }
        }
        drop(prepared_loans);
        facts.borrow.call_loan_resources.reset_retain_capacity();
        facts.borrow.call_loan_paths.reset_retain_capacity();
        facts.borrow.call_loan_backing_paths.reset_retain_capacity();
        facts.borrow.call_loan_parents = parents;
        for draft in resources {
            let _ = install_resource(program, facts, draft);
        }
    }

    /*
        Publish one captured permission without changing any already published
        handle. Completed output debts use the same original-source coordinates
        as ordinary call inputs, even when the actual's slot has since changed.
    */
    pub(super) fn install_resource(
        program: &dyn validation::CheckingInput,
        facts: &mut CheckFacts,
        mut draft: ResourceDraft,
    ) -> Option<Handle<CheckedCallBorrowLoanResource>> {
        if !facts
            .flow
            .control
            .operands
            .is_valid(draft.resource.owner_operand)
        {
            return None;
        }
        let owner = facts
            .flow
            .control
            .operands
            .get(draft.resource.owner_operand);
        let path = crate::borrow::carrier_path(program, owner.type_reference, &draft.path)?;
        if draft.backing.root != draft.resource.parent_lifetime.root {
            return None;
        }
        draft.resource.backing_incoming_reference = draft.backing.incoming_reference;
        draft.resource.backing_incoming_owner_path = facts
            .borrow
            .call_loan_backing_paths
            .insert_many(draft.backing.incoming_owner_path);
        draft.resource.backing_path = facts
            .borrow
            .call_loan_backing_paths
            .insert_many(draft.backing.segments);
        draft.resource.capture_path = facts.borrow.call_loan_paths.insert_many(path);
        draft.resource.owner_path = facts.borrow.owner_segments.insert_many(draft.path);
        Some(facts.borrow.call_loan_resources.insert(draft.resource))
    }

    fn collect(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        call: Handle<FlowCallFact>,
        owner_operand: Handle<FlowOperandFact>,
        operand: Handle<FlowOperandFact>,
        path: &[BorrowLoanOwnerSegment],
        resources: &mut Vec<ResourceDraft>,
        parents: &mut Arena<Handle<BorrowLoanFact>>,
    ) {
        if !facts.flow.control.operands.is_valid(operand) {
            return;
        }
        let capture = facts.flow.control.operands.get(operand);
        let Some(paths) =
            crate::borrow::view_link::borrow_carrying_owner_paths(program, capture.type_reference)
        else {
            return;
        };
        if paths.is_empty() {
            return;
        }
        if let checked_trees::FlowOperandCompletion::Selected(alternatives) = capture.completion {
            let Some(alternatives) = facts.flow.control.operand_alternatives.span(alternatives)
            else {
                return;
            };
            if alternatives.is_empty() {
                return;
            }
            let mut complete = Vec::new();
            for alternative in alternatives {
                if !alternative.source_arm.is_valid() {
                    return;
                }
                let mut branch = Vec::new();
                collect(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    call,
                    owner_operand,
                    alternative.value,
                    path,
                    &mut branch,
                    parents,
                );
                if !paths.iter().all(|leaf| {
                    branch
                        .iter()
                        .any(|draft| draft.path.strip_prefix(path) == Some(leaf.as_slice()))
                }) {
                    return;
                }
                complete.extend(branch);
            }
            resources.extend(complete);
            return;
        }
        if !capture.constructed_children().is_empty() {
            for child in facts
                .flow
                .control
                .operand_children
                .span_or_empty(capture.constructed_children())
            {
                let mut child_path = path.to_vec();
                child_path.extend(crate::borrow::owner_path_from_place_segments(
                    program,
                    facts
                        .flow
                        .control
                        .operand_child_paths
                        .span_or_empty(child.path),
                ));
                collect(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    call,
                    owner_operand,
                    child.value,
                    &child_path,
                    resources,
                    parents,
                );
            }
            return;
        }
        if let Some(reference) = validation::reference_shell(program, capture.type_reference) {
            let access = checked_trees::BorrowAccessKind::from(reference.access);
            collect_suppliers(
                program,
                facts,
                loan_occurrences,
                state,
                call,
                owner_operand,
                operand,
                operand,
                &[],
                path,
                &access,
                false,
                &mut Vec::new(),
                resources,
                parents,
            );
            return;
        }
        let reference_paths =
            validation::borrow_contracts::carried_reference_paths(program, capture.type_reference);
        for leaf in paths {
            let leaf_path = crate::borrow::owner_place_segments(&leaf);
            if let Some(access) = reference_paths.as_ref().and_then(|paths| {
                paths
                    .iter()
                    .find_map(|(path, access)| (path == &leaf).then_some(access))
            }) && let Some(selected) = validation::completed_storage::reference_captures_at_path(
                program,
                &facts.flow.control,
                operand,
                &leaf_path,
                capture.storage_snapshot,
            ) {
                if let Some(selected) = selected.captures() {
                    let mut owner_path = path.to_vec();
                    owner_path.extend(leaf.iter().copied());
                    let mut complete = Vec::new();
                    let mut resolved = !selected.is_empty();
                    for selected in selected {
                        if !collect_suppliers(
                            program,
                            facts,
                            loan_occurrences,
                            state,
                            call,
                            owner_operand,
                            operand,
                            selected.capture,
                            &selected.projection,
                            &owner_path,
                            access,
                            false,
                            &mut Vec::new(),
                            &mut complete,
                            parents,
                        ) {
                            resolved = false;
                            break;
                        }
                    }
                    if resolved {
                        resources.extend(complete);
                    }
                }
                continue;
            }
            let Some(source) = place_at(
                program,
                state.state_symbol,
                capture.statement_index,
                capture.expression,
            ) else {
                continue;
            };
            let mut source_path =
                crate::borrow::owner_path_from_place_segments(program, &source.segments);
            source_path.extend(leaf.iter().copied());
            for loan in facts
                .flow
                .borrow_loan_constraints(capture.constraints)
                .filter(|loan| {
                    let loan = facts.borrow.loans.get(*loan);
                    loan.storage_owner == source.root
                        && facts.borrow.loan_owner_path(loan) == source_path
                })
            {
                let row = facts.borrow.loans.get(loan);
                let mut owner_path = path.to_vec();
                owner_path.extend(leaf.iter().copied());
                resources.push(ResourceDraft {
                    resource: CheckedCallBorrowLoanResource {
                        machine_symbol: state.machine_symbol,
                        state_symbol: state.state_symbol,
                        call,
                        owner_operand,
                        operand,
                        owner_path: HandleSpan::empty(),
                        capture_path: HandleSpan::empty(),
                        source: CheckedCallBorrowLoanSource::StoredLoan { loan },
                        access: row.kind.clone(),
                        parent_lifetime: CheckedDirectBorrowParentLifetime {
                            machine_symbol: state.machine_symbol,
                            state_symbol: state.state_symbol,
                            root: row.root,
                        },
                        backing_incoming_reference: Handle::invalid(),
                        backing_incoming_owner_path: HandleSpan::empty(),
                        backing_path: HandleSpan::empty(),
                    },
                    path: owner_path,
                    backing: CapturedPlace {
                        incoming_reference: row.incoming_reference,
                        incoming_owner_path: facts
                            .borrow
                            .access_segments
                            .span_or_empty(row.incoming_owner_path)
                            .to_vec(),
                        root: row.root,
                        segments: facts.borrow.loan_segments(row).to_vec(),
                    },
                });
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_suppliers(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        call: Handle<FlowCallFact>,
        owner_operand: Handle<FlowOperandFact>,
        operand: Handle<FlowOperandFact>,
        source: Handle<FlowOperandFact>,
        projection: &[facts::PlaceSegment],
        path: &[BorrowLoanOwnerSegment],
        output_access: &checked_trees::BorrowAccessKind,
        retained_by_store: bool,
        active: &mut Vec<(Handle<FlowOperandFact>, Vec<facts::PlaceSegment>)>,
        output: &mut Vec<ResourceDraft>,
        parents: &mut Arena<Handle<BorrowLoanFact>>,
    ) -> bool {
        let mut suppliers = Vec::new();
        if !permission_sources::collect(
            program,
            facts,
            loan_occurrences,
            state,
            call,
            source,
            projection,
            output_access,
            retained_by_store,
            active,
            &mut suppliers,
            parents,
            &mut permission_sources::PermissionFailures::default(),
        ) {
            return false;
        }
        output.extend(suppliers.into_iter().map(|supplier| ResourceDraft {
            resource: CheckedCallBorrowLoanResource {
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                call,
                owner_operand,
                operand,
                source: supplier.source,
                access: output_access.clone(),
                parent_lifetime: CheckedDirectBorrowParentLifetime {
                    machine_symbol: state.machine_symbol,
                    state_symbol: state.state_symbol,
                    root: supplier.backing.root,
                },
                ..Default::default()
            },
            path: path.to_vec(),
            backing: supplier.backing,
        }));
        true
    }
}
mod disposition_events {
    /*
        Planning and resolving reborrow disposition events.
    */
    use crate::checks::borrows::resources::lifecycle::span_handle;
    use crate::checks::borrows::resources::lifecycle::{
        EphemeralResourceStatus as ResourceStatus, EphemeralStatuses, LifecycleBoundaryKey,
        LifecycleEvent, LifecycleEventKind, LifecyclePhase, activation_boundary,
        apply_activation_batch, apply_weakening_batch, exact_loan_lifecycle_rows,
        weakening_event_boundary,
    };
    use crate::checks::borrows::resources::reborrow_drafts::{
        CheckedReborrowDispositionEventDraft as EventDraft,
        CheckedReborrowLoanResourceDraft as ResourceDraft, DispositionTargetIndex as TargetIndex,
        ParentResourceIndex,
    };
    use crate::checks::borrows::resources::{
        reborrow_disposition_drift, reborrow_restored_call_use_drift,
    };
    use checked_trees::{
        BorrowFacts, CheckedBorrowResourceLifecyclePhase, CheckedDirectBorrowLoanResource,
        CheckedReborrowAccessEffect as AccessEffect,
        CheckedReborrowResourceDisposition as ResourceDisposition, FlowFacts,
        FlowInvalidationSource, ParentLexicalStatusAtChildEnd,
    };
    use diagnostics::Diagnostic;

    pub(crate) fn plan_reborrow_disposition_events(
        borrow: &BorrowFacts,
        flow: &FlowFacts,
        direct: &[CheckedDirectBorrowLoanResource],
        reborrows: &[ResourceDraft],
        installation: &[ParentResourceIndex],
    ) -> Result<Vec<EventDraft>, Vec<Diagnostic>> {
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
            crate::shared::collections::push_unique(&mut states, (machine, state));
        }

        for (_, resource) in borrow.supplied_output_resources.iter() {
            crate::shared::collections::push_unique(
                &mut states,
                (resource.machine_symbol, resource.state_symbol),
            );
        }
        let mut statuses = EphemeralStatuses::new(
            direct.len(),
            reborrows.len(),
            borrow.supplied_output_resources.len(),
        );
        let mut dispositions = Vec::new();
        for (machine_symbol, state_symbol) in states {
            let mut events = Vec::new();
            let Some(flow_state) = flow.control.state(machine_symbol, state_symbol) else {
                return Err(reborrow_disposition_drift());
            };
            for (index, resource) in direct.iter().enumerate().filter(|(_, resource)| {
                resource.machine_symbol == machine_symbol && resource.state_symbol == state_symbol
            }) {
                let Ok((activation, weakening)) =
                    exact_loan_lifecycle_rows(borrow, flow, flow_state, resource.loan)
                else {
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
                events.push(LifecycleEvent {
                    boundary: weakening_event_boundary(
                        resource.weakening_source,
                        resource.weakening_reason,
                    )?,
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
            for (index, (_, resource)) in borrow
                .supplied_output_resources
                .iter()
                .enumerate()
                .filter(|(_, (_, resource))| {
                    resource.machine_symbol == machine_symbol
                        && resource.state_symbol == state_symbol
                })
            {
                let actual = flow.control.calls.get(resource.call);
                events.push(LifecycleEvent {
                    boundary: super::lifecycle::normal_completion_boundary(actual.statement_index),
                    resource: ParentResourceIndex::Supplied(index),
                    loan_order: resource.loan.arena_index(),
                    kind: LifecycleEventKind::CompleteOutput {
                        call: resource.call,
                    },
                });
                if flow
                    .borrow_lifetimes
                    .weakenings
                    .is_valid(resource.holder_weakening)
                {
                    let end = flow
                        .borrow_lifetimes
                        .weakenings
                        .get(resource.holder_weakening);
                    events.push(LifecycleEvent {
                        boundary: weakening_event_boundary(end.source, end.reason)?,
                        resource: ParentResourceIndex::Supplied(index),
                        loan_order: resource.loan.arena_index(),
                        kind: LifecycleEventKind::Weaken {
                            weakening: resource.holder_weakening,
                        },
                    });
                }
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
                if matches!(
                    boundary.phase,
                    LifecyclePhase::Activation | LifecyclePhase::NormalCompletion
                ) {
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

    /*
        Close one leaf and then only those retired ancestors whose entire child
        roster is empty. A sibling release never restores whole-parent authority.
        The batch snapshot supplies the shared roster; evolving statuses decide
        closure, so simultaneous grandchildren cannot close a parent twice.
    */
    fn resolve_disposition_event(
        flow: &FlowFacts,
        child_index: usize,
        child_weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
        boundary: LifecycleBoundaryKey,
        statuses: &mut EphemeralStatuses,
        snapshot: &EphemeralStatuses,
        reborrows: &[ResourceDraft],
        installation: &[ParentResourceIndex],
    ) -> Result<EventDraft, Vec<Diagnostic>> {
        let Some(child) = reborrows.get(child_index) else {
            return Err(reborrow_disposition_drift());
        };
        if child.child_weakening != child_weakening {
            return Err(reborrow_disposition_drift());
        }
        let Some(immediate_parent) = installation.get(child_index).copied() else {
            return Err(reborrow_disposition_drift());
        };
        let shared_cohort = if child.access_effect == AccessEffect::SharedRelease {
            vec![child_index]
        } else if child.access_effect == AccessEffect::SharedFreeze {
            let children = match snapshot.get(immediate_parent) {
                Some(ResourceStatus::BorrowedBy { children })
                | Some(ResourceStatus::RetiredWhileBorrowed { children, .. }) => children,
                _ => return Err(reborrow_disposition_drift()),
            };
            children
                .into_iter()
                .filter_map(|resource| match resource {
                    ParentResourceIndex::Reborrow(index)
                        if reborrows[index].access_effect == AccessEffect::SharedFreeze =>
                    {
                        Some(index)
                    }
                    _ => None,
                })
                .collect()
        } else {
            Vec::new()
        };
        let release = if child.access_effect == AccessEffect::ExclusiveSuspension {
            ResourceDisposition::BranchRelease
        } else {
            ResourceDisposition::SharedRelease
        };
        let mut closed_child = ParentResourceIndex::Reborrow(child_index);
        let mut parent = immediate_parent;
        let mut retired_parent_path = Vec::new();
        let (disposition, final_target) = loop {
            let (mut children, parent_weakening) = match statuses.get(parent) {
                Some(ResourceStatus::BorrowedBy { children }) => (children, None),
                Some(ResourceStatus::RetiredWhileBorrowed {
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
                    ResourceStatus::RetiredWhileBorrowed {
                        children,
                        weakening,
                    }
                } else {
                    ResourceStatus::BorrowedBy { children }
                };
                if !statuses.set(parent, status) {
                    return Err(reborrow_disposition_drift());
                }
                break (release, TargetIndex::ParentResource(parent));
            }
            let Some(weakening) = parent_weakening else {
                if !statuses.set(parent, ResourceStatus::Available) {
                    return Err(reborrow_disposition_drift());
                }
                let disposition = if parent != immediate_parent {
                    ResourceDisposition::CascadeThroughRetiredParent
                } else {
                    match child.access_effect {
                        AccessEffect::SharedRelease => ResourceDisposition::SharedRelease,
                        AccessEffect::SharedFreeze => ResourceDisposition::RestoreSharedCohort,
                        AccessEffect::ExclusiveSuspension => ResourceDisposition::Reactivate,
                    }
                };
                break (disposition, TargetIndex::ParentResource(parent));
            };
            let retired_fact = flow.borrow_lifetimes.weakenings.get(weakening);
            let retired_boundary =
                weakening_event_boundary(retired_fact.source, retired_fact.reason)?;
            if retired_boundary > boundary || !statuses.set(parent, ResourceStatus::Retired) {
                return Err(reborrow_disposition_drift());
            }
            retired_parent_path.push((parent, weakening));
            match parent {
                ParentResourceIndex::Direct(index) => {
                    let final_target = TargetIndex::DirectRootLifetime(index);
                    let disposition = if boundary.phase == LifecyclePhase::StateExit {
                        ResourceDisposition::StateExitDirectRootHandoff
                    } else if retired_boundary == boundary {
                        ResourceDisposition::SameBoundaryLineageClosure
                    } else {
                        ResourceDisposition::CascadeThroughRetiredParent
                    };
                    break (disposition, final_target);
                }
                ParentResourceIndex::Reborrow(index) => {
                    closed_child = parent;
                    parent = *installation
                        .get(index)
                        .ok_or_else(reborrow_disposition_drift)?;
                }
                ParentResourceIndex::Supplied(index) => {
                    break (
                        ResourceDisposition::CascadeThroughRetiredParent,
                        TargetIndex::CompletedOutput(index),
                    );
                }
            }
        };
        Ok(EventDraft {
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
        cohort: &[(usize, &ResourceDraft)],
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
        let observation_calls = crate::flow::call_flows(&flow.control, flow_state.calls)
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
        let Some(target_machine) = program.machine_containing_state(target_state.symbol) else {
            return false;
        };
        if target_machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || !crate::flow::state_statements(program, target_state.statement_nodes).is_empty()
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

    /*
        Bind the checked post-restoration mutating call onto each qualifying
        disposition event. A Reactivate or RestoreSharedCohort event already names
        the restored parent; this pass additionally requires the boundary's exact
        receiver-free, single-argument mutating call, the whole-root mutable access
        onto the parent, the exact constraint roster, and the call's one mutated
        place. Unsupported shapes stay unbound rather than receiving inferred
        authority.
    */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn bind_restored_call_uses(
        program: &dyn ::validation::CheckingInput,
        borrow: &checked_trees::BorrowFacts,
        flow: &checked_trees::FlowFacts,
        direct: &[CheckedDirectBorrowLoanResource],
        reborrows: &[ResourceDraft],
        dispositions: &mut [EventDraft],
        mutation_summaries: &crate::flow::StateMutationSummaryCache,
    ) -> Result<(), Vec<Diagnostic>> {
        for event in dispositions.iter_mut() {
            let restored = match event.disposition {
                ResourceDisposition::Reactivate => true,
                ResourceDisposition::RestoreSharedCohort => false,
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
                checked_trees::BorrowAccessKind::Mutable
                    | checked_trees::BorrowAccessKind::WriteOnly
            ) && child.access_effect
                == AccessEffect::ExclusiveSuspension;
            let shared_cohort = reborrows
                .iter()
                .enumerate()
                .filter(|(_, candidate)| candidate.parent_loan == parent.loan)
                .collect::<Vec<_>>();
            let bounded_shared_freeze = matches!(shared_cohort.len(), 1..=3)
                && shared_cohort.iter().all(|(_, member)| {
                    member.access == checked_trees::BorrowAccessKind::Read
                        && member.access_effect == AccessEffect::SharedFreeze
                        && member.machine_symbol == child.machine_symbol
                        && member.state_symbol == child.state_symbol
                        && member.parent_lexical_status
                            == ParentLexicalStatusAtChildEnd::LivePastChild
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
                || child.weakening_reason
                    != checked_trees::FlowBorrowWeakeningReason::LastUseExpired
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
                || event.final_target != TargetIndex::ParentResource(event.parent_resource)
            {
                continue;
            }
            let FlowInvalidationSource::Statement { statement_index } = child.weakening_source
            else {
                continue;
            };

            let Some(flow_state) = flow
                .control
                .unique_state(child.machine_symbol, child.state_symbol)
            else {
                continue;
            };
            let calls = crate::flow::call_flows(&flow.control, flow_state.calls)
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
            let Some(target_machine) = program.machine_containing_state(target_state.symbol) else {
                continue;
            };
            if call.has_receiver && target_machine.attached_data_symbol != call.receiver_symbol {
                continue;
            }

            let Some(borrow_state) =
                borrow.unique_state_borrow(child.machine_symbol, child.state_symbol)
            else {
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
}
mod formation_sources {
    /*
        When a new view is made from a copied reference, check the reference that
        was actually read. Its old slot may now contain a different reference.
        Every selected supplier must retain its established borrowing parent and
        presented access; neither the old slot nor equal backing authorizes a view.
    */
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    mod returned_projections {
        /*
            A helper's field view must keep the reference that actually supplied it.
            The checked normal-return relation identifies the completed source; its
            borrowing permission, presentation and lifetime remain separate obligations.
        */
        use super::super::super::formation_captures::formation_capture;
        use super::{nearest_parents, refusal};
        use crate::checks::borrows::loan_occurrences::LoanOccurrences;
        use arena::Handle;
        use checked_trees::{
            BorrowLoanFact, BorrowLoanFormation, BorrowLoanLineage, CheckFacts, FlowOperandFact,
        };
        use diagnostics::Diagnostic;

        pub(super) fn validate(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &checked_trees::StateBorrowFact,
            flow_state: &checked_trees::FlowStateFact,
            loan: Handle<BorrowLoanFact>,
            row: &BorrowLoanFact,
        ) -> Result<(), Vec<Diagnostic>> {
            let (activation, _) = super::super::lifecycle::exact_loan_lifecycle_rows(
                &facts.borrow,
                &facts.flow,
                flow_state,
                loan,
            )
            .map_err(|error| vec![error])?;
            let activation = facts.flow.borrow_lifetimes.activations.get(activation);
            let capture =
                formation_capture(facts, activation.formation_value, row.formation_expression)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "returned view lost its completed invocation capture",
                        )]
                    })?;
            let value = facts.flow.control.operands.get(capture);
            let path =
                crate::borrow::owner_place_segments(facts.borrow.owner_segments.span_or_empty(
                    row.formation_owner_path.ok_or_else(|| {
                        vec![Diagnostic::error("returned view lost its result path")]
                    })?,
                ));
            let profiles = validation::completed_storage::captured_retention_profiles_at_path(
                program,
                &facts.flow.control,
                value,
                &path,
            )
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "returned view has no complete formation frontier",
                )]
            })?;
            let mut pending = profiles
                .into_iter()
                .map(|profile| (profile, false))
                .collect::<Vec<_>>();
            let mut active = Vec::new();
            let mut completed = Vec::new();
            let mut selected = false;
            while let Some((profile, closing)) = pending.pop() {
                if closing {
                    active.pop();
                    completed.push(profile);
                    continue;
                }
                if active.contains(&profile) {
                    return refusal("returned view has a cyclic formation alternative");
                }
                if completed.contains(&profile) {
                    continue;
                }
                if !facts.flow.control.operand_retentions.is_valid(profile) {
                    return refusal("returned view has an unavailable formation alternative");
                }
                active.push(profile);
                pending.push((profile, true));
                let profile = facts.flow.control.operand_retentions.get(profile);
                if facts.flow.control.operands.is_valid(profile.descriptor) {
                    let descriptor = facts.flow.control.operands.get(profile.descriptor);
                    let checked_trees::FlowOperandCompletion::InvocationResult {
                        invocation,
                        path: result_path,
                        ..
                    } = descriptor.completion
                    else {
                        return refusal(
                            "returned projection is not bound to a completed invocation",
                        );
                    };
                    if !facts.flow.control.calls.is_valid(invocation)
                        || !flow_state.calls.handles().any(|call| call == invocation)
                        || descriptor.state_symbol != state.state_symbol
                        || descriptor.statement_index != row.statement_index
                        || !facts
                            .flow
                            .control
                            .operand_retentions
                            .is_valid(descriptor.retention)
                    {
                        return refusal("returned projection has no current invocation identity");
                    }
                    let call = facts.flow.control.calls.get(invocation);
                    if call.authored_expression != row.formation_expression
                        || call.statement_index != row.statement_index
                        || facts
                            .flow
                            .control
                            .operand_retained_paths
                            .span_or_empty(result_path)
                            != path
                    {
                        return refusal(
                            "returned projection does not match this invocation and result path",
                        );
                    }
                    let descriptor_profile = facts
                        .flow
                        .control
                        .operand_retentions
                        .get(descriptor.retention);
                    if !descriptor_profile.alternatives.is_empty() {
                        pending.extend(
                            descriptor_profile
                                .alternatives
                                .handles()
                                .map(|profile| (profile, false)),
                        );
                        continue;
                    }
                    if descriptor_profile.reborrow_sources.is_empty() {
                        if descriptor_profile.immutable_image {
                            continue;
                        }
                        let returned_state =
                            program.state_by_symbol(state.state_symbol).ok_or_else(|| {
                                vec![Diagnostic::error(
                                    "returned projection has no checked state",
                                )]
                            })?;
                        let sources =
                            validation::reference_result_custody::invocation_borrow_sources(
                                program,
                                facts,
                                returned_state,
                                descriptor,
                            )
                            .ok_or_else(|| {
                                vec![Diagnostic::error(
                                    "returned projection lost its actual borrowing source",
                                )]
                            })?;
                        for source in sources {
                            selected |= validate_captured_source(
                                program,
                                facts,
                                loan_occurrences,
                                state,
                                flow_state,
                                invocation,
                                row,
                                CapturedBorrowSource {
                                    operand: source.operand,
                                    source_path: &source.source_path,
                                    projection: &source.projection,
                                    extent: source.extent,
                                },
                            )?;
                        }
                        continue;
                    }
                    for source in facts
                        .flow
                        .control
                        .operand_reborrow_sources
                        .span_or_empty(descriptor_profile.reborrow_sources)
                    {
                        selected |= validate_source(
                            program,
                            facts,
                            loan_occurrences,
                            state,
                            flow_state,
                            invocation,
                            row,
                            source,
                        )?;
                    }
                } else if !profile.alternatives.is_empty() {
                    pending.extend(
                        profile
                            .alternatives
                            .handles()
                            .map(|profile| (profile, false)),
                    );
                } else {
                    return refusal(&format!(
                        "returned projection has no checked descriptor formation: machine {:?}, state {:?} ({:?}), call target {:?}, statement {}, source {:?}, captured completion {:?}, selected path {path:?}, retained source {profile:?}",
                        program.symbols.name(state.machine_symbol),
                        program.symbols.name(state.state_symbol),
                        state.state_symbol,
                        program
                            .expression_table
                            .call_node(row.formation_expression)
                            .map(|call| program.symbols.name(call.target_symbol)),
                        row.statement_index,
                        row.formation_expression,
                        value.completion,
                    ));
                }
            }
            if !selected {
                return refusal("returned projection has no matching actual borrowing parent");
            }
            Ok(())
        }

        struct CapturedBorrowSource<'source> {
            operand: Handle<FlowOperandFact>,
            source_path: &'source [facts::PlaceSegment],
            projection: &'source [facts::PlaceSegment],
            extent: semantic_vocabulary::ReferenceOriginExtent,
        }

        fn validate_source(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &checked_trees::StateBorrowFact,
            flow_state: &checked_trees::FlowStateFact,
            call: Handle<checked_trees::FlowCallFact>,
            row: &BorrowLoanFact,
            source: &checked_trees::FlowOperandReborrowSource,
        ) -> Result<bool, Vec<Diagnostic>> {
            validate_captured_source(
                program,
                facts,
                loan_occurrences,
                state,
                flow_state,
                call,
                row,
                CapturedBorrowSource {
                    operand: source.operand,
                    source_path: facts
                        .flow
                        .control
                        .operand_retained_paths
                        .span_or_empty(source.source_path),
                    projection: facts
                        .flow
                        .control
                        .operand_retained_paths
                        .span_or_empty(source.projection),
                    extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                },
            )
        }

        fn validate_captured_source(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &checked_trees::StateBorrowFact,
            flow_state: &checked_trees::FlowStateFact,
            call: Handle<checked_trees::FlowCallFact>,
            row: &BorrowLoanFact,
            source: CapturedBorrowSource<'_>,
        ) -> Result<bool, Vec<Diagnostic>> {
            let CapturedBorrowSource {
                operand,
                source_path,
                projection,
                extent,
            } = source;
            let captured: &FlowOperandFact = facts.flow.control.operands.get(operand);
            let reference = validation::project_type_reference_from_segments(
                program,
                captured.type_reference,
                source_path,
            )
            .and_then(|reference| validation::reference_shell(program, reference))
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "returned projection source is not a captured reference",
                )]
            })?;
            let access = checked_trees::BorrowAccessKind::from(reference.access);
            if projection.is_empty() || access.direct_reborrow_effect(&row.kind).is_none() {
                return refusal("returned projection exceeds its source presentation");
            }
            let (suppliers, parent_rows) = super::super::permission_sources::lending_sources(
                program,
                facts,
                loan_occurrences,
                flow_state,
                call,
                operand,
                source_path,
                &access,
            )
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "returned projection source permission is unavailable: {}",
                    reason.describe(program)
                ))]
            })?;
            let mut selected = false;
            for supplier in suppliers {
                let mut place = supplier.backing;
                place.segments.extend_from_slice(projection);
                let Some(place) = physical_place(place) else {
                    return refusal("returned projection has no complete source projection");
                };
                let matches_place = |candidate: &checked_trees::CapturedPlace| {
                    // A returned view can retain its enclosing collection. The output
                    // must remain beneath that same captured backing.
                    if extent == semantic_vocabulary::ReferenceOriginExtent::WithinRegion {
                        candidate.incoming_reference == place.incoming_reference
                            && candidate.incoming_owner_path == place.incoming_owner_path
                            && candidate.root == place.root
                            && candidate.segments.starts_with(&place.segments)
                    } else {
                        candidate == &place
                    }
                };
                let parents = match supplier.source {
                    checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                        if !facts
                            .flow
                            .borrow_loan_constraints(captured.constraints)
                            .any(|current| current == loan)
                        {
                            return refusal(
                                "returned projection source was not available at capture",
                            );
                        }
                        nearest_parents(facts, loan_occurrences, state, loan)?
                    }
                    checked_trees::CheckedCallBorrowLoanSource::CallAccess {
                        access: argument,
                        parents,
                    } => {
                        let argument = facts.borrow.argument_accesses.get(argument);
                        if argument.expression != captured.expression
                            || argument.kind.direct_reborrow_effect(&row.kind).is_none()
                        {
                            return refusal(
                                "returned projection has no matching explicit argument borrow",
                            );
                        }
                        let mut roots = Vec::new();
                        for parent in parent_rows.span_or_empty(parents) {
                            for parent in nearest_parents(facts, loan_occurrences, state, *parent)?
                            {
                                crate::shared::collections::push_unique(&mut roots, parent);
                            }
                        }
                        let mut nearest = Vec::new();
                        for candidate in &roots {
                            let ancestor =
                                roots
                                    .iter()
                                    .filter(|other| *other != candidate)
                                    .any(|other| {
                                        crate::checks::borrows::aliases::retained_lineage(
                                            facts.view(),
                                            loan_occurrences,
                                            state,
                                            *other,
                                        )
                                        .is_some_and(|lineage| lineage.contains(candidate))
                                    });
                            if !ancestor {
                                nearest.push(*candidate);
                            }
                        }
                        nearest
                    }
                };
                let represented = facts
                    .borrow
                    .state_loans(state)
                    .filter(|(_, candidate)| {
                        candidate.formation == BorrowLoanFormation::ReturnedProjection
                            && candidate.formation_expression == row.formation_expression
                            && candidate.statement_index == row.statement_index
                            && candidate.storage_owner == row.storage_owner
                            && facts.borrow.loan_owner_path(candidate)
                                == facts.borrow.loan_owner_path(row)
                            && captured_place(&facts.borrow, candidate)
                                .as_ref()
                                .is_some_and(matches_place)
                    })
                    .collect::<Vec<_>>();
                if parents.is_empty() {
                    if !represented
                        .iter()
                        .any(|(_, candidate)| candidate.lineage == BorrowLoanLineage::DirectRoot)
                    {
                        return refusal("returned projection dropped its explicit argument borrow");
                    }
                } else if !parents.iter().all(|parent| {
                    represented.iter().any(|(_, candidate)| {
                        candidate.lineage
                            == BorrowLoanLineage::Reborrow {
                                parent_loan: *parent,
                            }
                    })
                }) {
                    return refusal("returned view did not retain every input borrow it needs");
                }
                selected |= captured_place(&facts.borrow, row)
                    .as_ref()
                    .is_some_and(matches_place)
                    && match row.lineage {
                        BorrowLoanLineage::DirectRoot => parents.is_empty(),
                        BorrowLoanLineage::Reborrow { parent_loan } => {
                            parents.contains(&parent_loan)
                        }
                        _ => false,
                    };
            }
            Ok(selected)
        }

        fn captured_place(
            borrow: &checked_trees::BorrowFacts,
            loan: &BorrowLoanFact,
        ) -> Option<checked_trees::CapturedPlace> {
            physical_place(checked_trees::CapturedPlace {
                incoming_reference: loan.incoming_reference,
                incoming_owner_path: borrow
                    .access_segments
                    .span_or_empty(loan.incoming_owner_path)
                    .to_vec(),
                root: loan.root,
                segments: borrow.loan_segments(loan).to_vec(),
            })
        }

        /*
            Compare footprints in the original collection dimension. Stored loan rows
            also retain the full input prefix used to suspend outside access; composing
            a window for this comparison must not rewrite that independent debt.
            Equal opaque footprints supply no pointer identity or permission: the exact
            invocation, output path and borrowing-parent checks above still own the join.
        */
        fn physical_place(
            mut place: checked_trees::CapturedPlace,
        ) -> Option<checked_trees::CapturedPlace> {
            crate::flow::compose_captured_view_segments(&mut place.segments)?;
            Some(place)
        }
    }

    use super::super::formation_captures::{descriptor_capture, formation_capture};

    use arena::Handle;
    use checked_trees::{BorrowLoanFact, BorrowLoanLineage, CheckFacts};
    use diagnostics::Diagnostic;

    pub(super) fn validate(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
    ) -> Result<(), Vec<Diagnostic>> {
        let prepared_loans = LoanOccurrences::new(program, facts);
        let loan_occurrences = &prepared_loans;
        for (_, state) in facts.borrow.states.iter() {
            let Some(flow_state) = facts
                .flow
                .control
                .state(state.machine_symbol, state.state_symbol)
            else {
                continue;
            };
            for (loan, row) in facts.borrow.state_loans(state) {
                if row.formation == checked_trees::BorrowLoanFormation::ReturnedProjection {
                    returned_projections::validate(
                        program,
                        facts,
                        loan_occurrences,
                        state,
                        flow_state,
                        loan,
                        row,
                    )?;
                    continue;
                }
                let BorrowLoanLineage::Reborrow { parent_loan } = row.lineage else {
                    continue;
                };
                let suppliers = facts
                    .borrow
                    .loan_suppliers
                    .span_or_empty(row.supplying_loans);
                let [source_loan] = suppliers else {
                    if suppliers.is_empty() {
                        continue;
                    }
                    return refusal("view formation has no exact source descriptor occurrence");
                };
                let (activation, _) = super::lifecycle::exact_loan_lifecycle_rows(
                    &facts.borrow,
                    &facts.flow,
                    flow_state,
                    loan,
                )
                .map_err(|error| vec![error])?;
                let activation = facts.flow.borrow_lifetimes.activations.get(activation);
                let formation =
                    validation::reference_formation_source(program, row.formation_expression)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "forwarded borrow has no checked view formation",
                            )]
                        })?;
                let capture =
                    formation_capture(facts, activation.formation_value, formation.source)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "view formation lost its exact evaluated source",
                            )]
                        })?;
                let capture = descriptor_capture(program, facts, capture).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "view source lost its captured reference boundary",
                    )]
                })?;
                let captured = facts.flow.control.operands.get(capture);
                let Some(source_type) =
                    validation::reference_shell(program, captured.type_reference)
                else {
                    return refusal("forwarded view source is not a captured reference descriptor");
                };
                let source_access = checked_trees::BorrowAccessKind::from(source_type.access);
                if source_access.direct_reborrow_effect(&row.kind).is_none() {
                    return refusal(
                        "view formation exceeds the saved descriptor's presented access",
                    );
                }
                if !facts
                    .flow
                    .borrow_loan_constraints(captured.constraints)
                    .any(|loan| loan == *source_loan)
                {
                    return refusal("view formation's source descriptor is not current at capture");
                }
                let mut selected = Vec::new();
                if !super::permission_sources::collect(
                    program,
                    facts,
                    loan_occurrences,
                    flow_state,
                    Handle::invalid(),
                    capture,
                    &[],
                    &source_access,
                    false,
                    &mut Vec::new(),
                    &mut selected,
                    &mut arena::Arena::new(),
                    &mut super::permission_sources::PermissionFailures::default(),
                ) {
                    return refusal(
                        "view formation has incomplete permission suppliers at its captured source",
                    );
                }
                let mut parents = Vec::new();
                for source in selected {
                    let checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } =
                        source.source
                    else {
                        return refusal(
                            "view formation requires an established borrowing occurrence",
                        );
                    };
                    for parent in nearest_parents(facts, loan_occurrences, state, loan)? {
                        crate::shared::collections::push_unique(&mut parents, parent);
                    }
                }
                if !parents.contains(&parent_loan)
                    || !parents.iter().all(|parent| {
                        facts.borrow.state_loans(state).any(|(_, candidate)| {
                            candidate.formation_expression == row.formation_expression
                                && candidate.statement_index == row.statement_index
                                && candidate.storage_owner == row.storage_owner
                                && facts.borrow.loan_owner_path(candidate)
                                    == facts.borrow.loan_owner_path(row)
                                && candidate.lineage
                                    == (BorrowLoanLineage::Reborrow {
                                        parent_loan: *parent,
                                    })
                        })
                    })
                {
                    return refusal(
                        "view formation does not retain every captured borrowing parent",
                    );
                }
            }
        }
        Ok(())
    }

    fn nearest_parents(
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &checked_trees::StateBorrowFact,
        source: Handle<BorrowLoanFact>,
    ) -> Result<Vec<Handle<BorrowLoanFact>>, Vec<Diagnostic>> {
        let borrow = &facts.borrow;
        if crate::checks::borrows::aliases::retained_lineage(
            facts.view(),
            loan_occurrences,
            state,
            source,
        )
        .is_none()
        {
            return refusal("captured descriptor has incomplete or cyclic borrowing ancestry");
        }
        let mut pending = vec![source];
        let mut visited = Vec::new();
        let mut parents = Vec::new();
        while let Some(loan) = pending.pop() {
            if visited.contains(&loan) {
                continue;
            }
            if !borrow.state_owns_loan(state, loan) {
                return refusal("captured descriptor supplier has no owning borrowing state");
            }
            visited.push(loan);
            match borrow.loans.get(loan).lineage {
                BorrowLoanLineage::DirectRoot
                | BorrowLoanLineage::Reborrow { .. }
                | BorrowLoanLineage::SuppliedOutput => parents.push(loan),
                BorrowLoanLineage::UnretainedDerived => {
                    let Some(suppliers) =
                        crate::checks::borrows::transport_suppliers(borrow, state, loan)
                    else {
                        return refusal(
                            "captured descriptor has no authenticated permission transfer",
                        );
                    };
                    pending.extend(suppliers.iter().copied());
                }
                BorrowLoanLineage::LifetimeDependency => {
                    return refusal("captured descriptor has no local borrowing resource");
                }
            }
        }
        if parents.is_empty() {
            return refusal("captured descriptor has no complete borrowing ancestry");
        }
        Ok(parents)
    }

    fn refusal<T>(message: &str) -> Result<T, Vec<Diagnostic>> {
        Err(vec![Diagnostic::error(message)])
    }
}
mod forwarded_lifetimes {
    /*
        Replacing a reference slot must not end access held by an earlier copy.
        Keep the original borrow held until all surviving copies finish. Lexical
        slot retirement remains separate from the final holder's resource closure.
    */
    use arena::Handle;
    use checked_trees::{
        BorrowLoanFact, CheckFacts, CheckedBorrowResourceRetention, FlowBorrowWeakeningFact,
    };
    use diagnostics::Diagnostic;

    use super::lifecycle::{
        activation_boundary, exact_loan_lifecycle_rows, weakening_event_boundary,
    };

    #[derive(Default)]
    struct TransferHolders {
        loan: Handle<BorrowLoanFact>,
        holders: Vec<Handle<BorrowLoanFact>>,
    }

    #[derive(Default)]
    pub(super) struct LoanRetention {
        pub(super) loan: Handle<BorrowLoanFact>,
        pub(super) retention: CheckedBorrowResourceRetention,
    }

    /*
        Resolve each complete holder closure before installing resource lifetimes.
        A holder must capture its supplier before that descriptor's lexical death;
        a later view cannot retrospectively establish its own parent's permission.
    */
    pub(super) fn plan(facts: &mut CheckFacts) -> Result<Vec<LoanRetention>, Vec<Diagnostic>> {
        let mut rows = Vec::new();
        let mut holders_arena = arena::Arena::new();
        let mut transfers: Vec<TransferHolders> = Vec::new();
        for (_, state) in facts.borrow.states.iter() {
            for (holder, _) in facts.borrow.state_loans(state) {
                let row = facts.borrow.loans.get(holder);
                let suppliers =
                    crate::checks::borrows::transport_suppliers(&facts.borrow, state, holder)
                        .or_else(|| {
                            let sources = facts.borrow.loan_suppliers.span(row.supplying_loans)?;
                            (row.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency
                                && !sources.is_empty()
                                && sources.iter().all(|source| {
                                    facts.borrow.state_owns_loan(state, *source)
                                        && facts.borrow.loans.get(*source).lineage
                                            == checked_trees::BorrowLoanLineage::LifetimeDependency
                                        && facts.borrow.loans.get(*source).owner_symbol
                                            == row.source_owner_symbol
                                        && facts.borrow.loans.get(*source).kind == row.kind
                                }))
                            .then_some(sources)
                        });
                let Some(suppliers) = suppliers else {
                    continue;
                };
                for supplier in suppliers {
                    let position = supplier.arena_index() as usize;
                    if transfers.len() <= position {
                        transfers.resize_with(position + 1, TransferHolders::default);
                    }
                    let entry = &mut transfers[position];
                    entry.loan = *supplier;
                    crate::shared::collections::push_unique(&mut entry.holders, holder);
                }
            }
        }
        for (_, held) in facts.borrow.stored_call_inputs.iter() {
            for input in facts
                .borrow
                .stored_call_input_resources
                .span_or_empty(held.resources)
            {
                let resource = facts.borrow.call_loan_resources.get(*input);
                let parents = match resource.source {
                    checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => vec![loan],
                    checked_trees::CheckedCallBorrowLoanSource::CallAccess { parents, .. } => facts
                        .borrow
                        .call_loan_parents
                        .span_or_empty(parents)
                        .to_vec(),
                };
                for source in parents {
                    let position = source.arena_index() as usize;
                    if transfers.len() <= position {
                        transfers.resize_with(position + 1, TransferHolders::default);
                    }
                    let entry = &mut transfers[position];
                    entry.loan = source;
                    crate::shared::collections::push_unique(&mut entry.holders, held.loan);
                }
            }
        }
        let descendants = close_output_descendants(facts, &transfers)?;
        for (_, output) in facts.borrow.supplied_output_resources.iter() {
            for input in facts
                .borrow
                .supplied_output_inputs
                .span_or_empty(output.inputs)
            {
                let checked_trees::CheckedOutputBorrowInputPermission::Borrowed { resource } =
                    input.permission
                else {
                    continue;
                };
                let input = facts.borrow.call_loan_resources.get(resource);
                let mut sources = Vec::new();
                match input.source {
                    checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => {
                        sources.push(loan)
                    }
                    checked_trees::CheckedCallBorrowLoanSource::CallAccess { parents, .. } => {
                        sources.extend_from_slice(
                            facts.borrow.call_loan_parents.span_or_empty(parents),
                        );
                    }
                }
                for source in sources {
                    let position = source.arena_index() as usize;
                    if transfers.len() <= position {
                        transfers.resize_with(position + 1, TransferHolders::default);
                    }
                    let entry = &mut transfers[position];
                    entry.loan = source;
                    for debt in facts.borrow.loan_suppliers.span_or_empty(output.debts) {
                        crate::shared::collections::push_unique(&mut entry.holders, *debt);
                    }
                }
            }
        }
        for (_, state) in facts.borrow.states.iter() {
            let Some(flow_state) = facts
                .flow
                .control
                .state(state.machine_symbol, state.state_symbol)
            else {
                return Err(vec![Diagnostic::error(
                    "retained borrow has no flow-state owner",
                )]);
            };
            for (loan, row) in facts.borrow.state_loans(state) {
                if !matches!(
                    row.lineage,
                    checked_trees::BorrowLoanLineage::DirectRoot
                        | checked_trees::BorrowLoanLineage::Reborrow { .. }
                        | checked_trees::BorrowLoanLineage::SuppliedOutput
                ) {
                    continue;
                }
                let lexical = lexical_end(facts, flow_state, loan)?;
                if lexical.is_none()
                    && row.lineage != checked_trees::BorrowLoanLineage::SuppliedOutput
                {
                    continue;
                }
                if row.lineage == checked_trees::BorrowLoanLineage::SuppliedOutput
                    && super::supplied_permissions::normal_call(&facts.borrow, &facts.flow, loan)
                        .is_none()
                {
                    continue;
                }
                let mut holders = vec![loan];
                let mut holder_ends = vec![lexical];
                let mut cursor = 0;
                let mut final_weakening = lexical;
                while cursor < holders.len() {
                    let supplier = holders[cursor];
                    cursor += 1;
                    let supplier_end = lexical_end(facts, flow_state, supplier)?
                        .map(|handle| facts.flow.borrow_lifetimes.weakenings.get(handle));
                    let Some(transferred) = transfers
                        .get(supplier.arena_index() as usize)
                        .filter(|row| row.loan == supplier)
                    else {
                        continue;
                    };
                    for candidate in transferred.holders.iter().copied() {
                        if holders.contains(&candidate) {
                            continue;
                        }
                        let start = holder_start(facts, flow_state, candidate)?;
                        let weakening =
                            holding_end(facts, flow_state, candidate, &transfers, &descendants)?;
                        if supplier_end.is_some_and(|end| {
                            weakening_event_boundary(end.source, end.reason)
                                .is_ok_and(|boundary| start >= boundary)
                        }) {
                            return Err(vec![Diagnostic::error(
                                "saved descriptor was not captured before its supplier retired",
                            )]);
                        }
                        if let Some(weakening) = weakening {
                            let end = facts.flow.borrow_lifetimes.weakenings.get(weakening);
                            let later = if let Some(previous) = final_weakening {
                                let previous = facts.flow.borrow_lifetimes.weakenings.get(previous);
                                weakening_event_boundary(end.source, end.reason)?
                                    > weakening_event_boundary(previous.source, previous.reason)?
                            } else {
                                true
                            };
                            if later {
                                final_weakening = Some(weakening);
                            }
                        }
                        holder_ends.push(weakening);
                        holders.push(candidate);
                    }
                }
                let position = loan.arena_index() as usize;
                if rows.len() <= position {
                    rows.resize_with(position + 1, LoanRetention::default);
                }
                rows[position] = LoanRetention {
                    loan,
                    retention: CheckedBorrowResourceRetention {
                        lexical_weakening: lexical.unwrap_or_default(),
                        holders: holders_arena.insert_many(holders),
                        final_weakening: final_weakening.unwrap_or_default(),
                    },
                };
            }
        }
        facts.borrow.resource_holders = holders_arena;
        Ok(rows)
    }

    /*
        The input hold is established by the completed call before any saved view
        can be formed. Its end must cover every descendant, even when that child's
        bookkeeping debt has no descriptor-forwarding edge of its own.
    */
    fn close_output_descendants(
        facts: &mut CheckFacts,
        transfers: &[TransferHolders],
    ) -> Result<Vec<TransferHolders>, Vec<Diagnostic>> {
        let mut children: Vec<TransferHolders> = Vec::new();
        for (child, row) in facts.borrow.loans.iter() {
            let checked_trees::BorrowLoanLineage::Reborrow { parent_loan } = row.lineage else {
                continue;
            };
            let position = parent_loan.arena_index() as usize;
            if children.len() <= position {
                children.resize_with(position + 1, TransferHolders::default);
            }
            let entry = &mut children[position];
            entry.loan = parent_loan;
            entry.holders.push(child);
        }
        for (_, output) in facts.borrow.supplied_output_resources.iter() {
            for input in facts
                .borrow
                .supplied_output_inputs
                .span_or_empty(output.inputs)
            {
                let checked_trees::CheckedOutputBorrowInputPermission::Borrowed { resource } =
                    input.permission
                else {
                    continue;
                };
                let input = facts.borrow.call_loan_resources.get(resource);
                let sources = match input.source {
                    checked_trees::CheckedCallBorrowLoanSource::StoredLoan { loan } => vec![loan],
                    checked_trees::CheckedCallBorrowLoanSource::CallAccess { parents, .. } => facts
                        .borrow
                        .call_loan_parents
                        .span_or_empty(parents)
                        .to_vec(),
                };
                for source in sources {
                    let position = source.arena_index() as usize;
                    if children.len() <= position {
                        children.resize_with(position + 1, TransferHolders::default);
                    }
                    let entry = &mut children[position];
                    entry.loan = source;
                    crate::shared::collections::push_unique(&mut entry.holders, output.loan);
                }
            }
        }
        let mut ends = Vec::new();
        for (handle, output) in facts.borrow.supplied_output_resources.iter() {
            let state = facts
                .flow
                .control
                .unique_state(output.machine_symbol, output.state_symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "completed output closure has no unique flow state",
                    )]
                })?;
            let (holders, holder_end) = closure_end(facts, state, output.loan, &[transfers])?;
            let (_, end) =
                closure_end(facts, state, output.loan, &[transfers, children.as_slice()])?;
            ends.push((
                handle,
                holders,
                holder_end.unwrap_or_default(),
                end.unwrap_or_default(),
            ));
        }
        for (handle, holders, holder_end, end) in ends {
            let holders = facts.borrow.loan_suppliers.insert_many(holders);
            let output = facts.borrow.supplied_output_resources.get_mut(handle);
            output.holders = holders;
            output.holder_weakening = holder_end;
            output.final_weakening = end;
        }
        Ok(children)
    }

    fn closure_end(
        facts: &CheckFacts,
        state: &checked_trees::FlowStateFact,
        root: Handle<BorrowLoanFact>,
        graphs: &[&[TransferHolders]],
    ) -> Result<
        (
            Vec<Handle<BorrowLoanFact>>,
            Option<Handle<FlowBorrowWeakeningFact>>,
        ),
        Vec<Diagnostic>,
    > {
        let (_, owner) = facts
            .borrow
            .first_state_borrow(state.machine_symbol, state.state_symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "completed output closure has no borrow owner",
                )]
            })?;
        let mut pending = vec![(root, false)];
        let mut completed = Vec::new();
        let mut active = Vec::new();
        let mut end = None;
        while let Some((loan, closing)) = pending.pop() {
            if completed.contains(&loan) {
                continue;
            }
            if closing {
                if active.pop() != Some(loan) {
                    return Err(vec![Diagnostic::error(
                        "completed output closure order is unavailable",
                    )]);
                }
                completed.push(loan);
                continue;
            }
            if active.contains(&loan) || !facts.borrow.state_owns_loan(owner, loan) {
                return Err(vec![Diagnostic::error(
                    "completed output has cyclic or foreign lifetime dependents",
                )]);
            }
            active.push(loan);
            pending.push((loan, true));
            if let Some(candidate) = lexical_end(facts, state, loan)? {
                end = later_end(facts, end, candidate)?;
            }
            for graph in graphs {
                if let Some(edges) = graph
                    .get(loan.arena_index() as usize)
                    .filter(|entry| entry.loan == loan)
                {
                    pending.extend(edges.holders.iter().rev().map(|child| (*child, false)));
                }
            }
        }
        Ok((completed, end))
    }

    fn holding_end(
        facts: &CheckFacts,
        state: &checked_trees::FlowStateFact,
        loan: Handle<BorrowLoanFact>,
        transfers: &[TransferHolders],
        descendants: &[TransferHolders],
    ) -> Result<Option<Handle<FlowBorrowWeakeningFact>>, Vec<Diagnostic>> {
        let mut end = lexical_end(facts, state, loan)?;
        if facts.borrow.retained_call_inputs(loan).is_some() {
            let (_, complete_end) = closure_end(facts, state, loan, &[transfers, descendants])?;
            if let Some(candidate) = complete_end {
                end = later_end(facts, end, candidate)?;
            }
        }
        for (_, dependency) in facts
            .borrow
            .output_lifetime_dependencies
            .iter()
            .filter(|(_, row)| row.loan == loan)
        {
            let checked_trees::CheckedOutputLifetimeDependencySource::Completion { output } =
                dependency.source
            else {
                continue;
            };
            let output = facts.borrow.supplied_output_resources.get(output);
            if facts
                .flow
                .borrow_lifetimes
                .weakenings
                .is_valid(output.final_weakening)
            {
                end = later_end(facts, end, output.final_weakening)?;
            }
        }
        Ok(end)
    }

    fn later_end(
        facts: &CheckFacts,
        previous: Option<Handle<FlowBorrowWeakeningFact>>,
        candidate: Handle<FlowBorrowWeakeningFact>,
    ) -> Result<Option<Handle<FlowBorrowWeakeningFact>>, Vec<Diagnostic>> {
        let Some(previous) = previous else {
            return Ok(Some(candidate));
        };
        let left = facts.flow.borrow_lifetimes.weakenings.get(previous);
        let right = facts.flow.borrow_lifetimes.weakenings.get(candidate);
        Ok(Some(
            if weakening_event_boundary(right.source, right.reason)?
                > weakening_event_boundary(left.source, left.reason)?
            {
                candidate
            } else {
                previous
            },
        ))
    }

    fn lexical_end(
        facts: &CheckFacts,
        state: &checked_trees::FlowStateFact,
        loan: Handle<BorrowLoanFact>,
    ) -> Result<Option<Handle<FlowBorrowWeakeningFact>>, Vec<Diagnostic>> {
        let rows = state
            .borrow_weakenings
            .handles()
            .filter(|handle| facts.flow.borrow_lifetimes.weakenings.get(*handle).loan == loan)
            .collect::<Vec<_>>();
        match rows.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(*one)),
            _ => Err(vec![Diagnostic::error(
                "retained descriptor has ambiguous lexical retirement",
            )]),
        }
    }

    fn holder_start(
        facts: &CheckFacts,
        state: &checked_trees::FlowStateFact,
        loan: Handle<BorrowLoanFact>,
    ) -> Result<super::lifecycle::LifecycleBoundaryKey, Vec<Diagnostic>> {
        let row = facts.borrow.loans.get(loan);
        if row.established_by_call.is_valid() {
            let origin = facts.borrow.calls.get(row.established_by_call);
            let normal = state
                .calls
                .handles()
                .filter(|handle| {
                    let actual = facts.flow.control.calls.get(*handle);
                    actual.statement_index == origin.statement_index
                        && actual.call_ordinal == origin.call_ordinal
                        && actual.target_symbol == origin.target_symbol
                        && actual.authored_expression == origin.authored_expression
                        && facts
                            .flow
                            .borrow_loan_constraints(actual.exit_constraints)
                            .any(|active| active == loan)
                })
                .collect::<Vec<_>>();
            if normal.len() != 1 {
                return Err(vec![Diagnostic::error(
                    "retained output holder has no exact normal completion",
                )]);
            }
            return Ok(super::lifecycle::normal_completion_boundary(
                origin.statement_index,
            ));
        }
        let (activation, _) = exact_loan_lifecycle_rows(&facts.borrow, &facts.flow, state, loan)
            .map_err(|error| vec![error])?;
        activation_boundary(
            facts
                .flow
                .borrow_lifetimes
                .activations
                .get(activation)
                .source,
        )
    }

    pub(super) fn get(
        rows: &[LoanRetention],
        loan: Handle<BorrowLoanFact>,
    ) -> Result<&CheckedBorrowResourceRetention, Vec<Diagnostic>> {
        rows.get(loan.arena_index() as usize)
            .filter(|row| row.loan == loan)
            .map(|row| &row.retention)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "borrow resource has no complete descriptor-holder closure",
                )]
            })
    }

    pub(super) fn end<'flow>(
        facts: &'flow checked_trees::FlowFacts,
        retention: &CheckedBorrowResourceRetention,
    ) -> &'flow FlowBorrowWeakeningFact {
        facts
            .borrow_lifetimes
            .weakenings
            .get(retention.final_weakening)
    }
}
pub(in crate::checks::borrows) mod invocation_permissions {
    /*
        A provider result uses the permission established by its completed call.
        Input lifetimes keep backing alive but do not identify the returned address.
    */
    use arena::Handle;
    use checked_trees::{BorrowLoanFact, CheckFacts, FlowCallFact, FlowOperandFact, FlowStateFact};

    /*
        Checked bodies retain their checked source relation. Opaque results instead
        require the exact permission published at this invocation's normal return.
    */
    pub(in crate::checks::borrows) fn resolve(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        invocation: &FlowCallFact,
        capture: &FlowOperandFact,
        result_type: typed_trees::types::TypeReferenceHandle,
        path: &[facts::PlaceSegment],
    ) -> Option<Result<Handle<BorrowLoanFact>, &'static str>> {
        if validation::call_target_has_checked_body_contract(program, invocation.target_symbol) {
            return None;
        }
        Some(resolve_opaque(
            program,
            facts,
            state,
            invocation,
            capture,
            result_type,
            path,
        ))
    }

    fn resolve_opaque(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        invocation: &FlowCallFact,
        capture: &FlowOperandFact,
        result_type: typed_trees::types::TypeReferenceHandle,
        path: &[facts::PlaceSegment],
    ) -> Result<Handle<BorrowLoanFact>, &'static str> {
        if capture.state_symbol != state.state_symbol
            || capture.statement_index != invocation.statement_index
            || !invocation.authored_expression.is_valid()
        {
            return Err("the returned permission belongs to another invocation");
        }
        let call = facts
            .borrow
            .call_at(
                state.machine_symbol,
                state.state_symbol,
                invocation.statement_index,
                invocation.call_ordinal,
            )
            .ok_or("the completed invocation has no checked borrow record")?;
        let checked = facts.borrow.calls.get(call);
        if checked.authored_expression != invocation.authored_expression
            || validation::call_target_entry_symbol(program, checked.target_symbol)
                != validation::call_target_entry_symbol(program, invocation.target_symbol)
        {
            return Err("the returned permission disagrees with its performed call");
        }
        let selected = validation::project_type_reference_from_segments(program, result_type, path)
            .ok_or("the returned permission has no declared result field")?;
        let access = crate::borrow::reference_borrow_access_kind(program, selected)
            .ok_or("the returned permission does not name a reference field")?;
        let expected = validation::reference_shell(program, selected)
            .ok_or("the returned permission has no reference carrier")?;
        let mut permissions = checked.completed_reference_loans.handles().filter(|loan| {
            let row = facts.borrow.loans.get(*loan);
            let Some(formed) = row.formation_owner_path else {
                return false;
            };
            if row.lineage != checked_trees::BorrowLoanLineage::DirectRoot
                || row.formation != checked_trees::BorrowLoanFormation::ReturnedDescriptor
                || row.established_by_call != call
                || row.statement_index != invocation.statement_index
                || row.formation_expression != invocation.authored_expression
                || row.root != facts::PlaceRoot::Expression(invocation.authored_expression)
                || row.kind.direct_reborrow_effect(&access).is_none()
                || !facts
                    .borrow
                    .incoming_references
                    .is_valid(row.incoming_reference)
                || !facts
                    .flow
                    .borrow_loan_constraints(capture.constraints)
                    .any(|active| active == *loan)
            {
                return false;
            }
            let incoming = facts.borrow.incoming_references.get(row.incoming_reference);
            let checked_trees::IncomingReferenceOrigin::Invocation { expression, target } =
                incoming.origin
            else {
                return false;
            };
            let owner = facts.borrow.owner_segments.span_or_empty(formed);
            let projection = crate::borrow::owner_place_segments(owner);
            incoming.machine_symbol == state.machine_symbol
                && incoming.state_symbol == state.state_symbol
                && expression == invocation.authored_expression
                && validation::call_target_entry_symbol(program, target)
                    == validation::call_target_entry_symbol(program, invocation.target_symbol)
                && validation::reference_shell(program, incoming.carrier_type).is_some_and(
                    |supplied| {
                        program.normalized_type_identity(supplied.referee)
                            == program.normalized_type_identity(expected.referee)
                    },
                )
                && facts
                    .borrow
                    .owner_segments
                    .span_or_empty(incoming.owner_path)
                    == owner
                && projection.len() == path.len()
                && projection.iter().zip(path).all(|(left, right)| {
                    crate::flow::canonical_place_segments_equal(program, *left, *right)
                })
        });
        let Some(permission) = permissions.next() else {
            if ::validation::trace_flags::state_reference_diagnostics() {
                eprintln!(
                    "returned-reference-permission-refused call={call:?} expression={:?} path={path:?} expected_type={selected:?} expected_access={access:?} capture={capture:?}",
                    invocation.authored_expression,
                );
                for loan in checked.completed_reference_loans.handles().take(12) {
                    let row = facts.borrow.loans.get(loan);
                    let incoming = facts
                        .borrow
                        .incoming_references
                        .is_valid(row.incoming_reference)
                        .then(|| facts.borrow.incoming_references.get(row.incoming_reference));
                    eprintln!(
                        "returned-reference-permission-candidate loan={loan:?} row={row:?} incoming={incoming:?}"
                    );
                }
            }
            return Err("the completed result has no active permission for this reference field");
        };
        if permissions.next().is_some() {
            return Err("the completed result has ambiguous reference permissions");
        }
        Ok(permission)
    }
}
mod lifecycle {
    /*
        Resource lifecycle phases, boundaries, events and the ephemeral statuses
        the activation and weakening batches update.
    */
    use crate::checks::borrows::resources::reborrow_disposition_drift;
    use crate::checks::borrows::resources::reborrow_drafts::{
        CheckedReborrowLoanResourceDraft, ParentResourceIndex,
    };
    use checked_trees::{
        BorrowFacts, BorrowLoanFact, CheckedBorrowResourceLifecyclePhase, FlowFacts,
        FlowInvalidationSource,
    };
    use diagnostics::Diagnostic;

    pub(crate) type LifecyclePhase = CheckedBorrowResourceLifecyclePhase;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) struct LifecycleBoundaryKey {
        statement_index: usize,
        pub(crate) phase: LifecyclePhase,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum LifecycleEventKind {
        CompleteOutput {
            call: arena::Handle<checked_trees::FlowCallFact>,
        },
        Activate {
            activation: arena::Handle<checked_trees::FlowBorrowActivationFact>,
        },
        Weaken {
            weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct LifecycleEvent {
        pub(crate) boundary: LifecycleBoundaryKey,
        pub(crate) resource: ParentResourceIndex,
        pub(crate) loan_order: u32,
        pub(crate) kind: LifecycleEventKind,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum EphemeralResourceStatus {
        Available,
        BorrowedBy {
            children: Vec<ParentResourceIndex>,
        },
        RetiredWhileBorrowed {
            children: Vec<ParentResourceIndex>,
            weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
        },
        Retired,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct EphemeralStatuses {
        direct: Vec<Option<EphemeralResourceStatus>>,
        reborrows: Vec<Option<EphemeralResourceStatus>>,
        supplied: Vec<Option<EphemeralResourceStatus>>,
    }

    impl EphemeralStatuses {
        pub(crate) fn new(direct: usize, reborrows: usize, supplied: usize) -> Self {
            Self {
                direct: vec![None; direct],
                reborrows: vec![None; reborrows],
                supplied: vec![None; supplied],
            }
        }

        pub(crate) fn get(&self, resource: ParentResourceIndex) -> Option<EphemeralResourceStatus> {
            match resource {
                ParentResourceIndex::Direct(index) => self.direct.get(index).cloned().flatten(),
                ParentResourceIndex::Reborrow(index) => {
                    self.reborrows.get(index).cloned().flatten()
                }
                ParentResourceIndex::Supplied(index) => self.supplied.get(index).cloned().flatten(),
            }
        }

        pub(crate) fn set(
            &mut self,
            resource: ParentResourceIndex,
            status: EphemeralResourceStatus,
        ) -> bool {
            let slot = match resource {
                ParentResourceIndex::Direct(index) => self.direct.get_mut(index),
                ParentResourceIndex::Reborrow(index) => self.reborrows.get_mut(index),
                ParentResourceIndex::Supplied(index) => self.supplied.get_mut(index),
            };
            let Some(slot) = slot else {
                return false;
            };
            *slot = Some(status);
            true
        }
    }

    /* The durable lifecycle rows of one loan: its activation and weakening. */
    pub(crate) type LoanLifecycleRows = (
        arena::Handle<checked_trees::FlowBorrowActivationFact>,
        arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
    );

    /*
        The one activation and one weakening row of `loan` at `flow_state`. Every
        checked borrow-resource reconstruction joins the lifecycle ledger through
        this reader, so a ledger that holds no single pair — or an activation that
        does not start at the loan's own formation statement — is refused here,
        never reconstructed differently per consumer.
    */
    pub(crate) fn exact_loan_lifecycle_rows(
        borrow: &BorrowFacts,
        flow: &FlowFacts,
        flow_state: &checked_trees::FlowStateFact,
        loan: arena::Handle<BorrowLoanFact>,
    ) -> Result<LoanLifecycleRows, Diagnostic> {
        if !borrow.loans.is_valid(loan) {
            return Err(Diagnostic::error("checked borrow lifecycle names no loan"));
        }
        let loan_fact = borrow.loans.get(loan);
        let formation_statement = loan_fact.statement_index;
        let activations = flow
            .borrow_lifetimes
            .activations
            .span_or_empty(flow_state.borrow_activations)
            .iter()
            .enumerate()
            .filter(|(_, event)| event.loan == loan)
            .filter_map(|(offset, _)| span_handle(flow_state.borrow_activations, offset))
            .collect::<Vec<_>>();
        let weakenings = flow
            .borrow_lifetimes
            .weakenings
            .span_or_empty(flow_state.borrow_weakenings)
            .iter()
            .enumerate()
            .filter(|(_, event)| event.loan == loan)
            .filter_map(|(offset, _)| span_handle(flow_state.borrow_weakenings, offset))
            .collect::<Vec<_>>();
        let ([activation], [weakening]) = (activations.as_slice(), weakenings.as_slice()) else {
            return Err(Diagnostic::error(format!(
                "checked borrow resource requires exactly one activation and one weakening \
             (state {:?}, loan {:?}, formation statement {formation_statement}, \
             last use {}, activations {}, weakenings {}, origin {:?})",
                flow_state.state_symbol,
                loan,
                loan_fact.last_use_statement_index,
                activations.len(),
                weakenings.len(),
                borrow
                    .incoming_references
                    .is_valid(loan_fact.incoming_reference)
                    .then(|| &borrow
                        .incoming_references
                        .get(loan_fact.incoming_reference)
                        .origin),
            )));
        };
        if flow.borrow_lifetimes.activations.get(*activation).source
            != (FlowInvalidationSource::Statement {
                statement_index: formation_statement,
            })
        {
            return Err(Diagnostic::error(
                "checked borrow resource activation drifted from loan formation",
            ));
        }
        Ok((*activation, *weakening))
    }

    pub(crate) fn activation_boundary(
        source: FlowInvalidationSource,
    ) -> Result<LifecycleBoundaryKey, Vec<Diagnostic>> {
        let FlowInvalidationSource::Statement { statement_index } = source else {
            return Err(reborrow_disposition_drift());
        };
        Ok(LifecycleBoundaryKey {
            statement_index,
            phase: LifecyclePhase::Activation,
        })
    }

    pub(crate) fn normal_completion_boundary(statement_index: usize) -> LifecycleBoundaryKey {
        LifecycleBoundaryKey {
            statement_index,
            phase: LifecyclePhase::NormalCompletion,
        }
    }

    pub(crate) fn weakening_event_boundary(
        source: FlowInvalidationSource,
        reason: checked_trees::FlowBorrowWeakeningReason,
    ) -> Result<LifecycleBoundaryKey, Vec<Diagnostic>> {
        let FlowInvalidationSource::Statement { statement_index } = source else {
            return Err(reborrow_disposition_drift());
        };
        let phase = match reason {
            checked_trees::FlowBorrowWeakeningReason::LastUseExpired => {
                LifecyclePhase::LastUseExpired
            }
            checked_trees::FlowBorrowWeakeningReason::LocalReassigned => {
                LifecyclePhase::LocalReassigned
            }
            checked_trees::FlowBorrowWeakeningReason::StateExit => LifecyclePhase::StateExit,
        };
        Ok(LifecycleBoundaryKey {
            statement_index,
            phase,
        })
    }

    pub(crate) fn apply_activation_batch(
        statuses: &mut EphemeralStatuses,
        batch: &[LifecycleEvent],
        installation: &[ParentResourceIndex],
        reborrows: &[CheckedReborrowLoanResourceDraft],
    ) -> Result<(), Vec<Diagnostic>> {
        for event in batch {
            let valid = match event.kind {
                LifecycleEventKind::Activate { activation } => activation.is_valid(),
                LifecycleEventKind::CompleteOutput { call } => {
                    call.is_valid() && matches!(event.resource, ParentResourceIndex::Supplied(_))
                }
                LifecycleEventKind::Weaken { .. } => false,
            };
            if !valid || statuses.get(event.resource).is_some() {
                return Err(reborrow_disposition_drift());
            }
            if let ParentResourceIndex::Reborrow(child_index) = event.resource {
                let Some(parent) = installation.get(child_index).copied() else {
                    return Err(reborrow_disposition_drift());
                };
                let Some(child) = reborrows.get(child_index) else {
                    return Err(reborrow_disposition_drift());
                };
                if child.parent_access.direct_reborrow_effect(&child.access)
                    != Some(child.access_effect)
                {
                    return Err(reborrow_disposition_drift());
                }
                let mut children = match statuses.get(parent) {
                    Some(EphemeralResourceStatus::Available) => Vec::new(),
                    Some(EphemeralResourceStatus::BorrowedBy { children }) => children,
                    _ => return Err(reborrow_disposition_drift()),
                };
                if children.contains(&event.resource) {
                    return Err(reborrow_disposition_drift());
                }
                children.push(event.resource);
                if !statuses.set(parent, EphemeralResourceStatus::BorrowedBy { children }) {
                    return Err(reborrow_disposition_drift());
                }
            }
            if !statuses.set(event.resource, EphemeralResourceStatus::Available) {
                return Err(reborrow_disposition_drift());
            }
        }
        Ok(())
    }

    pub(crate) fn apply_weakening_batch(
        statuses: &mut EphemeralStatuses,
        batch: &[LifecycleEvent],
    ) -> Result<Vec<(usize, arena::Handle<checked_trees::FlowBorrowWeakeningFact>)>, Vec<Diagnostic>>
    {
        let mut completed = Vec::new();
        for event in batch {
            let LifecycleEventKind::Weaken { weakening } = event.kind else {
                return Err(reborrow_disposition_drift());
            };
            let next = match statuses.get(event.resource) {
                Some(EphemeralResourceStatus::Available) => {
                    if let ParentResourceIndex::Reborrow(index) = event.resource {
                        completed.push((index, weakening));
                    }
                    EphemeralResourceStatus::Retired
                }
                Some(EphemeralResourceStatus::BorrowedBy { children }) => {
                    EphemeralResourceStatus::RetiredWhileBorrowed {
                        children,
                        weakening,
                    }
                }
                None
                | Some(EphemeralResourceStatus::RetiredWhileBorrowed { .. })
                | Some(EphemeralResourceStatus::Retired) => {
                    return Err(reborrow_disposition_drift());
                }
            };
            if !statuses.set(event.resource, next) {
                return Err(reborrow_disposition_drift());
            }
        }
        Ok(completed)
    }

    pub(crate) fn weakening_boundary_key(
        source: FlowInvalidationSource,
        reason: checked_trees::FlowBorrowWeakeningReason,
    ) -> Option<(usize, u8)> {
        let FlowInvalidationSource::Statement { statement_index } = source else {
            return None;
        };
        let phase = match reason {
            checked_trees::FlowBorrowWeakeningReason::LastUseExpired => 0,
            checked_trees::FlowBorrowWeakeningReason::LocalReassigned => 1,
            checked_trees::FlowBorrowWeakeningReason::StateExit => 2,
        };
        Some((statement_index, phase))
    }

    /* Translate a span offset into the durable handle the row carries. */
    pub(crate) fn span_handle<T>(
        span: arena::HandleSpan<T>,
        offset: usize,
    ) -> Option<arena::Handle<T>> {
        u32::try_from(offset)
            .ok()
            .map(|offset| span.handle_at(offset))
    }
}
mod loan_resources {
    /*
        Rebuilding the direct-borrow and reborrow resource arenas from the loan
        and flow-lifetime ledgers, and resolving each reborrow's parent graph.

        One traversal over the flow ledger's state-owned loans projects both
        rosters: a loan's recorded lineage decides whether it closes as a
        direct-root resource or a reborrow draft, so neither roster can silently
        omit loans the other sees. Every reconstruction joins the loan's
        activation and weakening rows through the lifecycle reader.
    */
    use crate::checks::borrows::details::binding_reference_access;
    use crate::checks::borrows::resources::invalid_reborrow_attenuation_diagnostic;
    use crate::checks::borrows::resources::lifecycle::{
        exact_loan_lifecycle_rows, span_handle, weakening_boundary_key,
    };
    use crate::checks::borrows::resources::reborrow_drafts::{
        CheckedReborrowDispositionEventDraft, CheckedReborrowLoanResourceDraft as ResourceDraft,
        ParentResourceIndex, ResourceHandles,
    };
    use crate::checks::borrows::resources::reborrow_resource_drift;
    use checked_trees::{
        BorrowFacts, BorrowLoanLineage, CheckedDirectBorrowLoanResource as LoanResource,
        CheckedDirectBorrowParentLifetime, CheckedDirectBorrowRestorationObligation,
        CheckedParentBorrowResource, CheckedReborrowLoanResource, FlowFacts,
        FlowInvalidationSource, ParentLexicalStatusAtChildEnd as ChildEnd,
    };
    use diagnostics::Diagnostic;

    /*
        Resolve the entire parent graph before either retained arena is reset.
        Installation is therefore a purely indexed, infallible rewrite.
    */
    pub(crate) fn plan_resource_installation(
        borrow: &BorrowFacts,
        direct: &[LoanResource],
        reborrows: &[ResourceDraft],
    ) -> Result<Vec<ParentResourceIndex>, Vec<Diagnostic>> {
        let mut plan = Vec::with_capacity(reborrows.len());
        for (child_index, child) in reborrows.iter().enumerate() {
            let direct_matches = direct
                .iter()
                .enumerate()
                .filter(|(_, resource)| resource.loan == child.parent_loan)
                .map(|(index, _)| ParentResourceIndex::Direct(index));
            let reborrow_matches = reborrows[..child_index]
                .iter()
                .enumerate()
                .filter(|(_, resource)| resource.loan == child.parent_loan)
                .map(|(index, _)| ParentResourceIndex::Reborrow(index));
            let supplied_matches = borrow
                .supplied_output_resources
                .iter()
                .enumerate()
                .filter(|(_, (_, resource))| resource.loan == child.parent_loan)
                .map(|(index, _)| ParentResourceIndex::Supplied(index));
            let mut matches = direct_matches
                .chain(reborrow_matches)
                .chain(supplied_matches);
            let Some(parent) = matches.next() else {
                return Err(reborrow_resource_drift());
            };
            if matches.next().is_some() {
                return Err(reborrow_resource_drift());
            }
            plan.push(parent);
        }
        Ok(plan)
    }

    pub(crate) fn install_borrow_resources(
        borrow: &mut BorrowFacts,
        direct: Vec<LoanResource>,
        reborrows: &[ResourceDraft],
        installation: &[ParentResourceIndex],
        dispositions: &[CheckedReborrowDispositionEventDraft],
    ) {
        borrow.direct_loan_resources.reset_retain_capacity();
        borrow.reborrow_loan_resources.reset_retain_capacity();
        borrow.reborrow_disposition_events.reset_retain_capacity();

        let mut direct_handles = Vec::with_capacity(direct.len());
        for resource in direct {
            let handle = borrow.direct_loan_resources.insert(resource);
            direct_handles.push(handle);
        }

        let mut reborrow_handles: Vec<arena::Handle<CheckedReborrowLoanResource>> =
            Vec::with_capacity(reborrows.len());
        let supplied_handles = borrow
            .supplied_output_resources
            .iter()
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for (draft, parent) in reborrows.iter().zip(installation) {
            let parent_resource = match *parent {
                ParentResourceIndex::Direct(index) => CheckedParentBorrowResource::DirectRoot {
                    resource: direct_handles[index],
                },
                ParentResourceIndex::Reborrow(index) => CheckedParentBorrowResource::Reborrow {
                    resource: reborrow_handles[index],
                },
                ParentResourceIndex::Supplied(index) => {
                    CheckedParentBorrowResource::SuppliedOutput {
                        resource: supplied_handles[index],
                    }
                }
            };
            let handle = borrow
                .reborrow_loan_resources
                .insert(draft.close(parent_resource));
            reborrow_handles.push(handle);
        }

        let handles = ResourceHandles {
            direct: direct_handles,
            reborrows: reborrow_handles,
            supplied: supplied_handles,
        };
        for disposition in dispositions {
            borrow
                .reborrow_disposition_events
                .insert(disposition.close(borrow, &handles));
        }
    }

    /*
        Rebuild both checked resource rosters in one traversal of the state-owned
        loans: direct-root loans close as direct resources in their own arena
        (every derived occurrence stays outside it), reborrows close as drafts for
        the parent graph and disposition events. A loan's lineage selects its
        roster; there is no second walk that could disagree about coverage.
    */
    pub(crate) fn reconstruct_borrow_resource_rosters(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        flow: &FlowFacts,
        retentions: &[super::forwarded_lifetimes::LoanRetention],
    ) -> Result<(Vec<LoanResource>, Vec<ResourceDraft>), Vec<Diagnostic>> {
        let mut direct = Vec::new();
        let mut reborrows = Vec::new();
        let mut diagnostics = Vec::new();

        for (_, state) in borrow.states.iter() {
            let Some(flow_state) = flow.control.state(state.machine_symbol, state.state_symbol)
            else {
                diagnostics.push(Diagnostic::error(
                    "checked borrow resource has no exact flow-state owner",
                ));
                continue;
            };

            if state.loans.is_empty() {
                continue;
            }

            let mut activated = vec![false; state.loans.count() as usize];
            for activation in flow
                .borrow_lifetimes
                .activations
                .span_or_empty(flow_state.borrow_activations)
            {
                if borrow.state_owns_loan(state, activation.loan) {
                    activated[(activation.loan.arena_index() - state.loans.start().arena_index())
                        as usize] = true;
                }
            }
            for (loan_handle, loan) in borrow.state_loans(state) {
                if (borrow.is_call_result_loan(loan)
                    || (borrow.incoming_references.is_valid(loan.incoming_reference)
                        && matches!(
                            borrow
                                .incoming_references
                                .get(loan.incoming_reference)
                                .origin,
                            checked_trees::IncomingReferenceOrigin::DescriptorLoad { .. }
                        )))
                    && !activated
                        [(loan_handle.arena_index() - state.loans.start().arena_index()) as usize]
                {
                    continue;
                }
                match &loan.lineage {
                    BorrowLoanLineage::DirectRoot => {
                        if let Some(parent_access) = binding_reference_access(
                            program,
                            state.machine_symbol,
                            state.state_symbol,
                            loan.statement_index,
                            loan.root,
                        ) && parent_access.direct_reborrow_effect(&loan.kind).is_none()
                        {
                            diagnostics.push(invalid_reborrow_attenuation_diagnostic(
                                &parent_access,
                                &loan.kind,
                            ));
                            continue;
                        }

                        let (activation_handle, _weakening_handle) = match exact_loan_lifecycle_rows(
                            borrow,
                            flow,
                            flow_state,
                            loan_handle,
                        ) {
                            Ok(rows) => rows,
                            Err(mut drift) => {
                                if let Some((machine, source_state)) =
                                    validation::find_state(program, state.state_symbol)
                                {
                                    drift.message.push_str(&format!(
                                        "; source scope {}::{}",
                                        machine.name, source_state.name
                                    ));
                                }
                                diagnostics.push(drift);
                                continue;
                            }
                        };
                        let activation = flow.borrow_lifetimes.activations.get(activation_handle);
                        let retention =
                            super::forwarded_lifetimes::get(retentions, loan_handle)?.clone();
                        let weakening = super::forwarded_lifetimes::end(flow, &retention);

                        let parent_lifetime = CheckedDirectBorrowParentLifetime {
                            machine_symbol: state.machine_symbol,
                            state_symbol: state.state_symbol,
                            root: loan.root,
                        };
                        let restoration = CheckedDirectBorrowRestorationObligation {
                            parent: parent_lifetime.clone(),
                            weakening_source: weakening.source,
                            weakening_reason: weakening.reason,
                        };
                        direct.push(LoanResource {
                            loan: loan_handle,
                            machine_symbol: state.machine_symbol,
                            state_symbol: state.state_symbol,
                            owner_symbol: loan.owner_symbol,
                            owner_path: borrow.loan_owner_path(loan).to_vec(),
                            captured_place: checked_trees::CapturedPlace {
                                incoming_reference: loan.incoming_reference,
                                incoming_owner_path: borrow
                                    .access_segments
                                    .span_or_empty(loan.incoming_owner_path)
                                    .to_vec(),
                                root: loan.root,
                                segments: borrow.loan_segments(loan).to_vec(),
                            },
                            access: loan.kind.clone(),
                            activation_source: activation.source,
                            retention,
                            weakening_source: weakening.source,
                            weakening_reason: weakening.reason,
                            parent_lifetime,
                            restoration,
                        });
                    }

                    BorrowLoanLineage::Reborrow { parent_loan } => {
                        let parent = borrow.loans.get(*parent_loan);
                        let Some(access_effect) = parent.kind.direct_reborrow_effect(&loan.kind)
                        else {
                            diagnostics.push(invalid_reborrow_attenuation_diagnostic(
                                &parent.kind,
                                &loan.kind,
                            ));
                            continue;
                        };
                        let (child_activation, child_weakening) = match exact_loan_lifecycle_rows(
                            borrow,
                            flow,
                            flow_state,
                            loan_handle,
                        ) {
                            Ok(rows) => rows,
                            Err(mut drift) => {
                                if let Some((machine, source_state)) =
                                    validation::find_state(program, state.state_symbol)
                                {
                                    drift.message.push_str(&format!(
                                        "; source scope {}::{}",
                                        machine.name, source_state.name
                                    ));
                                }
                                diagnostics.push(drift);
                                continue;
                            }
                        };
                        let activation = flow.borrow_lifetimes.activations.get(child_activation);
                        let retention =
                            super::forwarded_lifetimes::get(retentions, loan_handle)?.clone();
                        let weakening = super::forwarded_lifetimes::end(flow, &retention);
                        let sources = borrow.loan_suppliers.span_or_empty(loan.supplying_loans);
                        let source_loan = match sources {
                            [] => *parent_loan,
                            [source] => *source,
                            _ => {
                                return Err(vec![Diagnostic::error(
                                    "reborrow formation has no unique current descriptor source",
                                )]);
                            }
                        };
                        if !borrow.state_owns_loan(state, source_loan)
                            || borrow
                                .loans
                                .get(source_loan)
                                .kind
                                .direct_reborrow_effect(&loan.kind)
                                .is_none()
                        {
                            return Err(vec![Diagnostic::error(
                                "reborrow exceeds its source descriptor's access",
                            )]);
                        }
                        let parent_weakenings =
                            if parent.lineage == BorrowLoanLineage::SuppliedOutput {
                                borrow
                                    .supplied_output_resources
                                    .iter()
                                    .filter(|(_, resource)| resource.loan == *parent_loan)
                                    .filter(|&(_, resource)| {
                                        flow.borrow_lifetimes
                                            .weakenings
                                            .is_valid(resource.holder_weakening)
                                    })
                                    .map(|(_, resource)| {
                                        (
                                            resource.holder_weakening,
                                            flow.borrow_lifetimes
                                                .weakenings
                                                .get(resource.holder_weakening),
                                        )
                                    })
                                    .collect::<Vec<_>>()
                            } else {
                                flow.borrow_lifetimes
                                    .weakenings
                                    .span_or_empty(flow_state.borrow_weakenings)
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, weakening)| weakening.loan == *parent_loan)
                                    .filter_map(|(offset, weakening)| {
                                        span_handle(flow_state.borrow_weakenings, offset)
                                            .map(|handle| (handle, weakening))
                                    })
                                    .collect::<Vec<_>>()
                            };
                        let [(parent_weakening, parent_weakening_fact)] =
                            parent_weakenings.as_slice()
                        else {
                            diagnostics.push(Diagnostic::error(
                                "checked direct-reborrow parent status requires exactly one parent weakening",
                            ));
                            continue;
                        };
                        let Some(parent_lexical_status) = parent_lexical_status_at_child_end(
                            parent_weakening_fact.source,
                            parent_weakening_fact.reason,
                            flow.borrow_lifetimes.weakenings.get(child_weakening).source,
                            flow.borrow_lifetimes.weakenings.get(child_weakening).reason,
                        ) else {
                            diagnostics.push(Diagnostic::error(
                                "checked direct-reborrow parent status has an unsupported weakening boundary",
                            ));
                            continue;
                        };

                        let Some(statement) = flow
                            .control
                            .statements
                            .span_or_empty(flow_state.statements)
                            .iter()
                            .find(|statement| statement.statement_index == loan.statement_index)
                        else {
                            diagnostics.push(Diagnostic::error(
                                "checked direct-reborrow suspension has no exact formation statement",
                            ));
                            continue;
                        };
                        let parent_constraints = flow
                            .contexts
                            .constraint_refs
                            .span_or_empty(statement.entry_constraints)
                            .iter()
                            .enumerate()
                            .filter(|(_, constraint)| {
                                constraint.kind
                                    == checked_trees::FlowConstraintKind::BorrowLoan {
                                        loan: source_loan,
                                    }
                            })
                            .filter_map(|(offset, _)| {
                                span_handle(statement.entry_constraints, offset)
                            })
                            .collect::<Vec<_>>();
                        let [parent_entry_constraint] = parent_constraints.as_slice() else {
                            diagnostics.push(Diagnostic::error(
                                "checked direct-reborrow suspension requires exactly one parent entry constraint",
                            ));
                            continue;
                        };

                        reborrows.push(ResourceDraft {
                            loan: loan_handle,
                            machine_symbol: state.machine_symbol,
                            state_symbol: state.state_symbol,
                            owner_symbol: loan.owner_symbol,
                            owner_path: borrow.loan_owner_path(loan).to_vec(),
                            captured_place: checked_trees::CapturedPlace {
                                incoming_reference: loan.incoming_reference,
                                incoming_owner_path: borrow
                                    .access_segments
                                    .span_or_empty(loan.incoming_owner_path)
                                    .to_vec(),
                                root: loan.root,
                                segments: borrow.loan_segments(loan).to_vec(),
                            },
                            access: loan.kind.clone(),
                            parent_access: parent.kind.clone(),
                            access_effect,
                            activation_source: activation.source,
                            retention,
                            source_loan,
                            weakening_source: weakening.source,
                            weakening_reason: weakening.reason,
                            parent_loan: *parent_loan,
                            child_activation,
                            parent_entry_constraint: *parent_entry_constraint,
                            child_weakening,
                            parent_end: if parent.lineage == BorrowLoanLineage::SuppliedOutput {
                                checked_trees::CheckedBorrowParentEnd::CompletedOutput {
                                    holder_weakening: *parent_weakening,
                                }
                            } else {
                                checked_trees::CheckedBorrowParentEnd::Lexical {
                                    weakening: *parent_weakening,
                                }
                            },
                            parent_lexical_status,
                        });
                    }

                    BorrowLoanLineage::LifetimeDependency
                    | BorrowLoanLineage::UnretainedDerived
                    | BorrowLoanLineage::SuppliedOutput => {}
                }
            }
        }

        if diagnostics.is_empty() {
            Ok((direct, reborrows))
        } else {
            Err(diagnostics)
        }
    }

    fn parent_lexical_status_at_child_end(
        parent_source: FlowInvalidationSource,
        parent_reason: checked_trees::FlowBorrowWeakeningReason,
        child_source: FlowInvalidationSource,
        child_reason: checked_trees::FlowBorrowWeakeningReason,
    ) -> Option<ChildEnd> {
        let parent = weakening_boundary_key(parent_source, parent_reason)?;
        let child = weakening_boundary_key(child_source, child_reason)?;
        Some(match parent.cmp(&child) {
            std::cmp::Ordering::Less => ChildEnd::RetiredBeforeChild,
            std::cmp::Ordering::Equal => ChildEnd::RetiredWithChild,
            std::cmp::Ordering::Greater => ChildEnd::LivePastChild,
        })
    }
}
mod permission_sources {
    /*
        A saved reference keeps access to its original data after its source slot
        changes. Follow that saved value to the checked borrow that permits its use.
        Completed captures and projections identify every possible supplier; equal
        geometry and lifetime roots cannot create that permission association.
        A temporary call access may end while its stored descriptor retains it.
    */
    use super::retained_backing::retained_places;
    use crate::borrow::{owner_path_from_place_segments, owner_place_segments};
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use crate::published::places::place_at;
    use arena::{Arena, Handle};
    use checked_trees::CheckedCallBorrowLoanSource;
    use checked_trees::{BorrowLoanFact, CheckFacts, FlowCallFact, FlowOperandFact, FlowStateFact};
    use validation::completed_storage::{FrozenReferenceSelection, frozen_reference_leaf};

    /*
        Permission selected by an exact completed descriptor. Its consumer owns
        destination paths and invocation coordinates separately.
    */
    pub(super) struct PermissionSupplier {
        pub(super) source: CheckedCallBorrowLoanSource,
        /* Original evaluated reference when this is an invocation access. */
        pub(super) actual: Handle<FlowOperandFact>,
        pub(super) backing: checked_trees::CapturedPlace,
    }

    #[derive(Debug, Clone, Copy)]
    struct PermissionFailureStep {
        source: Handle<FlowOperandFact>,
        state: symbols::SymbolHandle,
        statement_index: usize,
        carrier: typed_trees::types::TypeReferenceHandle,
        expected: Option<typed_trees::types::TypeReferenceHandle>,
        projection: [Option<facts::PlaceSegment>; 8],
        projection_count: usize,
        rule: &'static str,
    }

    /* Failure scratch stays on the request's stack. Successful paths emit nothing. */
    #[derive(Debug, Clone, Copy, Default)]
    pub(super) struct PermissionFailures {
        steps: [Option<PermissionFailureStep>; 4],
        kept: usize,
        total: usize,
    }

    impl PermissionFailures {
        fn record(
            &mut self,
            source: Handle<FlowOperandFact>,
            capture: Option<&FlowOperandFact>,
            projection: &[facts::PlaceSegment],
            expected: Option<typed_trees::types::TypeReferenceHandle>,
            rule: &'static str,
        ) {
            self.total = self.total.saturating_add(1);
            if self.kept == self.steps.len() {
                return;
            }
            let mut path = [None; 8];
            for (selected, segment) in path.iter_mut().zip(projection) {
                *selected = Some(*segment);
            }
            self.steps[self.kept] = Some(PermissionFailureStep {
                source,
                state: capture.map_or_else(Default::default, |capture| capture.state_symbol),
                statement_index: capture.map_or(0, |capture| capture.statement_index),
                carrier: capture.map_or_else(Default::default, |capture| capture.type_reference),
                expected,
                projection: path,
                projection_count: projection.len(),
                rule,
            });
            self.kept += 1;
        }
    }

    #[derive(Debug)]
    pub(super) struct PermissionSourceFailure {
        reason: &'static str,
        details: Option<Box<PermissionFailures>>,
    }

    impl From<&'static str> for PermissionSourceFailure {
        fn from(reason: &'static str) -> Self {
            Self {
                reason,
                details: None,
            }
        }
    }

    impl From<PermissionSourceFailure> for &'static str {
        fn from(failure: PermissionSourceFailure) -> Self {
            failure.reason
        }
    }

    impl std::fmt::Display for PermissionSourceFailure {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(self.reason)
        }
    }

    impl PermissionSourceFailure {
        pub(super) fn describe(&self, program: &dyn validation::CheckingInput) -> String {
            use std::fmt::Write;
            let mut message = self.reason.to_owned();
            let Some(details) = &self.details else {
                return message;
            };
            for step in details.steps.iter().take(details.kept).flatten() {
                let state = if step.state.is_valid() {
                    program
                        .symbols
                        .display_path(step.state, "::")
                        .chars()
                        .take(128)
                        .collect::<String>()
                } else {
                    "<unavailable>".to_owned()
                };
                let carrier = step.carrier.is_valid().then(|| {
                    program
                        .normalized_type_identity_text(step.carrier)
                        .chars()
                        .take(256)
                        .collect::<String>()
                });
                let expected =
                    step.expected
                        .filter(|reference| reference.is_valid())
                        .map(|reference| {
                            program
                                .normalized_type_identity_text(reference)
                                .chars()
                                .take(256)
                                .collect::<String>()
                        });
                let path = &step.projection[..step.projection_count.min(step.projection.len())];
                let _ = write!(
                    message,
                    "; source {:?} in {state} statement {}: {}; carrier {carrier:?}, expected {expected:?}, path {path:?} ({} segments)",
                    step.source, step.statement_index, step.rule, step.projection_count
                );
            }
            if details.total > details.kept {
                let _ = write!(
                    message,
                    "; {} further failed source checks omitted",
                    details.total - details.kept
                );
            }
            message
        }
    }

    /*
        Find the permission lending a new view, separately from the complete call
        input debt. Stored descriptor alternatives retain their own parents; the
        call's conservative ancestry union cannot choose a parent for one leaf.
        An explicit argument borrow supplies its checked access when it has no
        stored descriptor occurrence. Both routes must establish complete coverage.
    */
    pub(super) fn lending_sources(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        call: Handle<FlowCallFact>,
        source: Handle<FlowOperandFact>,
        projection: &[facts::PlaceSegment],
        access: &checked_trees::BorrowAccessKind,
    ) -> Result<(Vec<PermissionSupplier>, Arena<Handle<BorrowLoanFact>>), PermissionSourceFailure>
    {
        use super::super::formation_captures::{
            AddressReceiver, address_receiver, formation_capture,
        };
        let capture = facts.flow.control.operands.get(source);
        let mut lending = source;
        let mut lending_path = projection.to_vec();
        let mut actual_projection = Vec::new();
        let mut argument_grant = projection.is_empty()
            && has_captured_entry_reference(program, facts, loan_occurrences, state, source);
        /*
            A bare range is its own shared formation (`bare-range-reference-access`):
            `split(items[a..b])` lends exactly what `split(&items[a..b])` lends, so
            the call's checked argument access supplies the view like an explicit
            declared borrow whose referee matches its target.
        */
        if projection.is_empty()
            && let Some((machine, checked_state)) =
                validation::machine_state_by_symbols(program, state.machine_symbol, state.state_symbol)
            && validation::bare_range_view_source(
                program,
                machine,
                Some(checked_state),
                capture.expression,
            )
            .is_some()
        {
            argument_grant = true;
        } else if projection.is_empty()
            && let Some(formation) =
                validation::reference_formation_source(program, capture.expression)
        {
            let target = formation_capture(facts, source, formation.source)
                .ok_or("the explicit reference source has no completed capture")?;
            match address_receiver(program, facts, target) {
                AddressReceiver::Reference {
                    capture: receiver,
                    projection,
                } => {
                    let target_type = facts.flow.control.operands.get(target).type_reference;
                    let result = validation::reference_shell(program, capture.type_reference)
                        .ok_or("the captured formation is not a reference")?;
                    if formation.interpretation
                        == validation::ReferenceFormationInterpretation::Declared
                        && validation::storage_origin_types_match(
                            program,
                            result.referee,
                            target_type,
                        )
                    {
                        argument_grant = true;
                    } else {
                        lending = receiver;
                        lending_path.clear();
                        actual_projection = projection;
                        argument_grant = has_captured_entry_reference(
                            program,
                            facts,
                            loan_occurrences,
                            state,
                            receiver,
                        );
                    }
                }
                AddressReceiver::InlineValue => argument_grant = true,
                AddressReceiver::Unavailable => {
                    return Err("the formation receiver has no complete descriptor path".into());
                }
            }
        }
        let (invocation, lending, lending_path) = if argument_grant {
            (call, source, projection)
        } else {
            (Handle::invalid(), lending, lending_path.as_slice())
        };
        let mut suppliers = Vec::new();
        let mut parents = Arena::new();
        let mut failures = PermissionFailures::default();
        let complete = collect(
            program,
            facts,
            loan_occurrences,
            state,
            invocation,
            lending,
            lending_path,
            access,
            false,
            &mut Vec::new(),
            &mut suppliers,
            &mut parents,
            &mut failures,
        );
        if !complete || suppliers.is_empty() {
            return Err(PermissionSourceFailure {
                reason: "the saved descriptor has no complete checked borrowing source",
                details: Some(Box::new(failures)),
            });
        }
        if !argument_grant {
            for supplier in &mut suppliers {
                supplier
                    .backing
                    .segments
                    .extend_from_slice(&actual_projection);
                crate::flow::compose_captured_view_segments(&mut supplier.backing.segments)
                    .ok_or("the captured source projection cannot be composed")?;
            }
        }
        Ok((suppliers, parents))
    }

    /*
        A helper can retain either a borrowed input or a reference field read from
        it. The unchanged storage snapshot and current entry roster identify that
        descriptor. Its complete owner path selects the pointer's own permission;
        an enclosing record's borrow cannot stand in for an interior reference.
    */
    fn has_captured_entry_reference(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        source: Handle<FlowOperandFact>,
    ) -> bool {
        let capture = facts.flow.control.operands.get(source);
        let Some(actual) = validation::reference_shell(program, capture.type_reference) else {
            return false;
        };
        if capture.state_symbol != state.state_symbol {
            return false;
        }
        let Some(place) = place_at(
            program,
            capture.state_symbol,
            capture.statement_index,
            capture.expression,
        ) else {
            return false;
        };
        let Some(validation::completed_storage::StoredOperand::Incoming(selected)) =
            validation::completed_storage::sources_at_snapshot(
                program,
                &facts.flow.control,
                capture.storage_snapshot,
                &place,
                validation::completed_storage::StoredObservation::Binding,
            )
        else {
            return false;
        };
        if selected != place {
            return false;
        }
        super::super::aliases::resolve_descriptor_places(
            program,
            facts,
            loan_occurrences,
            state,
            capture.constraints,
            checked_trees::CapturedPlace {
                root: place.root,
                segments: place.segments,
                ..Default::default()
            },
        )
        .is_some_and(|sources| {
            !sources.is_empty()
                && sources.iter().all(|source| {
                    if source.local_loan.is_valid()
                        || !source.place.segments.is_empty()
                        || !facts
                            .borrow
                            .incoming_references
                            .is_valid(source.place.incoming_reference)
                    {
                        return false;
                    }
                    let incoming = facts
                        .borrow
                        .incoming_references
                        .get(source.place.incoming_reference);
                    if !matches!(
                        incoming.origin,
                        checked_trees::IncomingReferenceOrigin::ParameterReferent { .. }
                            | checked_trees::IncomingReferenceOrigin::Parameter { .. }
                    ) {
                        return false;
                    }
                    let Some(owner) = facts.borrow.owner_segments.span(incoming.owner_path) else {
                        return false;
                    };
                    let Some(selected) = validation::owned_projection_type_reference(
                        program,
                        incoming.carrier_type,
                        &owner_place_segments(owner),
                    ) else {
                        return false;
                    };
                    validation::reference_shell(program, selected).is_some_and(|declared| {
                        checked_trees::BorrowAccessKind::from(declared.access)
                            .direct_reborrow_effect(&checked_trees::BorrowAccessKind::from(
                                actual.access,
                            ))
                            .is_some()
                            && validation::storage_origin_types_match(
                                program,
                                actual.referee,
                                declared.referee,
                            )
                    })
                })
        })
    }

    pub(super) fn collect(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        call: Handle<FlowCallFact>,
        source: Handle<FlowOperandFact>,
        projection: &[facts::PlaceSegment],
        output_access: &checked_trees::BorrowAccessKind,
        retained_by_store: bool,
        active: &mut Vec<(Handle<FlowOperandFact>, Vec<facts::PlaceSegment>)>,
        output: &mut Vec<PermissionSupplier>,
        parents: &mut Arena<Handle<BorrowLoanFact>>,
        failures: &mut PermissionFailures,
    ) -> bool {
        macro_rules! push_stored_loan {
            ($loan:expr, $row:expr) => {
                output.push(PermissionSupplier {
                    source: CheckedCallBorrowLoanSource::StoredLoan { loan: $loan },
                    actual: Handle::invalid(),
                    backing: checked_trees::CapturedPlace {
                        incoming_reference: $row.incoming_reference,
                        incoming_owner_path: facts
                            .borrow
                            .access_segments
                            .span_or_empty($row.incoming_owner_path)
                            .to_vec(),
                        root: $row.root,
                        segments: facts.borrow.loan_segments($row).to_vec(),
                    },
                });
            };
        }
        macro_rules! collect_supplier {
            ($source:expr, $projection:expr, $retained_by_store:expr $(,)?) => {
                collect(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    call,
                    $source,
                    $projection,
                    output_access,
                    $retained_by_store,
                    active,
                    output,
                    parents,
                    failures,
                )
            };
        }
        let Some((_, borrow_state)) = facts
            .borrow
            .first_state_borrow(state.machine_symbol, state.state_symbol)
        else {
            failures.record(
                source,
                None,
                projection,
                None,
                "source has no checked borrow state",
            );
            return false;
        };
        let failure_start = (failures.kept, failures.total);
        let query = (source, projection.to_vec());
        if active.contains(&query) {
            failures.record(
                source,
                None,
                projection,
                None,
                "source association is cyclic",
            );
            return false;
        }
        active.push(query);
        let before = output.len();
        if !facts.flow.control.operands.is_valid(source) {
            active.pop();
            failures.record(
                source,
                None,
                projection,
                None,
                "source capture is unavailable",
            );
            return false;
        }
        let capture = facts.flow.control.operands.get(source);
        let retained_suppliers = validation::completed_storage::descriptor_suppliers(
            &facts.flow.control,
            capture.retention,
            &mut Vec::new(),
        );
        let frozen = frozen_reference_leaf(program, &facts.flow.control, source, projection);
        let plain_observation = matches!(&frozen, FrozenReferenceSelection::NotFrozen);
        let frozen_kind = match &frozen {
            FrozenReferenceSelection::NotFrozen => "not-frozen",
            FrozenReferenceSelection::Endpoint { .. } => "endpoint",
            FrozenReferenceSelection::ThroughReference => "through-reference",
            FrozenReferenceSelection::Unavailable => "unavailable",
        };
        let rule;
        let mut resolved = if let Some(complete) = collect_completed_invocation(
            program,
            facts,
            state,
            capture,
            projection,
            output_access,
            output,
        ) {
            rule = "completed invocation did not supply permission";
            complete
        } else if let Some(complete) = collect_completed_output(
            program,
            facts,
            state,
            capture,
            projection,
            output_access,
            output,
        ) {
            rule = "completed output did not supply permission";
            complete
        } else if matches!(&frozen, FrozenReferenceSelection::Unavailable) {
            rule = "frozen reference selection is unavailable";
            false
        } else if collect_formed_projection(
            program,
            facts,
            state,
            capture,
            projection,
            output_access,
            output,
        ) {
            rule = "checked formation supplied permission";
            true
        } else if let checked_trees::FlowOperandCompletion::Selected(alternatives) =
            capture.completion
        {
            rule = "selected descriptor has an unresolved alternative";
            facts
                .flow
                .control
                .operand_alternatives
                .span(alternatives)
                .is_some_and(|rows| {
                    !rows.is_empty()
                        && rows.iter().all(|alternative| {
                            alternative.source_arm.is_valid()
                                && collect_supplier!(
                                    alternative.value,
                                    projection,
                                    retained_by_store
                                )
                        })
                })
        } else if let Some(complete) = collect_entry_invocation_accesses(
            program,
            facts,
            loan_occurrences,
            state,
            call,
            source,
            capture,
            projection,
            output_access,
            retained_by_store,
            output,
            parents,
        ) {
            rule = "the incoming descriptor has no complete invocation access";
            complete
        } else if let Some(complete) = collect_completed_formations(
            program,
            facts,
            loan_occurrences,
            state,
            call,
            source,
            projection,
            output_access,
            retained_by_store,
            active,
            output,
            parents,
            failures,
        ) {
            rule = "completed descriptor formation is unresolved";
            complete
        } else if let Some(complete) = collect_descriptor_captures(
            program,
            facts,
            loan_occurrences,
            state,
            call,
            source,
            projection,
            output_access,
            retained_by_store,
            active,
            output,
            parents,
            failures,
        ) {
            rule = "saved descriptor capture is unresolved";
            complete
        } else if let FrozenReferenceSelection::Endpoint {
            owner,
            leaf,
            path: leaf_path,
        } = frozen
        {
            rule = "frozen descriptor has no matching stored loan or invocation access";
            if let Some(suppliers) = validation::completed_storage::descriptor_suppliers(
                &facts.flow.control,
                leaf.retention,
                &mut Vec::new(),
            ) {
                !suppliers.is_empty()
                    && suppliers.iter().all(|supplier| {
                        facts
                            .flow
                            .control
                            .operand_retained_paths
                            .span(supplier.projection)
                            .is_some_and(|projection| {
                                collect_supplier!(supplier.operand, projection, true)
                            })
                    })
            } else {
                let coverage = if projection.is_empty()
                    && validation::reference_shell(program, capture.type_reference).is_some()
                {
                    collect_invocation_accesses(
                        program,
                        facts,
                        loan_occurrences,
                        state,
                        call,
                        source,
                        capture,
                        output_access,
                        retained_by_store,
                        output,
                        parents,
                    )
                } else {
                    InvocationAccessCoverage::Absent
                };
                match coverage {
                    InvocationAccessCoverage::Complete => true,
                    InvocationAccessCoverage::Unavailable => false,
                    InvocationAccessCoverage::Absent => {
                        if facts
                            .flow
                            .control
                            .operand_referents
                            .span(leaf.referents)
                            .is_some_and(|referents| {
                                !referents.is_empty()
                                    && referents.iter().all(|referent| {
                                        facts
                                            .flow
                                            .control
                                            .referent_formations
                                            .is_valid(referent.formation)
                                            && facts
                                                .flow
                                                .control
                                                .referent_formations
                                                .get(referent.formation)
                                                .owner
                                                .is_valid()
                                    })
                            })
                            && let Some(source_place) = place_at(
                                program,
                                owner.state_symbol,
                                owner.statement_index,
                                owner.expression,
                            )
                        {
                            let mut source_path =
                                owner_path_from_place_segments(program, &source_place.segments);
                            source_path.extend(owner_path_from_place_segments(program, &leaf_path));
                            for loan in facts.flow.borrow_loan_constraints(leaf.constraints) {
                                let row = facts.borrow.loans.get(loan);
                                if matches!(
                                    row.lineage,
                                    checked_trees::BorrowLoanLineage::LifetimeDependency
                                ) || !facts.borrow.state_owns_loan(borrow_state, loan)
                                    || row.storage_owner != source_place.root
                                    || facts.borrow.loan_owner_path(row) != source_path
                                {
                                    continue;
                                }
                                push_stored_loan!(loan, row);
                            }
                            output.len() > before
                        } else {
                            false
                        }
                    }
                }
            }
        } else if let Some(suppliers) = retained_suppliers {
            rule = "retained descriptor supplier is unresolved";
            !suppliers.is_empty()
                && suppliers.iter().all(|supplier| {
                    collect_supplier!(
                        supplier.operand,
                        match facts
                            .flow
                            .control
                            .operand_retained_paths
                            .span(supplier.projection)
                        {
                            Some(projection) => projection,
                            None => return false,
                        },
                        true,
                    )
                })
        } else if let Some(complete) = collect_reborrow_sources(
            program,
            facts,
            loan_occurrences,
            state,
            call,
            source,
            projection,
            output_access,
            active,
            output,
            parents,
            failures,
        ) {
            rule = "retained reborrow projection is unresolved";
            complete
        } else if collect_binding_observation(
            program,
            facts,
            state,
            capture,
            projection,
            output_access,
            output,
        ) {
            rule = "binding observation supplied permission";
            true
        } else {
            rule = "no exact invocation access supplies this descriptor";
            match collect_invocation_accesses(
                program,
                facts,
                loan_occurrences,
                state,
                call,
                source,
                capture,
                output_access,
                retained_by_store,
                output,
                parents,
            ) {
                InvocationAccessCoverage::Complete => true,
                InvocationAccessCoverage::Unavailable => false,
                InvocationAccessCoverage::Absent => false,
            }
        };
        active.pop();
        if !resolved {
            if ::validation::trace_flags::struct_trace() {
                eprintln!(
                    "STRUCT descriptor-permission-unavailable source={source:?} expression={:?} completion={:?} permission={:?} path={projection:?} frozen={frozen_kind}",
                    capture.expression, capture.completion, capture.descriptor_permission
                );
            }
            output.truncate(before);
            if plain_observation
                && collect_binding_observation(
                    program,
                    facts,
                    state,
                    capture,
                    projection,
                    output_access,
                    output,
                )
            {
                resolved = true;
            }
        }
        if resolved {
            failures.kept = failure_start.0;
            failures.total = failure_start.1;
        } else {
            failures.record(source, Some(capture), projection, None, rule);
        }
        resolved
    }

    /* Match a checked projection's result presentation, without granting access. */
    fn validate_reborrow_projection_type(
        program: &dyn validation::CheckingInput,
        source: typed_trees::types::TypeReferenceHandle,
        projection: &[facts::PlaceSegment],
        expected: typed_trees::types::TypeReferenceHandle,
    ) -> Result<(), &'static str> {
        let Some((selector, prefix)) = projection.split_last() else {
            return if validation::storage_origin_types_match(program, source, expected) {
                Ok(())
            } else {
                Err("unprojected reference carrier differs from its result")
            };
        };
        if matches!(
            selector,
            facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange
        ) {
            let expected = validation::peel_declared_constraints(program, expected);
            let typed_trees::types::TypeReferenceNode::Slice { element_type } =
                program.type_reference_table.type_reference(expected)
            else {
                return Err("retained window result is not a slice");
            };
            let collection = validation::owned_projection_type_reference(program, source, prefix)
                .ok_or("retained window prefix has no owned source type")?;
            let element = validation::collection_element_type_reference(program, collection)
                .ok_or("retained window source has no collection element")?;
            return if validation::storage_origin_types_match(program, element, *element_type) {
                Ok(())
            } else {
                Err("retained window element differs from its result element")
            };
        }
        let selected = validation::owned_projection_type_reference(program, source, projection)
            .ok_or("retained element projection has no owned source type")?;
        if validation::storage_origin_types_match(program, selected, expected) {
            Ok(())
        } else {
            Err("retained element carrier differs from its result")
        }
    }

    /*
        A helper may save a narrower view of an evaluated argument. Its source
        path selects the original descriptor; only the separate suffix projects
        into that descriptor's data. Authenticate the original permission before
        composing this checked projection, and preserve every source alternative.
    */
    #[allow(clippy::too_many_arguments)]
    fn collect_reborrow_sources(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        call: Handle<FlowCallFact>,
        source: Handle<FlowOperandFact>,
        projection: &[facts::PlaceSegment],
        access: &checked_trees::BorrowAccessKind,
        active: &mut Vec<(Handle<FlowOperandFact>, Vec<facts::PlaceSegment>)>,
        output: &mut Vec<PermissionSupplier>,
        parents: &mut Arena<Handle<BorrowLoanFact>>,
        failures: &mut PermissionFailures,
    ) -> Option<bool> {
        let control = &facts.flow.control;
        let capture = control.operands.get(source);
        if !control.operand_retentions.is_valid(capture.retention) {
            return None;
        }
        let profile = control.operand_retentions.get(capture.retention);
        if profile.reborrow_sources.is_empty() {
            return None;
        }
        let mut resolve = || -> Result<Vec<(Handle<FlowOperandFact>, Vec<facts::PlaceSegment>, Vec<facts::PlaceSegment>)>, &'static str> {
            if !projection.is_empty() {
                return Err("retained reborrow does not select a whole descriptor");
            }
            let result = validation::reference_shell(program, capture.type_reference)
                .ok_or("retained reborrow result is not a reference")?;
            checked_trees::BorrowAccessKind::from(result.access).direct_reborrow_effect(access)
                .ok_or("retained reborrow result cannot supply the requested access")?;
            let sources = control.operand_reborrow_sources.span(profile.reborrow_sources)
                .ok_or("retained reborrow has no complete source roster")?;
            let mut selected = Vec::new();
            for source in sources {
                if !control.operands.is_valid(source.operand) { return Err("retained reborrow source capture is unavailable"); }
                let value = control.operands.get(source.operand);
                let path = control.operand_retained_paths.span(source.source_path)
                    .ok_or("retained reborrow source path is unavailable")?;
                let reference = validation::owned_projection_type_reference(program, value.type_reference, path)
                    .ok_or("retained reborrow source path has no owned type")?;
                let reference = validation::reference_shell(program, reference)
                    .ok_or("retained reborrow source path does not select a descriptor")?;
                checked_trees::BorrowAccessKind::from(reference.access).direct_reborrow_effect(access)
                    .ok_or("retained reborrow source cannot supply the requested access")?;
                let suffix = validation::corresponding_place_projection(
                    program, reference.referee, control.operand_retained_paths.span(source.projection)
                        .ok_or("retained reborrow suffix is unavailable")?,
                ).ok_or("retained reborrow suffix has no exact declaration correspondence")?;
                validate_reborrow_projection_type(program, reference.referee, &suffix, result.referee)
                    .inspect_err(|&reason| {
                        failures.record(source.operand, Some(value), &suffix, Some(result.referee), reason);
                    })?;
                selected.push((source.operand, path.to_vec(), suffix));
            }
            if selected.is_empty() { Err("retained reborrow selected no source") } else { Ok(selected) }
        };
        let selected = match resolve() {
            Ok(selected) => selected,
            Err(reason) => {
                failures.record(
                    source,
                    Some(capture),
                    projection,
                    Some(capture.type_reference),
                    reason,
                );
                return Some(false);
            }
        };
        let before = output.len();
        for (source, path, suffix) in selected {
            let start = output.len();
            if !collect(
                program,
                facts,
                loan_occurrences,
                state,
                call,
                source,
                &path,
                access,
                true,
                active,
                output,
                parents,
                failures,
            ) {
                output.truncate(before);
                return Some(false);
            }
            for supplier in &mut output[start..] {
                let Some(projected) =
                    facts::project_captured_view_segments(&supplier.backing.segments, &suffix)
                else {
                    failures.record(
                        source,
                        Some(control.operands.get(source)),
                        &suffix,
                        Some(capture.type_reference),
                        "retained suffix does not compose with its captured source boundary",
                    );
                    output.truncate(before);
                    return Some(false);
                };
                supplier.backing.segments = projected;
            }
        }
        Some(output.len() > before)
    }

    /*
        A pointer installed by a helper carries its completed output permission.
        This link belongs to the produced value, so replacing its former variable
        cannot retarget it. Input lifetime debts never enter this permission route.
    */
    fn collect_completed_output(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        capture: &FlowOperandFact,
        projection: &[facts::PlaceSegment],
        access: &checked_trees::BorrowAccessKind,
        output: &mut Vec<PermissionSupplier>,
    ) -> Option<bool> {
        if !matches!(
            capture.descriptor_permission,
            checked_trees::FlowDescriptorPermission::SuppliedOutput { .. }
        ) {
            return None;
        }
        let complete = (|| -> Result<PermissionSupplier, &'static str> {
            if !projection.is_empty() || capture.state_symbol != state.state_symbol {
                return Err("the output permission does not describe this whole captured leaf");
            }
            let loan = crate::borrow::supplied_outputs::captured_permission(
                program,
                &facts.borrow,
                &facts.flow,
                capture,
            )?;
            let row = facts.borrow.loans.get(loan);
            let incoming = facts.borrow.incoming_references.get(row.incoming_reference);
            if incoming.machine_symbol != state.machine_symbol
                || row.kind.direct_reborrow_effect(access).is_none()
            {
                return Err("the output permission cannot lend this state or access");
            }
            Ok(PermissionSupplier {
                source: CheckedCallBorrowLoanSource::StoredLoan { loan },
                actual: Handle::invalid(),
                backing: checked_trees::CapturedPlace {
                    incoming_reference: row.incoming_reference,
                    incoming_owner_path: facts
                        .borrow
                        .access_segments
                        .span(row.incoming_owner_path)
                        .ok_or("the output permission lost its incoming path")?
                        .to_vec(),
                    root: row.root,
                    segments: facts.borrow.loan_segments(row).to_vec(),
                },
            })
        })();
        Some(match complete {
            Ok(supplier) => {
                output.push(supplier);
                true
            }
            Err(reason) => {
                if ::validation::trace_flags::struct_trace() {
                    eprintln!("STRUCT supplied-descriptor-permission-refused {reason}");
                }
                false
            }
        })
    }

    /*
        A helper can return a saved reference without making a new borrow. Follow
        that exact descriptor even when the whole reference, rather than a field,
        is selected. Its lifetime sources alone cannot authorize the next view.
    */
    #[allow(clippy::too_many_arguments)]
    fn collect_descriptor_captures(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        call: Handle<FlowCallFact>,
        source: Handle<FlowOperandFact>,
        projection: &[facts::PlaceSegment],
        access: &checked_trees::BorrowAccessKind,
        retained_by_store: bool,
        active: &mut Vec<(Handle<FlowOperandFact>, Vec<facts::PlaceSegment>)>,
        output: &mut Vec<PermissionSupplier>,
        parents: &mut Arena<Handle<BorrowLoanFact>>,
        failures: &mut PermissionFailures,
    ) -> Option<bool> {
        let capture = facts.flow.control.operands.get(source);
        let selected = validation::completed_storage::reference_captures_at_path(
            program,
            &facts.flow.control,
            source,
            projection,
            capture.storage_snapshot,
        )?;
        let selection_kind = match &selected {
            validation::completed_storage::StoredOperand::Unavailable => "unavailable",
            validation::completed_storage::StoredOperand::Incoming(_) => "incoming",
            validation::completed_storage::StoredOperand::Alternatives(_) => "alternatives",
            validation::completed_storage::StoredOperand::Value { .. } => "value",
        };
        let Some(selected) = selected.captures() else {
            if ::validation::trace_flags::struct_trace() {
                eprintln!(
                    "STRUCT descriptor-capture-frontier-unavailable source={source:?} path={projection:?} kind={selection_kind} capture={capture:?}"
                );
                trace_descriptor_storage(program, facts, capture, projection);
            }
            return Some(false);
        };
        if projection.is_empty()
            && selected.len() == 1
            && selected[0].capture == source
            && selected[0].projection.is_empty()
        {
            return None;
        }
        Some(
            !selected.is_empty()
                && selected.into_iter().all(|selected| {
                    if selected.capture == source && selected.projection == projection {
                        return collect_formed_projection(
                            program, facts, state, capture, projection, access, output,
                        );
                    }
                    collect(
                        program,
                        facts,
                        loan_occurrences,
                        state,
                        call,
                        selected.capture,
                        &selected.projection,
                        access,
                        retained_by_store,
                        active,
                        output,
                        parents,
                        failures,
                    )
                }),
        )
    }

    fn trace_descriptor_storage(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        capture: &FlowOperandFact,
        projection: &[facts::PlaceSegment],
    ) {
        let Some(mut place) = place_at(
            program,
            capture.state_symbol,
            capture.statement_index,
            capture.expression,
        ) else {
            return;
        };
        place.segments.extend_from_slice(projection);
        let root_type = validation::canonical_place_type_reference(
            program,
            capture.state_symbol,
            capture.statement_index,
            &validation::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        );
        eprintln!("STRUCT descriptor-storage-query place={place:?} root_type={root_type:?}");
        let mut pending = vec![capture.storage_snapshot];
        let mut visited = Vec::new();
        while let Some(snapshot) = pending.pop() {
            if visited.contains(&snapshot)
                || !facts.flow.control.storage_snapshots.is_valid(snapshot)
            {
                continue;
            }
            visited.push(snapshot);
            let event = facts.flow.control.storage_snapshots.get(snapshot);
            match event.operation {
                checked_trees::FlowStorageOperation::Store {
                    destination,
                    value,
                    projection,
                } if destination.root == place.root => {
                    eprintln!(
                        "STRUCT descriptor-storage-store at={snapshot:?} root={:?} path={:?} value={value:?} value_type={:?} permission={:?} projection={:?}",
                        destination.root,
                        facts
                            .flow
                            .control
                            .operand_referent_segments
                            .span(destination.segments),
                        facts.flow.control.operands.is_valid(value).then(|| facts
                            .flow
                            .control
                            .operands
                            .get(value)
                            .type_reference),
                        facts.flow.control.operands.is_valid(value).then(|| facts
                            .flow
                            .control
                            .operands
                            .get(value)
                            .descriptor_permission),
                        facts.flow.control.operand_retained_paths.span(projection)
                    );
                }
                checked_trees::FlowStorageOperation::Writes {
                    bindings,
                    contents,
                    bindings_complete,
                    contents_complete,
                } => {
                    for (kind, rows, complete) in [
                        ("binding", bindings, bindings_complete),
                        ("contents", contents, contents_complete),
                    ] {
                        let writes = facts
                            .flow
                            .control
                            .operand_origin_writes
                            .span_or_empty(rows)
                            .iter()
                            .filter(|write| write.root == place.root)
                            .map(|write| {
                                facts
                                    .flow
                                    .control
                                    .operand_referent_segments
                                    .span(write.segments)
                            })
                            .collect::<Vec<_>>();
                        if !writes.is_empty() || !complete {
                            eprintln!(
                                "STRUCT descriptor-storage-writes at={snapshot:?} kind={kind} complete={complete} paths={writes:?}"
                            );
                        }
                    }
                }
                checked_trees::FlowStorageOperation::Join { predecessors } => pending
                    .extend_from_slice(
                        facts
                            .flow
                            .control
                            .storage_predecessors
                            .span_or_empty(predecessors),
                    ),
                _ => {}
            }
            pending.push(event.previous);
        }
    }

    /*
        Resolve the installed descriptor generation before joining a loan. A later
        read has a different expression from its initializer; the completed-storage
        relation carries that exact earlier formation without rereading its slot.
    */
    #[allow(clippy::too_many_arguments)]
    fn collect_completed_formations(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        call: Handle<FlowCallFact>,
        source: Handle<FlowOperandFact>,
        projection: &[facts::PlaceSegment],
        access: &checked_trees::BorrowAccessKind,
        retained_by_store: bool,
        active: &mut Vec<(Handle<FlowOperandFact>, Vec<facts::PlaceSegment>)>,
        output: &mut Vec<PermissionSupplier>,
        parents: &mut Arena<Handle<BorrowLoanFact>>,
        failures: &mut PermissionFailures,
    ) -> Option<bool> {
        let selected = validation::completed_storage::predecessors_in_control(
            program,
            &facts.flow.control,
            source,
            projection,
            validation::completed_storage::StoredObservation::Binding,
            Some((state.state_symbol, state.statements)),
        )?;
        if selected.is_empty() {
            return Some(false);
        }
        let descriptors = selected
            .iter()
            .map(|(capture, projection)| {
                facts.flow.control.operands.is_valid(*capture)
                    && validation::owned_projection_type_reference(
                        program,
                        facts.flow.control.operands.get(*capture).type_reference,
                        projection,
                    )
                    .is_some_and(|selected| {
                        validation::reference_shell(program, selected).is_some()
                    })
            })
            .collect::<Vec<_>>();
        if descriptors.iter().all(|descriptor| !descriptor) {
            return None;
        }
        if descriptors.iter().any(|descriptor| !descriptor) {
            return Some(false);
        }
        let before = output.len();
        for (selected, projection) in selected {
            if !collect(
                program,
                facts,
                loan_occurrences,
                state,
                call,
                selected,
                &projection,
                access,
                retained_by_store,
                active,
                output,
                parents,
                failures,
            ) {
                if ::validation::trace_flags::struct_trace() {
                    eprintln!(
                        "STRUCT descriptor-predecessor-permission-unavailable source={source:?} predecessor={selected:?} path={projection:?}"
                    );
                }
                output.truncate(before);
                return Some(false);
            }
        }
        Some(true)
    }

    /*
        Exact checked access occurrences are permission evidence for a whole
        evaluated descriptor, including one projected from an earlier owned copy.
        A failed present roster cannot recover an older source-slot formation.
    */
    enum InvocationAccessCoverage {
        Absent,
        Complete,
        Unavailable,
    }

    /*
        A completed expression can name an earlier evaluation of the same entry
        descriptor. Keep the actual argument's checked access before following
        that earlier value, which need not itself be an invocation argument.
        The captured snapshot must still select the exact incoming reference leaf;
        the enclosing record's borrow cannot supply its stored pointer permission.
    */
    #[allow(clippy::too_many_arguments)]
    fn collect_entry_invocation_accesses(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        call: Handle<FlowCallFact>,
        source: Handle<FlowOperandFact>,
        capture: &FlowOperandFact,
        projection: &[facts::PlaceSegment],
        output_access: &checked_trees::BorrowAccessKind,
        retained_by_store: bool,
        output: &mut Vec<PermissionSupplier>,
        parents: &mut Arena<Handle<BorrowLoanFact>>,
    ) -> Option<bool> {
        if !projection.is_empty()
            || !has_captured_entry_reference(program, facts, loan_occurrences, state, source)
        {
            return None;
        }
        match collect_invocation_accesses(
            program,
            facts,
            loan_occurrences,
            state,
            call,
            source,
            capture,
            output_access,
            retained_by_store,
            output,
            parents,
        ) {
            InvocationAccessCoverage::Absent => None,
            InvocationAccessCoverage::Complete => Some(true),
            InvocationAccessCoverage::Unavailable => Some(false),
        }
    }

    fn collect_invocation_accesses(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        call: Handle<FlowCallFact>,
        source: Handle<FlowOperandFact>,
        capture: &FlowOperandFact,
        output_access: &checked_trees::BorrowAccessKind,
        retained_by_store: bool,
        output: &mut Vec<PermissionSupplier>,
        parents: &mut Arena<Handle<BorrowLoanFact>>,
    ) -> InvocationAccessCoverage {
        let unavailable = |reason| {
            if ::validation::trace_flags::struct_trace() {
                eprintln!(
                    "STRUCT captured-call-access-refused source={source:?} call={call:?} expression={:?} reason={reason}",
                    capture.expression
                );
            }
            InvocationAccessCoverage::Unavailable
        };
        let call = if retained_by_store {
            match super::super::formation_captures::actual_invocation(program, facts, state, source)
            {
                Ok(Some(original)) => original,
                Ok(None) => call,
                Err(reason) => return unavailable(reason),
            }
        } else {
            call
        };
        if !facts.flow.control.calls.is_valid(call) {
            return InvocationAccessCoverage::Absent;
        }
        if !capture.expression.is_valid() {
            return InvocationAccessCoverage::Absent;
        }
        match super::super::formation_captures::actual_invocation(program, facts, state, source) {
            Ok(Some(owner)) if owner == call => {}
            Ok(_) => return InvocationAccessCoverage::Absent,
            Err(reason) => return unavailable(reason),
        }
        let Some(presentation) = validation::reference_shell(program, capture.type_reference)
        else {
            return InvocationAccessCoverage::Absent;
        };
        if checked_trees::BorrowAccessKind::from(presentation.access)
            .direct_reborrow_effect(output_access)
            .is_none()
        {
            return unavailable(
                "the captured descriptor's presented access cannot supply this request",
            );
        }
        let formation =
            match super::super::formation_captures::reference_formation(program, facts, source) {
                Ok(formation) => formation,
                Err(reason) => return unavailable(reason),
            };
        match super::super::formation_captures::argument_formation(program, facts, state, source) {
            Ok(Some(owner)) if owner.storage && owner.value != source => {
                return InvocationAccessCoverage::Absent;
            }
            Ok(_) => {}
            Err(reason) => return unavailable(reason),
        }
        let target_expression = formation
            .as_ref()
            .map(|formation| facts.flow.control.operands.get(formation.target).expression);
        let invocation = facts.flow.control.calls.get(call);
        let accesses = invocation
            .accesses
            .handles()
            .filter(|access| {
                let access = facts.borrow.argument_accesses.get(*access);
                (access.expression == capture.expression
                    || Some(access.expression) == target_expression)
                    && access.kind.direct_reborrow_effect(output_access).is_some()
                    && (retained_by_store || access.end_before_access.is_none())
            })
            .collect::<Vec<_>>();
        if accesses.is_empty() {
            return InvocationAccessCoverage::Absent;
        }
        let before = output.len();
        for access in accesses {
            let row = facts.borrow.argument_accesses.get(access);
            let backings = if let Some(formation) = &formation
                && formation.storage
            {
                match super::super::access_sources::stored_address(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    formation.target,
                    true,
                ) {
                    Ok(backings) => Some(backings),
                    Err(reason) => {
                        output.truncate(before);
                        return unavailable(reason);
                    }
                }
            } else {
                retained_places(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    capture.statement_index,
                    capture.constraints,
                    checked_trees::CapturedPlace {
                        root: row.root,
                        segments: facts.borrow.access_segments(row).to_vec(),
                        ..Default::default()
                    },
                    &mut Vec::new(),
                )
            };
            let Some(backings) = backings else {
                output.truncate(before);
                return InvocationAccessCoverage::Unavailable;
            };
            if backings.is_empty() {
                output.truncate(before);
                return InvocationAccessCoverage::Unavailable;
            }
            for backing in backings {
                output.push(PermissionSupplier {
                    source: CheckedCallBorrowLoanSource::CallAccess {
                        access,
                        parents: parents.insert_many(backing.lineage),
                    },
                    actual: source,
                    backing: backing.place,
                });
            }
        }
        InvocationAccessCoverage::Complete
    }

    /*
        Stop at the checked borrow that created this exact reference. Copies and
        selections create no permission and must follow every saved predecessor.
        Public backing regions and enclosing storage cannot authorize this event.
    */
    #[allow(clippy::too_many_arguments)]
    fn collect_formed_projection(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        capture: &FlowOperandFact,
        projection: &[facts::PlaceSegment],
        output_access: &checked_trees::BorrowAccessKind,
        output: &mut Vec<PermissionSupplier>,
    ) -> bool {
        if capture.state_symbol != state.state_symbol
            || !capture.expression.is_valid()
            || projection.iter().any(|segment| {
                !matches!(
                    segment,
                    facts::PlaceSegment::Field { .. }
                        | facts::PlaceSegment::Case { .. }
                        | facts::PlaceSegment::FixedIndex { .. }
                )
            })
        {
            return false;
        }
        let Some(selected) = validation::owned_projection_type_reference(
            program,
            capture.type_reference,
            projection,
        )
        .and_then(|selected| validation::reference_shell(program, selected)) else {
            return false;
        };
        if !matches!(
            (selected.access, output_access),
            (language_semantics::ReferenceAccess::Mutable, _)
                | (
                    language_semantics::ReferenceAccess::Shared,
                    checked_trees::BorrowAccessKind::Read
                )
                | (
                    language_semantics::ReferenceAccess::WriteOnly,
                    checked_trees::BorrowAccessKind::WriteOnly
                )
        ) {
            return false;
        }
        let Some((_, borrow_state)) = facts
            .borrow
            .first_state_borrow(state.machine_symbol, state.state_symbol)
        else {
            return false;
        };
        let before = output.len();
        for (loan, row) in facts.borrow.state_loans(borrow_state) {
            let Some(formed) = row.formation_owner_path else {
                continue;
            };
            if !matches!(
                row.lineage,
                checked_trees::BorrowLoanLineage::DirectRoot
                    | checked_trees::BorrowLoanLineage::Reborrow { .. }
            ) || row.formation == checked_trees::BorrowLoanFormation::ReturnedDescriptor
                || row.kind.direct_reborrow_effect(output_access).is_none()
                || row.formation_expression != capture.expression
                || row.statement_index != capture.statement_index
                || owner_place_segments(facts.borrow.owner_segments.span_or_empty(formed))
                    != projection
            {
                continue;
            }
            output.push(PermissionSupplier {
                source: CheckedCallBorrowLoanSource::StoredLoan { loan },
                actual: Handle::invalid(),
                backing: checked_trees::CapturedPlace {
                    incoming_reference: row.incoming_reference,
                    incoming_owner_path: facts
                        .borrow
                        .access_segments
                        .span_or_empty(row.incoming_owner_path)
                        .to_vec(),
                    root: row.root,
                    segments: facts.borrow.loan_segments(row).to_vec(),
                },
            });
        }
        output.len() > before
    }

    /*
        Reading a reference binding uses the loan that binding owns. The descriptor
        may be stored in the referent, so a storage-owner match on the binding
        place does not see it. The lexical owner is the association the checker
        already recorded.
    */
    fn collect_binding_observation(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        capture: &FlowOperandFact,
        projection: &[facts::PlaceSegment],
        output_access: &checked_trees::BorrowAccessKind,
        output: &mut Vec<PermissionSupplier>,
    ) -> bool {
        if !projection.is_empty()
            || !matches!(
                capture.completion,
                checked_trees::FlowOperandCompletion::Observation
            )
        {
            return false;
        }
        let Some(place) = place_at(
            program,
            state.state_symbol,
            capture.statement_index,
            capture.expression,
        ) else {
            return false;
        };
        if !place.segments.is_empty() {
            return false;
        }
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return false;
        };
        let Some((_, borrow_state)) = facts
            .borrow
            .first_state_borrow(state.machine_symbol, state.state_symbol)
        else {
            return false;
        };
        let before = output.len();
        for (loan, row) in facts.borrow.state_loans(borrow_state) {
            if row.owner_symbol != symbol
                || row.statement_index > capture.statement_index
                || (row.last_use_statement_index != 0
                    && row.last_use_statement_index < capture.statement_index)
                || matches!(
                    row.lineage,
                    checked_trees::BorrowLoanLineage::LifetimeDependency
                )
                || row.kind.direct_reborrow_effect(output_access).is_none()
            {
                continue;
            }
            output.push(PermissionSupplier {
                source: CheckedCallBorrowLoanSource::StoredLoan { loan },
                actual: Handle::invalid(),
                backing: checked_trees::CapturedPlace {
                    incoming_reference: row.incoming_reference,
                    incoming_owner_path: facts
                        .borrow
                        .access_segments
                        .span_or_empty(row.incoming_owner_path)
                        .to_vec(),
                    root: row.root,
                    segments: facts.borrow.loan_segments(row).to_vec(),
                },
            });
        }
        output.len() > before
    }

    /*
        An evaluated provider result supplies its own reference permission. The
        invocation's input lifetime debts cannot replace that exact result field.
    */
    fn collect_completed_invocation(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        capture: &FlowOperandFact,
        projection: &[facts::PlaceSegment],
        output_access: &checked_trees::BorrowAccessKind,
        output: &mut Vec<PermissionSupplier>,
    ) -> Option<bool> {
        let control = &facts.flow.control;
        let (invocation, result_type, path) = match capture.completion {
            checked_trees::FlowOperandCompletion::InvocationResult {
                invocation,
                result_type,
                path,
            } => {
                if !projection.is_empty() {
                    return Some(false);
                }
                let path = control.operand_retained_paths.span(path)?;
                (invocation, result_type, path)
            }
            _ if matches!(
                program.expression_table.expression(capture.expression),
                typed_trees::expression::ExpressionNode::Call(_)
            ) =>
            {
                let mut calls = state.calls.handles().filter(|handle| {
                    let call = control.calls.get(*handle);
                    call.authored_expression == capture.expression
                        && call.statement_index == capture.statement_index
                });
                let invocation = calls.next()?;
                if calls.next().is_some() {
                    return Some(false);
                }
                (invocation, capture.type_reference, projection)
            }
            _ => return None,
        };
        if !control.calls.is_valid(invocation) {
            return Some(false);
        }
        let permission = super::invocation_permissions::resolve(
            program,
            facts,
            state,
            control.calls.get(invocation),
            capture,
            result_type,
            path,
        )?;
        let Ok(loan) = permission else {
            return Some(false);
        };
        let row = facts.borrow.loans.get(loan);
        if row.kind.direct_reborrow_effect(output_access).is_none() {
            return Some(false);
        }
        output.push(PermissionSupplier {
            source: CheckedCallBorrowLoanSource::StoredLoan { loan },
            actual: Handle::invalid(),
            backing: checked_trees::CapturedPlace {
                incoming_reference: row.incoming_reference,
                incoming_owner_path: facts
                    .borrow
                    .access_segments
                    .span_or_empty(row.incoming_owner_path)
                    .to_vec(),
                root: row.root,
                segments: facts.borrow.loan_segments(row).to_vec(),
            },
        });
        Some(true)
    }
}
mod reborrow_drafts {
    /*
        Draft rows for reborrow loan resources and disposition events.
    */
    use checked_trees::{
        BorrowFacts, BorrowLoanFact, CheckedBorrowResourceDispositionTarget,
        CheckedBorrowResourceLifecyclePhase, CheckedDirectBorrowLoanResource,
        CheckedParentBorrowResource as BorrowResource, CheckedReborrowAccessEffect,
        CheckedReborrowLoanResource as LoanResource, CheckedReborrowParentEndStatus,
        CheckedReborrowParentSuspensionBoundary, CheckedReborrowResourceDisposition,
        CheckedReborrowResourceDispositionEvent, CheckedReborrowRestorationObligation,
        CheckedRetiredParentResourceDispositionStep, FlowInvalidationSource,
        ParentLexicalStatusAtChildEnd,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CheckedReborrowLoanResourceDraft {
        pub(crate) loan: arena::Handle<BorrowLoanFact>,
        pub(crate) machine_symbol: symbols::SymbolHandle,
        pub(crate) state_symbol: symbols::SymbolHandle,
        pub(crate) owner_symbol: symbols::SymbolHandle,
        pub(crate) owner_path: Vec<checked_trees::BorrowLoanOwnerSegment>,
        pub(crate) captured_place: checked_trees::CapturedPlace,
        pub(crate) access: checked_trees::BorrowAccessKind,
        pub(crate) parent_access: checked_trees::BorrowAccessKind,
        pub(crate) access_effect: CheckedReborrowAccessEffect,
        pub(crate) activation_source: checked_trees::FlowInvalidationSource,
        pub(crate) retention: checked_trees::CheckedBorrowResourceRetention,
        pub(crate) source_loan: arena::Handle<BorrowLoanFact>,
        pub(crate) weakening_source: checked_trees::FlowInvalidationSource,
        pub(crate) weakening_reason: checked_trees::FlowBorrowWeakeningReason,
        pub(crate) parent_loan: arena::Handle<BorrowLoanFact>,
        pub(crate) child_activation: arena::Handle<checked_trees::FlowBorrowActivationFact>,
        pub(crate) parent_entry_constraint: arena::Handle<checked_trees::FlowConstraintRef>,
        pub(crate) child_weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
        pub(crate) parent_end: checked_trees::CheckedBorrowParentEnd,
        pub(crate) parent_lexical_status: ParentLexicalStatusAtChildEnd,
    }

    impl CheckedReborrowLoanResourceDraft {
        pub(crate) fn close(&self, parent_resource: BorrowResource) -> LoanResource {
            LoanResource {
                loan: self.loan,
                machine_symbol: self.machine_symbol,
                state_symbol: self.state_symbol,
                owner_symbol: self.owner_symbol,
                owner_path: self.owner_path.clone(),
                captured_place: self.captured_place.clone(),
                access: self.access.clone(),
                parent_access: self.parent_access.clone(),
                access_effect: self.access_effect,
                activation_source: self.activation_source,
                retention: self.retention.clone(),
                weakening_source: self.weakening_source,
                weakening_reason: self.weakening_reason,
                parent_loan: self.parent_loan,
                parent_resource: parent_resource.clone(),
                parent_suspension: CheckedReborrowParentSuspensionBoundary {
                    child_loan: self.loan,
                    parent_loan: self.parent_loan,
                    parent_resource: parent_resource.clone(),
                    child_activation: self.child_activation,
                    parent_entry_constraint: self.parent_entry_constraint,
                    source_loan: self.source_loan,
                    source: self.activation_source,
                },
                parent_end_status: CheckedReborrowParentEndStatus {
                    child_loan: self.loan,
                    parent_loan: self.parent_loan,
                    parent_resource: parent_resource.clone(),
                    child_weakening: self.child_weakening,
                    parent_end: self.parent_end,
                    status: self.parent_lexical_status,
                },
                restoration: CheckedReborrowRestorationObligation {
                    child_loan: self.loan,
                    parent_loan: self.parent_loan,
                    parent_resource,
                    child_weakening_source: self.weakening_source,
                    child_weakening_reason: self.weakening_reason,
                },
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ParentResourceIndex {
        Direct(usize),
        Reborrow(usize),
        Supplied(usize),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum DispositionTargetIndex {
        ParentResource(ParentResourceIndex),
        DirectRootLifetime(usize),
        CompletedOutput(usize),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CheckedReborrowDispositionEventDraft {
        pub(crate) machine_symbol: symbols::SymbolHandle,
        pub(crate) state_symbol: symbols::SymbolHandle,
        pub(crate) child_loan: arena::Handle<BorrowLoanFact>,
        pub(crate) child_resource: usize,
        pub(crate) child_activation: arena::Handle<checked_trees::FlowBorrowActivationFact>,
        pub(crate) child_weakening: arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
        pub(crate) parent_loan: arena::Handle<BorrowLoanFact>,
        pub(crate) parent_resource: ParentResourceIndex,
        pub(crate) boundary_source: FlowInvalidationSource,
        pub(crate) boundary_phase: CheckedBorrowResourceLifecyclePhase,
        pub(crate) shared_cohort: Vec<usize>,
        pub(crate) retired_parent_path: Vec<(
            ParentResourceIndex,
            arena::Handle<checked_trees::FlowBorrowWeakeningFact>,
        )>,
        pub(crate) final_target: DispositionTargetIndex,
        pub(crate) disposition: CheckedReborrowResourceDisposition,
        /*
            Exact post-restoration mutating-call join; the three stay absent unless
            `bind_restored_call_uses` certifies this event's boundary call.
        */
        pub(crate) restored_call: arena::Handle<checked_trees::FlowCallFact>,
        pub(crate) restored_borrow_call: arena::Handle<checked_trees::BorrowCallFact>,
        pub(crate) restored_call_access: arena::Handle<checked_trees::BorrowArgumentAccessFact>,
    }

    impl CheckedReborrowDispositionEventDraft {
        pub(crate) fn close(
            &self,
            borrow: &BorrowFacts,
            handles: &ResourceHandles,
        ) -> CheckedReborrowResourceDispositionEvent {
            CheckedReborrowResourceDispositionEvent {
                machine_symbol: self.machine_symbol,
                state_symbol: self.state_symbol,
                child_loan: self.child_loan,
                child_resource: handles.reborrows[self.child_resource],
                child_activation: self.child_activation,
                child_weakening: self.child_weakening,
                parent_loan: self.parent_loan,
                parent_resource: handles.parent(self.parent_resource),
                boundary_source: self.boundary_source,
                boundary_phase: self.boundary_phase,
                shared_cohort: self
                    .shared_cohort
                    .iter()
                    .map(|index| handles.reborrows[*index])
                    .collect(),
                retired_parent_path: self
                    .retired_parent_path
                    .iter()
                    .map(
                        |(resource, weakening)| CheckedRetiredParentResourceDispositionStep {
                            resource: handles.parent(*resource),
                            weakening: *weakening,
                        },
                    )
                    .collect(),
                final_target: match self.final_target {
                    DispositionTargetIndex::ParentResource(resource) => {
                        CheckedBorrowResourceDispositionTarget::ParentResource(
                            handles.parent(resource),
                        )
                    }
                    DispositionTargetIndex::DirectRootLifetime(index) => {
                        CheckedBorrowResourceDispositionTarget::DirectRootLifetime(
                            borrow
                                .direct_loan_resources
                                .get(handles.direct[index])
                                .parent_lifetime
                                .clone(),
                        )
                    }
                    DispositionTargetIndex::CompletedOutput(index) => {
                        CheckedBorrowResourceDispositionTarget::CompletedOutput(
                            handles.supplied[index],
                        )
                    }
                },
                disposition: self.disposition,
                restored_call: self.restored_call,
                restored_borrow_call: self.restored_borrow_call,
                restored_call_access: self.restored_call_access,
            }
        }
    }

    #[derive(Debug)]
    pub(crate) struct ResourceHandles {
        pub(crate) direct: Vec<arena::Handle<CheckedDirectBorrowLoanResource>>,
        pub(crate) reborrows: Vec<arena::Handle<LoanResource>>,
        pub(crate) supplied: Vec<arena::Handle<checked_trees::CheckedSuppliedOutputBorrowResource>>,
    }

    impl ResourceHandles {
        fn parent(&self, index: ParentResourceIndex) -> BorrowResource {
            match index {
                ParentResourceIndex::Direct(index) => BorrowResource::DirectRoot {
                    resource: self.direct[index],
                },
                ParentResourceIndex::Reborrow(index) => BorrowResource::Reborrow {
                    resource: self.reborrows[index],
                },
                ParentResourceIndex::Supplied(index) => BorrowResource::SuppliedOutput {
                    resource: self.supplied[index],
                },
            }
        }
    }
}
pub(in crate::checks::borrows) mod retained_backing {
    /*
        Find the original storage behind a saved reference, including references
        inside records. Resolve its completed storage and permission dependencies
        without creating access, restoration authority, or call destinations.
    */
    use crate::checks::borrows::access_sources::resolve_captured_all;
    use crate::checks::borrows::aliases::ResolvedAlias;
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use crate::flow::{captured_call_operands, expressions};
    use crate::published::places::place_at;
    use arena::{Handle, HandleSpan};
    use checked_trees::{CapturedPlace, CheckFacts, FlowCallFact, FlowOperandFact, FlowStateFact};

    /*
        Lifetime retention follows the captured contents of every stored reference
        crossing, just as a referent access does. Resolving only a reference local
        leaves `holder.reference_field` anchored to the holder instead of its
        referent. The original invocation access remains unchanged; each crossed
        stored loan joins the parent roster so suspension also retains its resource
        obligations. This does not change the alias checker's access exemptions.
    */
    pub(in crate::checks::borrows) fn retained_places(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        statement: usize,
        constraints: HandleSpan<checked_trees::FlowConstraintRef>,
        source: CapturedPlace,
        active_calls: &mut Vec<Handle<FlowCallFact>>,
    ) -> Option<Vec<ResolvedAlias>> {
        let typed_state = program.state_by_symbol(state.state_symbol)?;
        let mut pending = resolve_captured_all(
            program,
            facts.view(),
            loan_occurrences,
            state,
            statement,
            constraints,
            source,
        )?
        .into_iter()
        .map(|source| (source.place, source.lineage))
        .collect::<Vec<_>>();
        let mut roots = Vec::new();
        while let Some((place, ancestors)) = pending.pop() {
            if place.incoming_reference.is_valid() {
                roots.push(ResolvedAlias {
                    place,
                    lineage: ancestors,
                });
                continue;
            }
            if let Some(expression) = place.root.as_expression() {
                let mut calls = state.calls.handles().filter(|handle| {
                    let call = facts.flow.control.calls.get(*handle);
                    call.authored_expression == expression && call.statement_index == statement
                });
                if let Some(call) = calls.next() {
                    if calls.next().is_some() || active_calls.contains(&call) {
                        return None;
                    }
                    active_calls.push(call);
                    let returned = returned_places(
                        program,
                        facts,
                        loan_occurrences,
                        state,
                        call,
                        &place.segments,
                        constraints,
                        active_calls,
                    );
                    active_calls.pop();
                    for mut source in returned? {
                        for ancestor in &ancestors {
                            if !source.lineage.contains(ancestor) {
                                source.lineage.push(*ancestor);
                            }
                        }
                        roots.push(source);
                    }
                    continue;
                }
            }
            let mut crossed = false;
            for handle in facts.flow.borrow_loan_constraints(constraints) {
                let loan = facts.borrow.loans.get(handle);
                let destinations = crate::borrow::store_destination_places(
                    program,
                    typed_state,
                    place.clone(),
                    true,
                    std::iter::once((
                        loan.storage_owner,
                        facts.borrow.loan_owner_path(loan),
                        loan.incoming_reference,
                        facts
                            .borrow
                            .access_segments
                            .span_or_empty(loan.incoming_owner_path),
                        loan.root,
                        facts.borrow.loan_segments(loan),
                    )),
                );
                if destinations.as_slice() == [place.clone()] {
                    continue;
                }
                if ancestors.contains(&handle) {
                    return None;
                }
                crossed = true;
                let mut suppliers = ancestors.clone();
                suppliers.push(handle);
                pending.extend(
                    destinations
                        .into_iter()
                        .map(|place| (place, suppliers.clone())),
                );
            }
            if !crossed {
                for (incoming_reference, incoming) in facts.borrow.incoming_references.iter() {
                    if incoming.machine_symbol != state.machine_symbol
                        || incoming.state_symbol != state.state_symbol
                    {
                        continue;
                    }
                    let checked_trees::IncomingReferenceOrigin::Parameter { symbol } =
                        incoming.origin
                    else {
                        continue;
                    };
                    let root = facts::PlaceRoot::Symbol(symbol);
                    let destinations = crate::borrow::store_destination_places(
                        program,
                        typed_state,
                        place.clone(),
                        true,
                        std::iter::once((
                            root,
                            facts
                                .borrow
                                .owner_segments
                                .span_or_empty(incoming.owner_path),
                            incoming_reference,
                            &[][..],
                            root,
                            &[][..],
                        )),
                    );
                    if destinations.as_slice() != [place.clone()] {
                        crossed = true;
                        pending.extend(
                            destinations
                                .into_iter()
                                .map(|place| (place, ancestors.clone())),
                        );
                    }
                }
            }
            if !crossed {
                if !terminal_storage_is_resolved(program, typed_state, &place)? {
                    return None;
                }
                roots.push(ResolvedAlias {
                    place,
                    lineage: ancestors,
                });
            }
        }
        Some(roots)
    }

    /*
        The terminal footprint must not stop at an unresolved descriptor. Regions
        select storage, not one scalar value: the scalar projector deliberately
        cannot type them. Inspect the collection's complete element frontier instead,
        retaining the original region unchanged. A region containing references still
        needs their supplying loans; neither an incoming root nor a range can erase
        that obligation. Bounds and interpretation were checked at loan formation.
    */
    fn terminal_storage_is_resolved(
        program: &dyn validation::CheckingInput,
        state: &typed_trees::state::State,
        place: &CapturedPlace,
    ) -> Option<bool> {
        let (segments, region) = match place.segments.split_last() {
            Some((
                facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange,
                prefix,
            )) => (prefix, true),
            _ => (place.segments.as_slice(), false),
        };
        let selected_type = validation::canonical_place_type_reference(
            program,
            state.symbol,
            state.statement_nodes.count() as usize,
            &validation::CanonicalPlace {
                root: place.root,
                segments: segments.to_vec(),
            },
        )?;
        let incoming_reference = segments.is_empty()
            && place
                .root
                .as_symbol()
                .is_some_and(|symbol| program.state_parameter_by_symbol(state, symbol).is_some());
        if validation::reference_shell(program, selected_type).is_some() && !incoming_reference {
            return Some(false);
        }
        if region {
            let element = validation::collection_element_type_reference(program, selected_type)?;
            let carriers = crate::borrow::view_link::borrow_carrying_owner_paths(program, element)?;
            return Some(carriers.is_empty());
        }
        Some(true)
    }

    /*
        A returned descriptor belongs to the completed invocation, not a new local
        owning its pointee. Instantiate its normal-return origins against the exact
        captured actuals. Declaration lifetime demands remain additional retention
        obligations; they do not replace the normal-return geometry.
    */
    fn returned_places(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        call_handle: Handle<FlowCallFact>,
        result_suffix: &[facts::PlaceSegment],
        completion: HandleSpan<checked_trees::FlowConstraintRef>,
        active_calls: &mut Vec<Handle<FlowCallFact>>,
    ) -> Option<Vec<ResolvedAlias>> {
        use checked_trees::{
            CheckedReferenceOriginProjection as Projection,
            CheckedUnitStructuralPathSegment as Segment,
        };
        let call = facts.flow.control.calls.get(call_handle);
        let (operands, receiver_offset) =
            captured_call_operands(program, facts.flow.view(), state, call)?;
        let parameters = validation::call_target_parameters(program, call.target_symbol)?;
        let target = validation::call_target_state(program, call.target_symbol)?;
        let sources = validation::reference_result_custody::call_return_sources(
            program,
            facts,
            call.target_symbol,
        )?;
        let mut output = Vec::new();
        let mut matched = false;
        for source in sources {
            let result_path = validation::reference_result_custody::loan_place_segments(
                program,
                target.return_type,
                &source.path,
            )?;
            let Some(suffix) = result_suffix.strip_prefix(result_path.as_slice()) else {
                continue;
            };
            if !suffix.is_empty() {
                let returned_type = validation::project_type_reference_from_segments(
                    program,
                    target.return_type,
                    &result_path,
                )?;
                let returned = validation::reference_shell(program, returned_type)?;
                if source.source.extent != semantic_vocabulary::ReferenceOriginExtent::Projected
                    || source.source.type_identity
                        != program.normalized_type_identity(returned.referee)
                {
                    return None;
                }
            }
            matched = true;
            let checked_trees::CheckedReferenceOriginRoot::Parameter { index } = source.source.root
            else {
                return None;
            };
            let (position, parameter) =
                validation::reference_result_custody::origin_formal(program, parameters, index)?;
            let operand = operands.get(receiver_offset + position)?;
            let backings = projected_operand_places(
                program,
                facts,
                loan_occurrences,
                state,
                operand,
                parameter.type_reference,
                &source.source.path,
                call.entry_constraints,
                active_calls,
            )?;
            for mut backing in backings {
                backing.place.segments.extend_from_slice(suffix);
                for mut selected in retained_places(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    call.statement_index,
                    completion,
                    backing.place,
                    active_calls,
                )? {
                    for parent in &backing.lineage {
                        if !selected.lineage.contains(parent) {
                            selected.lineage.push(*parent);
                        }
                    }
                    output.push(selected);
                }
            }
        }
        if !matched {
            return None;
        }
        let typed_trees::expression::ExpressionNode::Call(expression) =
            expressions(program).expression(call.authored_expression)
        else {
            return None;
        };
        for (source, _) in
            crate::borrow::call_reference_sources(program, state.state_symbol, expression)?
        {
            let result_path =
                crate::borrow::carrier_path(program, target.return_type, &source.owner_path)?;
            let result_path = validation::reference_result_custody::loan_place_segments(
                program,
                target.return_type,
                &result_path,
            )?;
            if !result_suffix.starts_with(&result_path) {
                continue;
            }
            let operand = operands.get(receiver_offset + source.parameter_index)?;
            let parameter = parameters.get(source.parameter_index)?;
            let mut path = Vec::new();
            if source.source == validation::borrow_contracts::ViewReturnInputSource::Stored {
                let carrier = validation::reference_result_custody::input_carrier_type(
                    program,
                    parameter.type_reference,
                )?;
                if validation::reference_shell(program, parameter.type_reference).is_some() {
                    path.push(Projection::Exact(Segment::Referent));
                }
                path.extend(
                    crate::borrow::carrier_path(program, carrier, &source.source_path)?
                        .into_iter()
                        .map(Projection::Exact),
                );
                path.push(Projection::Exact(Segment::Referent));
            }
            output.extend(projected_operand_places(
                program,
                facts,
                loan_occurrences,
                state,
                operand,
                parameter.type_reference,
                &path,
                call.entry_constraints,
                active_calls,
            )?);
        }
        Some(output)
    }

    /*
        Each descriptor crossing chooses its own epoch. The actual's descriptor
        is frozen at operand completion; storage reached through that address is
        observed at this nested invocation, before its body runs. Callee writes
        have already selected replacement inputs in the normal-return relation.
    */
    fn projected_operand_places(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &FlowStateFact,
        operand: &FlowOperandFact,
        reference: typed_trees::types::TypeReferenceHandle,
        path: &[checked_trees::CheckedReferenceOriginProjection],
        invocation: HandleSpan<checked_trees::FlowConstraintRef>,
        active_calls: &mut Vec<Handle<FlowCallFact>>,
    ) -> Option<Vec<ResolvedAlias>> {
        use checked_trees::{
            CheckedReferenceOriginProjection as Projection,
            CheckedUnitStructuralPathSegment as Segment,
        };
        if let checked_trees::FlowOperandCompletion::Selected(alternatives) = operand.completion {
            let alternatives = facts.flow.control.operand_alternatives.span(alternatives)?;
            if alternatives.is_empty() {
                return None;
            }
            let mut complete = Vec::new();
            for alternative in alternatives {
                if !alternative.source_arm.is_valid()
                    || !facts.flow.control.operands.is_valid(alternative.value)
                {
                    return None;
                }
                let places = projected_operand_places(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    facts.flow.control.operands.get(alternative.value),
                    reference,
                    path,
                    invocation,
                    active_calls,
                )?;
                if places.is_empty() {
                    return None;
                }
                complete.extend(places);
            }
            return Some(complete);
        }
        if !operand.constructed_children().is_empty() {
            let count = path
                .iter()
                .take_while(|projection| {
                    !matches!(projection, Projection::Exact(Segment::Referent))
                })
                .count();
            let selected = validation::reference_result_custody::origin_place_segments(
                program,
                reference,
                path[..count].iter().cloned(),
            )?;
            let mut children = facts
                .flow
                .control
                .operand_children
                .span(operand.constructed_children())?
                .iter()
                .filter_map(|child| {
                    let prefix = facts.flow.control.operand_child_paths.span(child.path)?;
                    selected
                        .starts_with(prefix)
                        .then_some((child, prefix.len()))
                });
            let (child, consumed) = children.next()?;
            if children.next().is_some() || consumed == 0 {
                return None;
            }
            return projected_operand_places(
                program,
                facts,
                loan_occurrences,
                state,
                facts.flow.control.operands.get(child.value),
                child.type_reference,
                &path[consumed..],
                invocation,
                active_calls,
            );
        }
        let source = place_at(
            program,
            state.state_symbol,
            operand.statement_index,
            operand.expression,
        )?;
        let borrowed = validation::reference_shell(program, reference);
        let mut selected = borrowed.as_ref().map_or(reference, |shell| shell.referee);
        let mut remaining = path;
        if borrowed.is_some()
            && matches!(
                remaining.first(),
                Some(Projection::Exact(Segment::Referent))
            )
        {
            remaining = &remaining[1..];
        }
        let mut backings = if borrowed.is_some() {
            retained_places(
                program,
                facts,
                loan_occurrences,
                state,
                operand.statement_index,
                operand.constraints,
                CapturedPlace {
                    root: source.root,
                    segments: source.segments,
                    ..Default::default()
                },
                active_calls,
            )?
        } else {
            vec![ResolvedAlias {
                place: CapturedPlace {
                    root: source.root,
                    segments: source.segments,
                    ..Default::default()
                },
                lineage: Vec::new(),
            }]
        };
        let mut constraints = if borrowed.is_some() {
            invocation
        } else {
            operand.constraints
        };
        loop {
            let count = remaining
                .iter()
                .take_while(|projection| {
                    !matches!(projection, Projection::Exact(Segment::Referent))
                })
                .count();
            let segments = validation::reference_result_custody::origin_place_segments(
                program,
                selected,
                remaining[..count].iter().cloned(),
            )?;
            selected =
                validation::project_type_reference_from_segments(program, selected, &segments)?;
            for backing in &mut backings {
                backing.place.segments.extend_from_slice(&segments);
            }
            remaining = &remaining[count..];
            if remaining.is_empty() {
                return Some(backings);
            }
            selected = validation::reference_shell(program, selected)?.referee;
            let mut crossed = Vec::new();
            for backing in backings {
                for mut source in retained_places(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    operand.statement_index,
                    constraints,
                    backing.place,
                    active_calls,
                )? {
                    for parent in &backing.lineage {
                        if !source.lineage.contains(parent) {
                            source.lineage.push(*parent);
                        }
                    }
                    crossed.push(source);
                }
            }
            backings = crossed;
            constraints = invocation;
            remaining = &remaining[1..];
        }
    }
}
mod stored_call_inputs {
    /*
        A reference saved by a helper keeps the evaluated input that lent it.
        Bind the completed stored generation to those original call resources.
        Their full access and parent lifetime stay intact even when the saved
        pointer selects a smaller part of the input's data.
    */
    use crate::checks::borrows::loan_occurrences::LoanOccurrences;
    use arena::Handle;
    use checked_trees::{BorrowLoanLineage, CheckFacts, CheckedCallBorrowLoanSource};

    pub(super) fn install(program: &dyn validation::CheckingInput, facts: &mut CheckFacts) {
        facts.borrow.stored_call_inputs.reset_retain_capacity();
        facts
            .borrow
            .stored_call_input_resources
            .reset_retain_capacity();
        let prepared_loans = LoanOccurrences::new(program, facts);
        let loan_occurrences = &prepared_loans;
        let mut retained = Vec::new();
        for (_, state) in facts.flow.control.states.iter() {
            let Some(owner) = facts
                .borrow
                .unique_state_borrow(state.machine_symbol, state.state_symbol)
            else {
                continue;
            };
            for (loan, row) in facts.borrow.state_loans(owner) {
                if row.lineage != BorrowLoanLineage::UnretainedDerived
                    || !row.established_by_call.is_valid()
                {
                    continue;
                }
                match retained_inputs(program, facts, loan_occurrences, state, loan, row) {
                    Ok(inputs) => retained.push((loan, inputs)),
                    Err(reason) if ::validation::trace_flags::check_sites() => {
                        eprintln!(
                            "CHECKSITE stored-call-input-unavailable loan={loan:?} reason={reason}"
                        );
                    }
                    Err(_) => {}
                }
            }
        }
        drop(prepared_loans);
        for (loan, inputs) in retained {
            let resources = facts.borrow.stored_call_input_resources.insert_many(inputs);
            facts
                .borrow
                .stored_call_inputs
                .insert(checked_trees::CheckedStoredCallBorrowInputs { loan, resources });
        }
    }

    fn retained_inputs(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        state: &checked_trees::FlowStateFact,
        loan: Handle<checked_trees::BorrowLoanFact>,
        row: &checked_trees::BorrowLoanFact,
    ) -> Result<Vec<Handle<checked_trees::CheckedCallBorrowLoanResource>>, &'static str> {
        let descriptors = validation::completed_storage::loan_call_descriptors(
            program,
            facts,
            state.machine_symbol,
            state.state_symbol,
            loan,
        )?
        .ok_or("the saved reference has no completed call generation")?;
        let mut inputs = Vec::new();
        for descriptor in descriptors {
            let validation::completed_storage::CompletedCallDescriptorSource::Captured(source) =
                descriptor.source
            else {
                return Err("the saved reference still requires its original incoming permission");
            };
            let (suppliers, source_parents) = super::permission_sources::lending_sources(
                program,
                facts,
                loan_occurrences,
                state,
                descriptor.call,
                source.capture,
                &source.projection,
                &row.kind,
            )?;
            for supplier in suppliers {
                let original = match supplier.source {
                    CheckedCallBorrowLoanSource::CallAccess { .. } => {
                        if !facts.flow.control.operands.is_valid(supplier.actual) {
                            return Err("the saved reference lost its original evaluated input");
                        }
                        crate::checks::borrows::formation_captures::actual_invocation(
                            program,
                            facts,
                            state,
                            supplier.actual,
                        )?
                        .ok_or("the saved reference has no original invocation tuple")?
                    }
                    CheckedCallBorrowLoanSource::StoredLoan { .. } => descriptor.call,
                };
                let mut found = false;
                for (resource_handle, resource) in facts.borrow.call_loan_resources.iter() {
                    if resource.call != original
                        || resource.parent_lifetime.root != supplier.backing.root
                        || resource.access.direct_reborrow_effect(&row.kind).is_none()
                    {
                        continue;
                    }
                    let same_permission = match (&supplier.source, &resource.source) {
                        (
                            CheckedCallBorrowLoanSource::StoredLoan { loan },
                            CheckedCallBorrowLoanSource::StoredLoan { loan: original },
                        ) => loan == original,
                        (
                            CheckedCallBorrowLoanSource::CallAccess { access, parents },
                            CheckedCallBorrowLoanSource::CallAccess {
                                access: original,
                                parents: original_parents,
                            },
                        ) => {
                            resource.operand == supplier.actual
                                && access == original
                                && facts.borrow.call_loan_parents.span(*original_parents)
                                    == source_parents.span(*parents)
                        }
                        _ => false,
                    };
                    if !same_permission {
                        continue;
                    }
                    found = true;
                    if !inputs.contains(&resource_handle) {
                        inputs.push(resource_handle);
                    }
                }
                if !found {
                    return Err("the saved reference's original call resource is unavailable");
                }
            }
        }
        if inputs.is_empty() {
            return Err("the saved reference has no complete input resource roster");
        }
        Ok(inputs)
    }
}
pub(in crate::checks::borrows) mod supplied_permissions {
    /*
        A normal provider completion supplies an opaque descriptor permission.
        It has no caller-owned root activation or reborrow restoration interval.
        Validate its published completion against the exact initialized slot and
        supplying invocation. Syntactic call rows alone need not have executed;
        use sites still require their own active cut and retained supplier lineage.
    */
    use checked_trees::{BorrowFacts, BorrowLoanLineage, FlowFacts};
    use diagnostics::Diagnostic;

    mod completions {
        /*
            A helper's completed store supplies a reference without lending the output
            variable itself. Keep that permission and every promised input together;
            copying the reference must not forget either when the variable changes.
        */
        use crate::checks::borrows::loan_occurrences::LoanOccurrences;
        use arena::{Handle, HandleSpan};
        use checked_trees::{
            BorrowLoanLineage, CheckFacts, CheckedCallBorrowLoanSource,
            CheckedOutputLifetimeDependencyResource, CheckedSuppliedOutputBorrowResource,
            IncomingReferenceOrigin,
        };
        use diagnostics::Diagnostic;

        pub(in crate::checks::borrows::resources) fn install(
            program: &dyn validation::CheckingInput,
            facts: &mut CheckFacts,
        ) -> Result<(), Vec<Diagnostic>> {
            let prepared_loans = LoanOccurrences::new(program, facts);
            let loan_occurrences = &prepared_loans;
            let mut plans = Vec::new();
            for (loan, row) in facts.borrow.loans.iter() {
                if row.lineage != BorrowLoanLineage::SuppliedOutput {
                    continue;
                }
                let Some(call) = super::normal_call(&facts.borrow, &facts.flow, loan) else {
                    continue;
                };
                let incoming = facts.borrow.incoming_references.get(row.incoming_reference);
                let diagnostic = |reason: &'static str| {
                    let invocation = facts.flow.control.calls.get(call);
                    let message = format!(
                        "{reason}; completed output from `{}` in machine `{}`, state `{}` ({:?}), statement {}, call {}, loan {loan:?}",
                        crate::published::labels::call_target_label(
                            program,
                            invocation.target_symbol
                        ),
                        crate::published::labels::symbol_name(program, incoming.machine_symbol),
                        crate::published::labels::symbol_name(program, incoming.state_symbol),
                        incoming.state_symbol,
                        row.statement_index,
                        invocation.call_ordinal,
                    );
                    let mut diagnostic = Diagnostic::error(message);
                    let span = invocation
                        .authored_expression
                        .is_valid()
                        .then(|| {
                            program
                                .expression_table
                                .source_span(invocation.authored_expression)
                        })
                        .filter(|span| span.span.end > span.span.start)
                        .or_else(|| {
                            let machine = program.machine_by_symbol(incoming.machine_symbol)?;
                            let state = program.state_in_machine(machine, incoming.state_symbol)?;
                            crate::flow::state_statements(program, state.statement_nodes)
                                .get(invocation.statement_index)
                                .and_then(|statement| {
                                    validation::statement_source_span(program, statement)
                                })
                        });
                    if let Some(span) = span {
                        diagnostic = diagnostic.with_source_span(span);
                    }
                    vec![diagnostic]
                };
                let IncomingReferenceOrigin::Output { identity } = incoming.origin else {
                    return refuse("completed output lost its output identity");
                };
                let owner = facts
                    .borrow
                    .owner_segments
                    .span(incoming.owner_path)
                    .ok_or_else(|| {
                        diagnostic("completed output lost its declared descriptor path")
                    })?;
                let path = crate::borrow::owner_place_segments(owner);
                let mut receipts =
                    facts
                        .flow
                        .control
                        .output_lifetime_sources
                        .iter()
                        .filter(|(_, receipt)| {
                            receipt.output == identity
                                && facts
                                    .flow
                                    .control
                                    .operand_retained_paths
                                    .span(receipt.owner_path)
                                    .is_some_and(|candidate| same_path(program, candidate, &path))
                        });
                let (_, receipt) = receipts.next().ok_or_else(|| {
                    diagnostic("completed output has no captured lifetime-input receipt")
                })?;
                if receipts.next().is_some() {
                    return refuse("completed output has ambiguous lifetime-input receipts");
                }
                let sources = receipt.sources.ok_or_else(|| {
                    diagnostic("completed output lifetime inputs are unavailable")
                })?;
                let ceilings = crate::borrow::lifetime_contract::output_input_accesses(
                    program,
                    &facts.borrow,
                    &facts.flow.control,
                    identity,
                    owner,
                    sources,
                )
                .map_err(diagnostic)?;
                let state = facts
                    .flow
                    .control
                    .unique_state(incoming.machine_symbol, incoming.state_symbol)
                    .ok_or_else(|| diagnostic("completed output has no unique flow owner"))?;
                let inputs = super::inputs::plan(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    call,
                    sources,
                    &ceilings,
                )
                .map_err(diagnostic)?;
                let origin = facts
                    .borrow
                    .supplied_output_call(loan)
                    .ok_or_else(|| diagnostic("completed output lost its establishing call"))?;
                let debts = facts
                    .borrow
                    .calls
                    .get(origin)
                    .completed_reference_loans
                    .handles()
                    .filter(|handle| {
                        let debt = facts.borrow.loans.get(*handle);
                        debt.lineage == BorrowLoanLineage::LifetimeDependency
                            && debt.storage_owner == row.storage_owner
                            && facts.borrow.loan_owner_path(debt)
                                == facts.borrow.loan_owner_path(row)
                    })
                    .collect::<Vec<_>>();
                plans.push((
                    CheckedSuppliedOutputBorrowResource {
                        loan,
                        machine_symbol: incoming.machine_symbol,
                        state_symbol: incoming.state_symbol,
                        call,
                        identity: row.incoming_reference,
                        lifetime_sources: sources,
                        inputs: HandleSpan::empty(),
                        debts: HandleSpan::empty(),
                        holders: HandleSpan::empty(),
                        holder_weakening: Handle::invalid(),
                        final_weakening: Handle::invalid(),
                    },
                    inputs,
                    debts,
                ));
            }
            drop(prepared_loans);
            facts
                .borrow
                .supplied_output_resources
                .reset_retain_capacity();
            facts.borrow.supplied_output_inputs.reset_retain_capacity();
            facts
                .borrow
                .output_lifetime_dependencies
                .reset_retain_capacity();
            facts
                .borrow
                .output_dependency_suppliers
                .reset_retain_capacity();
            for (mut output, inputs, debts) in plans {
                let mut handles = Vec::with_capacity(inputs.resources.len());
                for input in inputs.resources {
                    let (permission, capture_path) = match input.permission {
                        super::inputs::InputPermissionDraft::Borrowed(mut draft) => {
                            if let CheckedCallBorrowLoanSource::CallAccess { parents, .. } =
                                &mut draft.resource.source
                            {
                                let recorded = inputs.parents.span(*parents).ok_or_else(|| {
                                    diagnostic("completed input lost its exact borrowing parents")
                                })?;
                                *parents = facts
                                    .borrow
                                    .call_loan_parents
                                    .insert_many(recorded.iter().copied());
                            }
                            let resource = super::super::call_operands::install_resource(
                                program, facts, draft,
                            )
                            .ok_or_else(|| {
                                diagnostic("completed input permission has no typed capture path")
                            })?;
                            (
                                checked_trees::CheckedOutputBorrowInputPermission::Borrowed {
                                    resource,
                                },
                                facts.borrow.call_loan_resources.get(resource).capture_path,
                            )
                        }
                        super::inputs::InputPermissionDraft::Incoming {
                            place,
                            access,
                            owner_path,
                        } => {
                            let contract = facts
                                .flow
                                .control
                                .operand_retained_suppliers
                                .get(input.source)
                                .contract
                                .ok_or_else(|| {
                                    diagnostic("completed input has no original declared edge")
                                })?;
                            let actual = facts.flow.control.operands.get(contract.input);
                            let path = crate::borrow::carrier_path(
                                program,
                                actual.type_reference,
                                &owner_path,
                            )
                            .ok_or_else(|| {
                                diagnostic("incoming input has no typed captured leaf path")
                            })?;
                            let capture_path = facts.borrow.call_loan_paths.insert_many(path);
                            (
                                checked_trees::CheckedOutputBorrowInputPermission::Incoming {
                                    identity: place.incoming_reference,
                                    access,
                                    root: place.root,
                                    path: facts
                                        .borrow
                                        .call_loan_backing_paths
                                        .insert_many(place.segments),
                                    owner_path: facts
                                        .borrow
                                        .call_loan_backing_paths
                                        .insert_many(place.incoming_owner_path),
                                },
                                capture_path,
                            )
                        }
                    };
                    handles.push(checked_trees::CheckedOutputBorrowInput {
                        capture_path,
                        source: input.source,
                        stored: input.stored,
                        permission,
                    });
                }
                output.inputs = facts.borrow.supplied_output_inputs.insert_many(handles);
                output.debts = facts
                    .borrow
                    .loan_suppliers
                    .insert_many(debts.iter().copied());
                let inputs = output.inputs;
                let output = facts.borrow.supplied_output_resources.insert(output);
                for debt in debts {
                    facts.borrow.output_lifetime_dependencies.insert(
                        CheckedOutputLifetimeDependencyResource {
                            loan: debt,
                            source:
                                checked_trees::CheckedOutputLifetimeDependencySource::Completion {
                                    output,
                                },
                            inputs,
                        },
                    );
                }
            }
            super::debts::install_transfers(facts)?;
            Ok(())
        }

        fn same_path(
            program: &dyn validation::CheckingInput,
            left: &[facts::PlaceSegment],
            right: &[facts::PlaceSegment],
        ) -> bool {
            left.len() == right.len()
                && left.iter().zip(right).all(|(left, right)| {
                    validation::storage_path_relations::canonical_place_segments_equal(
                        program, *left, *right,
                    )
                })
        }

        fn diagnostic(reason: &'static str) -> Vec<Diagnostic> {
            vec![Diagnostic::error(reason)]
        }

        fn refuse<T>(reason: &'static str) -> Result<T, Vec<Diagnostic>> {
            Err(diagnostic(reason))
        }
    }
    mod debts {
        /*
            A saved output keeps the helper's original input restrictions. Those
            restrictions can cover several inputs and earlier outputs; they are not
            the address of the saved pointer, or a borrow of its former variable.
        */
        use arena::Handle;
        use checked_trees::{
            BorrowFacts, BorrowLoanFact, BorrowLoanLineage, CapturedPlace, CheckFacts,
            CheckedCallBorrowLoanSource, CheckedOutputBorrowInput,
            CheckedOutputLifetimeDependencyResource as Debt,
            CheckedOutputLifetimeDependencySource as Source,
        };
        use diagnostics::Diagnostic;

        pub(in crate::checks::borrows::resources) fn install_transfers(
            facts: &mut CheckFacts,
        ) -> Result<(), Vec<Diagnostic>> {
            let borrow = &facts.borrow;
            let mut pending = borrow
                .states
                .iter()
                .flat_map(|(_, state)| {
                    borrow.state_loans(state).filter_map(move |(loan, row)| {
                        (row.lineage == BorrowLoanLineage::LifetimeDependency
                            && resource(borrow, loan).is_none())
                        .then_some((state.machine_symbol, state.state_symbol, loan))
                    })
                })
                .collect::<Vec<_>>();
            loop {
                let before = pending.len();
                let mut remaining = Vec::new();
                for (machine, state, loan) in pending {
                    let row = facts.borrow.loans.get(loan);
                    let Some(suppliers) = facts.borrow.loan_suppliers.span(row.supplying_loans)
                    else {
                        remaining.push((machine, state, loan));
                        continue;
                    };
                    let mut parents = Vec::new();
                    let mut inputs: Vec<CheckedOutputBorrowInput> = Vec::new();
                    let mut complete = !suppliers.is_empty() && row.source_owner_symbol.is_valid();
                    let Some((_, owner)) = facts.borrow.first_state_borrow(machine, state) else {
                        return Err(vec![Diagnostic::error(
                            "transported output debt has no owning state",
                        )]);
                    };
                    for source in suppliers {
                        if !facts.borrow.state_owns_loan(owner, *source) {
                            complete = false;
                            break;
                        }
                        let Some(parent) = resource(&facts.borrow, *source) else {
                            complete = false;
                            break;
                        };
                        let source_row = facts.borrow.loans.get(*source);
                        if source_row.owner_symbol != row.source_owner_symbol
                            || source_row.kind.direct_reborrow_effect(&row.kind).is_none()
                        {
                            complete = false;
                            break;
                        }
                        let parent_row = facts.borrow.output_lifetime_dependencies.get(parent);
                        let Some(captured) =
                            facts.borrow.supplied_output_inputs.span(parent_row.inputs)
                        else {
                            return Err(vec![Diagnostic::error(
                                "transported output debt lost its complete captured inputs",
                            )]);
                        };
                        for input in captured {
                            crate::shared::collections::push_unique(&mut inputs, *input);
                        }
                        crate::shared::collections::push_unique(&mut parents, parent);
                    }
                    if !complete {
                        remaining.push((machine, state, loan));
                        continue;
                    }
                    let suppliers = facts
                        .borrow
                        .output_dependency_suppliers
                        .insert_many(parents);
                    let inputs = facts.borrow.supplied_output_inputs.insert_many(inputs);
                    facts.borrow.output_lifetime_dependencies.insert(Debt {
                        loan,
                        source: Source::Transfer { suppliers },
                        inputs,
                    });
                }
                if remaining.len() == before {
                    break;
                }
                pending = remaining;
            }
            Ok(())
        }

        fn resource(borrow: &BorrowFacts, loan: Handle<BorrowLoanFact>) -> Option<Handle<Debt>> {
            if !borrow.loans.is_valid(loan)
                || borrow.loans.get(loan).lineage != BorrowLoanLineage::LifetimeDependency
            {
                return None;
            }
            let mut rows = borrow
                .output_lifetime_dependencies
                .iter()
                .filter(|(_, row)| row.loan == loan);
            let (handle, _) = rows.next()?;
            rows.next().is_none().then_some(handle)
        }

        pub(in crate::checks::borrows) fn has_resource(
            borrow: &BorrowFacts,
            loan: Handle<BorrowLoanFact>,
        ) -> bool {
            matches!(windows(borrow, loan), Some(Ok(_)))
        }

        #[derive(Clone)]
        pub(in crate::checks::borrows) struct HeldInput {
            pub(in crate::checks::borrows) place: CapturedPlace,
            pub(in crate::checks::borrows) access: checked_trees::BorrowAccessKind,
        }

        /*
            All input windows remain separate. In particular, a shared result cannot
            turn an original whole-input Mutable restriction into Read, and another
            output's lifetime frontier must be followed without making it a pointee.
        */
        pub(in crate::checks::borrows) fn windows(
            borrow: &BorrowFacts,
            loan: Handle<BorrowLoanFact>,
        ) -> Option<Result<Vec<HeldInput>, &'static str>> {
            let resource = match resource(borrow, loan) {
                Some(resource) => resource,
                None if completed_output_value(borrow, loan) => {
                    return Some(Err(
                        "completed output debt has no complete captured-generation resource",
                    ));
                }
                None => return None,
            };
            Some(resolve_windows(borrow, resource))
        }

        fn completed_output_value(borrow: &BorrowFacts, loan: Handle<BorrowLoanFact>) -> bool {
            if !borrow.loans.is_valid(loan) {
                return false;
            }
            let debt = borrow.loans.get(loan);
            debt.lineage == BorrowLoanLineage::LifetimeDependency
                && borrow.loans.iter().any(|(_, peer)| {
                    peer.lineage != BorrowLoanLineage::LifetimeDependency
                        && peer.storage_owner == debt.storage_owner
                        && peer.statement_index == debt.statement_index
                        && peer.formation_expression == debt.formation_expression
                        && borrow.loan_owner_path(peer) == borrow.loan_owner_path(debt)
                        && borrow.incoming_references.is_valid(peer.incoming_reference)
                        && matches!(
                            borrow
                                .incoming_references
                                .get(peer.incoming_reference)
                                .origin,
                            checked_trees::IncomingReferenceOrigin::Output { .. }
                        )
                })
        }

        /*
            Returned leaves retain their completed descriptor namespace even though
            they have no local loan handle. That exact identity selects the complete
            input cohort; equal backing or a field name supplies no such association.
        */
        pub(in crate::checks::borrows) fn output_windows(
            borrow: &BorrowFacts,
            identity: Handle<checked_trees::IncomingReferenceIdentity>,
        ) -> Option<Result<Vec<HeldInput>, &'static str>> {
            let mut outputs = borrow
                .supplied_output_resources
                .iter()
                .filter(|(_, output)| output.identity == identity);
            let Some((handle, output)) = outputs.next() else {
                return (borrow.incoming_references.is_valid(identity)
                    && matches!(
                        borrow.incoming_references.get(identity).origin,
                        checked_trees::IncomingReferenceOrigin::Output { .. }
                    ))
                .then_some(Err(
                    "returned output has no authenticated completed generation",
                ));
            };
            if outputs.next().is_some() {
                return Some(Err("returned debt has ambiguous completed generations"));
            }
            let result = (|| {
                if output.lifetime_sources.is_empty() {
                    return Ok(Vec::new());
                }
                let debts = borrow
                    .loan_suppliers
                    .span(output.debts)
                    .filter(|debts| !debts.is_empty())
                    .ok_or("returned output has no original lifetime cohort")?;
                let mut windows = Vec::new();
                for loan in debts {
                    let resource = resource(borrow, *loan)
                        .ok_or("returned output lost one original input debt")?;
                    if borrow.output_lifetime_dependencies.get(resource).source
                        != (Source::Completion { output: handle })
                    {
                        return Err("returned output debt belongs to another completed generation");
                    }
                    windows.extend(resolve_windows(borrow, resource)?);
                }
                Ok(windows)
            })();
            Some(result)
        }

        fn resolve_windows(
            borrow: &BorrowFacts,
            debt_handle: Handle<Debt>,
        ) -> Result<Vec<HeldInput>, &'static str> {
            validate_debt_sources(borrow, debt_handle)?;
            let row = borrow.output_lifetime_dependencies.get(debt_handle);
            let mut inputs = borrow
                .supplied_output_inputs
                .span(row.inputs)
                .ok_or("output debt's captured input roster is unavailable")?
                .iter()
                .map(|input| input.permission)
                .collect::<Vec<_>>();
            let mut input_seen = Vec::new();
            let mut loans = Vec::new();
            let mut loan_seen = Vec::new();
            let mut result = Vec::new();
            while !inputs.is_empty() || !loans.is_empty() {
                if let Some(input) = inputs.pop() {
                    if input_seen.contains(&input) {
                        continue;
                    }
                    input_seen.push(input);
                    let input = match input {
                        checked_trees::CheckedOutputBorrowInputPermission::Borrowed {
                            resource,
                        } => resource,
                        checked_trees::CheckedOutputBorrowInputPermission::Incoming {
                            identity,
                            access,
                            root,
                            path,
                            owner_path,
                        } => {
                            if !borrow.incoming_references.is_valid(identity) {
                                return Err("output debt lost its exact incoming permission");
                            }
                            result.push(HeldInput {
                                place: CapturedPlace {
                                    incoming_reference: identity,
                                    incoming_owner_path: borrow
                                        .call_loan_backing_paths
                                        .span(owner_path)
                                        .ok_or("output debt lost its incoming owner path")?
                                        .to_vec(),
                                    root,
                                    segments: borrow
                                        .call_loan_backing_paths
                                        .span(path)
                                        .ok_or("output debt lost its incoming input path")?
                                        .to_vec(),
                                },
                                access: access.into(),
                            });
                            continue;
                        }
                    };
                    if !borrow.call_loan_resources.is_valid(input) {
                        return Err("output debt names an unavailable input permission");
                    }
                    let input = borrow.call_loan_resources.get(input);
                    result.push(HeldInput {
                        place: CapturedPlace {
                            incoming_reference: input.backing_incoming_reference,
                            incoming_owner_path: borrow
                                .call_loan_backing_paths
                                .span(input.backing_incoming_owner_path)
                                .ok_or("output debt lost its captured incoming path")?
                                .to_vec(),
                            root: input.parent_lifetime.root,
                            segments: borrow
                                .call_loan_backing_paths
                                .span(input.backing_path)
                                .ok_or("output debt lost its captured input window")?
                                .to_vec(),
                        },
                        access: input.access.clone(),
                    });
                    match input.source {
                        CheckedCallBorrowLoanSource::StoredLoan { loan } => loans.push(loan),
                        CheckedCallBorrowLoanSource::CallAccess { parents, .. } => loans
                            .extend_from_slice(
                                borrow
                                    .call_loan_parents
                                    .span(parents)
                                    .ok_or("output debt lost its captured input ancestors")?,
                            ),
                    }
                    continue;
                }
                let loan = loans.pop().ok_or("output debt traversal lost a source")?;
                if loan_seen.contains(&loan) {
                    continue;
                }
                if !borrow.loans.is_valid(loan) {
                    return Err("output debt has an unavailable input ancestor");
                }
                loan_seen.push(loan);
                let row = borrow.loans.get(loan);
                for (_, output) in borrow
                    .supplied_output_resources
                    .iter()
                    .filter(|(_, output)| output.loan == loan)
                {
                    if !complete_inputs(borrow, output) {
                        return Err("earlier output has an unrepresented input lifetime region");
                    }
                    inputs.extend(
                        borrow
                            .supplied_output_inputs
                            .span(output.inputs)
                            .ok_or("earlier output's captured inputs are unavailable")?
                            .iter()
                            .map(|input| input.permission),
                    );
                }
                if let Some(debt) = resource(borrow, loan) {
                    validate_debt_sources(borrow, debt)?;
                    inputs.extend(
                        borrow
                            .supplied_output_inputs
                            .span(borrow.output_lifetime_dependencies.get(debt).inputs)
                            .ok_or("earlier debt's captured inputs are unavailable")?
                            .iter()
                            .map(|input| input.permission),
                    );
                } else if row.lineage == BorrowLoanLineage::LifetimeDependency {
                    return Err("earlier input debt has no complete captured lifetime resource");
                }
                if let BorrowLoanLineage::Reborrow { parent_loan } = row.lineage {
                    loans.push(parent_loan);
                }
                loans.extend_from_slice(
                    borrow
                        .loan_suppliers
                        .span(row.supplying_loans)
                        .ok_or("output debt's input ancestry is unavailable")?,
                );
            }
            Ok(result)
        }

        fn validate_debt_sources(
            borrow: &BorrowFacts,
            root: Handle<Debt>,
        ) -> Result<(), &'static str> {
            let mut pending = vec![(root, false)];
            let mut visited = Vec::new();
            let mut active = Vec::new();
            while let Some((debt, finished)) = pending.pop() {
                if finished {
                    active.retain(|current| *current != debt);
                    visited.push(debt);
                    continue;
                }
                if visited.contains(&debt) {
                    continue;
                }
                if active.contains(&debt) {
                    return Err("input debt has cyclic original generations");
                }
                if !borrow.output_lifetime_dependencies.is_valid(debt) {
                    return Err("input debt has an unavailable original generation");
                }
                active.push(debt);
                pending.push((debt, true));
                match borrow.output_lifetime_dependencies.get(debt).source {
                    Source::Completion { output } => {
                        if !borrow.supplied_output_resources.is_valid(output)
                            || !complete_inputs(
                                borrow,
                                borrow.supplied_output_resources.get(output),
                            )
                        {
                            return Err("input debt has an incomplete original input frontier");
                        }
                    }
                    Source::Transfer { suppliers } => {
                        pending.extend(
                            borrow
                                .output_dependency_suppliers
                                .span(suppliers)
                                .filter(|sources| !sources.is_empty())
                                .ok_or("input debt has no original suppliers")?
                                .iter()
                                .map(|source| (*source, false)),
                        );
                    }
                }
            }
            Ok(())
        }

        fn complete_inputs(
            borrow: &BorrowFacts,
            output: &checked_trees::CheckedSuppliedOutputBorrowResource,
        ) -> bool {
            let Some(inputs) = borrow.supplied_output_inputs.span(output.inputs) else {
                return false;
            };
            output
                .lifetime_sources
                .handles()
                .all(|source| inputs.iter().any(|input| input.source == source))
        }
    }
    mod inputs {
        /*
            A saved output keeps the references selected before its helper writes.
            The short borrow used to reach a slot is separate from a stored reference's
            own backing. Preserve every frozen member and its declared access ceiling.
        */
        use super::super::call_operands::ResourceDraft;
        use super::super::permission_sources;
        use crate::checks::borrows::loan_occurrences::LoanOccurrences;
        use arena::{Arena, Handle, HandleSpan};
        use checked_trees::{
            BorrowLoanFact, CapturedPlace, CheckFacts, CheckedCallBorrowLoanResource,
            CheckedCallBorrowLoanSource, CheckedDirectBorrowParentLifetime, FlowCallFact,
            FlowOperandRetainedSupplier, FlowOperandStoredSource, FlowStateFact,
        };
        use language_semantics::ReferenceAccess;

        pub(super) enum InputPermissionDraft {
            Borrowed(ResourceDraft),
            Incoming {
                place: CapturedPlace,
                access: ReferenceAccess,
                owner_path: Vec<checked_trees::BorrowLoanOwnerSegment>,
            },
        }
        pub(super) struct InputResourceDraft {
            pub(super) source: Handle<FlowOperandRetainedSupplier>,
            pub(super) stored: Handle<FlowOperandStoredSource>,
            pub(super) permission: InputPermissionDraft,
        }
        pub(super) struct InputPlan {
            pub(super) resources: Vec<InputResourceDraft>,
            pub(super) parents: Arena<Handle<BorrowLoanFact>>,
        }

        pub(super) fn plan(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            call: Handle<FlowCallFact>,
            sources: HandleSpan<FlowOperandRetainedSupplier>,
            ceilings: &[ReferenceAccess],
        ) -> Result<InputPlan, &'static str> {
            let mut resources = Vec::new();
            let mut parents = Arena::new();
            let control = &facts.flow.control;
            let recorded = control
                .operand_retained_suppliers
                .span(sources)
                .ok_or("completed output lifetime inputs are unavailable")?;
            if recorded.len() != ceilings.len() {
                return Err("completed output input access roster is incomplete");
            }
            for ((handle, supplier), ceiling) in sources.handles().zip(recorded).zip(ceilings) {
                let contract = supplier
                    .contract
                    .ok_or("completed output lifetime input has no original applied contract")?;
                let source_type = crate::borrow::lifetime_contract::source_type(
                    program,
                    &facts.borrow,
                    control,
                    supplier,
                )
                .ok_or(
                    "completed output lifetime input does not match its captured call argument",
                )?;
                let source_path = control
                    .operand_retained_paths
                    .span(contract.source_path)
                    .ok_or("completed output input lost its declared source path")?;
                let leaves = if let Some(shell) = validation::reference_shell(program, source_type)
                {
                    vec![(
                        Vec::new(),
                        checked_trees::BorrowAccessKind::from(shell.access),
                    )]
                } else {
                    validation::borrow_contracts::carried_reference_paths(program, source_type)
                        .ok_or("completed output input has an unresolved reference frontier")?
                        .into_iter()
                        .map(|(path, access)| (crate::borrow::owner_place_segments(&path), access))
                        .collect()
                };
                for (leaf, access) in leaves {
                    let access = bounded_access(*ceiling, access)?;
                    let mut owner_path =
                        crate::borrow::owner_path_from_place_segments(program, source_path);
                    owner_path.extend(crate::borrow::owner_path_from_place_segments(
                        program, &leaf,
                    ));
                    if contract.source == checked_trees::ViewReturnInputSource::Referent {
                        let mut captured_path = control
                            .operand_retained_paths
                            .span(supplier.projection)
                            .ok_or("completed output input lost its captured projection")?
                            .to_vec();
                        captured_path.extend_from_slice(&leaf);
                        append_borrowed(
                            program,
                            facts,
                            loan_occurrences,
                            state,
                            call,
                            handle,
                            Handle::invalid(),
                            contract.input,
                            supplier.operand,
                            &captured_path,
                            &owner_path,
                            access,
                            &mut parents,
                            &mut resources,
                        )?;
                        continue;
                    }
                    let selected = control
                        .operand_stored_sources
                        .span(contract.stored_sources)
                        .ok_or("completed output input lost its frozen stored frontier")?;
                    let mut count = 0;
                    for (stored, member) in contract.stored_sources.handles().zip(selected) {
                        let (FlowOperandStoredSource::Value { path, .. }
                        | FlowOperandStoredSource::Incoming { path, .. }) = member;
                        let path = control
                            .operand_retained_paths
                            .span(*path)
                            .ok_or("stored input lost its selected leaf path")?;
                        let corresponding =
                            validation::corresponding_place_projection(program, source_type, path)
                                .ok_or("stored input leaf has no applied member correspondence")?;
                        let expected =
                            validation::corresponding_place_projection(program, source_type, &leaf)
                                .ok_or(
                                    "declared input leaf has no applied member correspondence",
                                )?;
                        if corresponding != expected {
                            continue;
                        }
                        count += 1;
                        match *member {
                            FlowOperandStoredSource::Value {
                                operand,
                                projection,
                                ..
                            } => {
                                let projection = control
                                    .operand_retained_paths
                                    .span(projection)
                                    .ok_or("stored input lost its original captured projection")?;
                                append_borrowed(
                                    program,
                                    facts,
                                    loan_occurrences,
                                    state,
                                    call,
                                    handle,
                                    stored,
                                    contract.input,
                                    operand,
                                    projection,
                                    &owner_path,
                                    access.clone(),
                                    &mut parents,
                                    &mut resources,
                                )?;
                            }
                            FlowOperandStoredSource::Incoming { slot, .. } => {
                                let call_row = control.calls.get(call);
                                let aliases =
                                    crate::checks::borrows::completed_backings::incoming_sources(
                                        program,
                                        facts,
                                        loan_occurrences,
                                        state,
                                        call_row.statement_index,
                                        call_row.entry_constraints,
                                        validation::CanonicalPlace {
                                            root: slot.root,
                                            segments: control
                                                .operand_referent_segments
                                                .span(slot.segments)
                                                .ok_or(
                                                    "stored input lost its original incoming slot",
                                                )?
                                                .to_vec(),
                                        },
                                    )?;
                                if aliases.is_empty() {
                                    return Err(
                                        "stored input has no incoming authority alternatives",
                                    );
                                }
                                for alias in aliases {
                                    let identity = alias.place.incoming_reference;
                                    let (_, owner) = facts
                                        .borrow
                                        .first_state_borrow(
                                            state.machine_symbol,
                                            state.state_symbol,
                                        )
                                        .ok_or("stored input has no owning borrow state")?;
                                    if !facts.borrow.state_owns_incoming_reference(owner, identity)
                                    {
                                        return Err(
                                            "stored input has no exact incoming entry permission",
                                        );
                                    }
                                    let incoming = facts.borrow.incoming_references.get(identity);
                                    let owner = facts
                                        .borrow
                                        .owner_segments
                                        .span(incoming.owner_path)
                                        .ok_or("stored input has no declared incoming path")?;
                                    let reference = validation::owned_projection_type_reference(
                                        program,
                                        incoming.carrier_type,
                                        &crate::borrow::owner_place_segments(owner),
                                    )
                                    .and_then(|reference| {
                                        validation::reference_shell(program, reference)
                                    })
                                    .ok_or("stored input's incoming identity is not a reference")?;
                                    let expected =
                                        validation::project_type_reference_from_segments(
                                            program,
                                            source_type,
                                            &leaf,
                                        )
                                        .and_then(|reference| {
                                            validation::reference_shell(program, reference)
                                        })
                                        .ok_or("stored input's declared leaf is not a reference")?;
                                    if !validation::storage_origin_types_match(
                                        program,
                                        reference.referee,
                                        expected.referee,
                                    ) {
                                        return Err(
                                            "stored input's incoming descriptor has another referent type",
                                        );
                                    }
                                    if reference.access != ReferenceAccess::Mutable
                                        && reference.access != reference_access(&access)
                                    {
                                        return Err(
                                            "stored input's incoming permission cannot provide the declared access",
                                        );
                                    }
                                    resources.push(InputResourceDraft {
                                        source: handle,
                                        stored,
                                        permission: InputPermissionDraft::Incoming {
                                            place: alias.place,
                                            access: reference_access(&access),
                                            owner_path: owner_path.clone(),
                                        },
                                    });
                                }
                            }
                        }
                    }
                    if count == 0 {
                        return Err("stored input does not cover a declared reference leaf");
                    }
                }
            }
            Ok(InputPlan { resources, parents })
        }

        #[allow(clippy::too_many_arguments)]
        fn append_borrowed(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            loan_occurrences: &LoanOccurrences<'_>,
            state: &FlowStateFact,
            call: Handle<FlowCallFact>,
            source: Handle<FlowOperandRetainedSupplier>,
            stored: Handle<FlowOperandStoredSource>,
            owner: Handle<checked_trees::FlowOperandFact>,
            operand: Handle<checked_trees::FlowOperandFact>,
            path: &[facts::PlaceSegment],
            owner_path: &[checked_trees::BorrowLoanOwnerSegment],
            access: checked_trees::BorrowAccessKind,
            parents: &mut Arena<Handle<BorrowLoanFact>>,
            resources: &mut Vec<InputResourceDraft>,
        ) -> Result<(), &'static str> {
            let original = if stored.is_valid() {
                crate::checks::borrows::formation_captures::actual_invocation(
                    program, facts, state, operand,
                )?
            } else {
                None
            };
            let lending_call = original.unwrap_or(call);
            let (suppliers, captured_parents) = permission_sources::lending_sources(
                program,
                facts, loan_occurrences,
                state,
                lending_call,
                operand,
                path,
                &access,
            ).inspect_err(|reason| {
                if ::validation::trace_flags::check_sites() {
                    eprintln!("CHECKSITE completed-output-input-refused source={source:?} stored={stored:?} owner={owner:?} operand={operand:?} original_call={original:?} lending_call={lending_call:?} path={path:?} access={access:?} reason={}", reason.describe(program));
                    let value = facts.flow.control.operands.get(operand);
                    if facts.flow.control.operand_retentions.is_valid(value.retention) {
                        let profile = facts.flow.control.operand_retentions.get(value.retention);
                        eprintln!("CHECKSITE completed-output-input-profile operand={operand:?} retention={:?} profile={profile:?} suppliers={:?} reborrows={:?}", value.retention,
                            facts.flow.control.operand_retained_suppliers.span_or_empty(profile.suppliers),
                            facts.flow.control.operand_reborrow_sources.span_or_empty(profile.reborrow_sources));
                    }
                }
            })?;
            if suppliers.is_empty() {
                return Err("completed input has no selected lending permission");
            }
            for mut selected in suppliers {
                if let CheckedCallBorrowLoanSource::CallAccess { parents: old, .. } =
                    &mut selected.source
                {
                    let retained = captured_parents
                        .span(*old)
                        .ok_or("completed input permission has an unavailable parent roster")?;
                    *old = parents.insert_many(retained.iter().copied());
                }
                resources.push(InputResourceDraft {
                    source,
                    stored,
                    permission: InputPermissionDraft::Borrowed(ResourceDraft {
                        resource: CheckedCallBorrowLoanResource {
                            machine_symbol: state.machine_symbol,
                            state_symbol: state.state_symbol,
                            call,
                            owner_operand: owner,
                            operand,
                            source: selected.source,
                            access: access.clone(),
                            parent_lifetime: CheckedDirectBorrowParentLifetime {
                                machine_symbol: state.machine_symbol,
                                state_symbol: state.state_symbol,
                                root: selected.backing.root,
                            },
                            ..Default::default()
                        },
                        path: owner_path.to_vec(),
                        backing: selected.backing,
                    }),
                });
            }
            Ok(())
        }
        fn reference_access(access: &checked_trees::BorrowAccessKind) -> ReferenceAccess {
            match access {
                checked_trees::BorrowAccessKind::Read => ReferenceAccess::Shared,
                checked_trees::BorrowAccessKind::Mutable => ReferenceAccess::Mutable,
                checked_trees::BorrowAccessKind::WriteOnly => ReferenceAccess::WriteOnly,
            }
        }
        fn bounded_access(
            ceiling: ReferenceAccess,
            access: checked_trees::BorrowAccessKind,
        ) -> Result<checked_trees::BorrowAccessKind, &'static str> {
            use checked_trees::BorrowAccessKind;
            Ok(match (ceiling, access) {
                (ReferenceAccess::Shared, BorrowAccessKind::WriteOnly)
                | (ReferenceAccess::WriteOnly, BorrowAccessKind::Read) => {
                    return Err(
                        "completed input's access ceiling permits no requested leaf access",
                    );
                }
                (ReferenceAccess::Shared, _) | (_, BorrowAccessKind::Read) => {
                    BorrowAccessKind::Read
                }
                (ReferenceAccess::WriteOnly, _) | (_, BorrowAccessKind::WriteOnly) => {
                    BorrowAccessKind::WriteOnly
                }
                _ => BorrowAccessKind::Mutable,
            })
        }
    }

    pub(super) use completions::install;
    pub(in crate::checks::borrows) use debts::{
        HeldInput, has_resource as has_debt_resource, output_windows, windows as debt_windows,
    };

    pub(super) use crate::borrow::supplied_outputs::normal_call;

    pub(super) fn validate_normal_output_completions(
        borrow: &BorrowFacts,
        flow: &FlowFacts,
    ) -> Result<(), Vec<Diagnostic>> {
        if !borrow
            .loans
            .iter()
            .any(|(_, loan)| loan.lineage == BorrowLoanLineage::SuppliedOutput)
        {
            return Ok(());
        }
        let mut diagnostics = Vec::new();
        for (_, state) in flow.control.states.iter() {
            for actual in flow.control.calls.span_or_empty(state.calls) {
                let Some(source) = borrow.call_at(
                    state.machine_symbol,
                    state.state_symbol,
                    actual.statement_index,
                    actual.call_ordinal,
                ) else {
                    continue;
                };
                let invocation = borrow.calls.get(source);
                for handle in invocation.completed_reference_loans.handles() {
                    if !borrow.loans.is_valid(handle) {
                        continue;
                    }
                    let loan = borrow.loans.get(handle);
                    if loan.lineage != BorrowLoanLineage::SuppliedOutput
                        || !flow
                            .borrow_loan_constraints(actual.exit_constraints)
                            .any(|live| live == handle)
                    {
                        continue;
                    }
                    if actual.target_symbol != invocation.target_symbol
                        || actual.authored_expression != invocation.authored_expression
                        || loan.established_by_call != source
                        || borrow.supplied_output_call(handle) != Some(source)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "supplied output permission has no exact normal-call and initialized-slot correspondence at state {:?}, statement {}, call {}",
                            state.state_symbol, actual.statement_index, actual.call_ordinal,
                        )));
                    }
                }
            }
        }
        crate::checks::finish(diagnostics)
    }
}

use crate::checks::borrows::resources::disposition_events::{
    bind_restored_call_uses, plan_reborrow_disposition_events,
};
use crate::checks::borrows::resources::loan_resources::{
    install_borrow_resources, plan_resource_installation, reconstruct_borrow_resource_rosters,
};
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;

/* Build once before the checks and preserve the resulting handles for lowering. */
pub(super) fn initialize_checked_direct_borrow_resources(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    supplied_permissions::validate_normal_output_completions(&facts.borrow, &facts.flow)?;
    call_operands::install(program, facts);
    supplied_permissions::install(program, facts)?;
    stored_call_inputs::install(program, facts);
    formation_sources::validate(program, facts)?;
    let retentions = forwarded_lifetimes::plan(facts)?;
    let (direct, reborrows) =
        reconstruct_borrow_resource_rosters(program, &facts.borrow, &facts.flow, &retentions)?;
    let installation = plan_resource_installation(&facts.borrow, &direct, &reborrows)?;
    let mut dispositions = plan_reborrow_disposition_events(
        &facts.borrow,
        &facts.flow,
        &direct,
        &reborrows,
        &installation,
    )?;
    bind_restored_call_uses(
        program,
        &facts.borrow,
        &facts.flow,
        &direct,
        &reborrows,
        &mut dispositions,
        mutation_summaries,
    )?;
    install_borrow_resources(
        &mut facts.borrow,
        direct,
        &reborrows,
        &installation,
        &dispositions,
    );
    Ok(())
}

pub(crate) fn reborrow_disposition_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked reborrow resource-lifecycle disposition drifted from semantic-phase replay",
    )]
}

pub(crate) fn reborrow_restored_call_use_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked reborrow restored mutating-call use drifted from exact lifecycle and call replay",
    )]
}

pub(crate) fn reborrow_resource_drift() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "checked direct-reborrow resource closure drifted from independent topological replay",
    )]
}

pub(crate) fn invalid_reborrow_attenuation_diagnostic(
    parent: &checked_trees::BorrowAccessKind,
    child: &checked_trees::BorrowAccessKind,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot derive {} reborrow authority from an exact {} parent loan; allowed direct reborrow access pairs are Read->Read, Mutable->Read, Mutable->Mutable, Mutable->WriteOnly, and WriteOnly->WriteOnly",
        borrow_access_name(child),
        borrow_access_name(parent),
    ))
}

fn borrow_access_name(access: &checked_trees::BorrowAccessKind) -> &'static str {
    match access {
        checked_trees::BorrowAccessKind::Read => "Read",
        checked_trees::BorrowAccessKind::Mutable => "Mutable",
        checked_trees::BorrowAccessKind::WriteOnly => "WriteOnly",
    }
}
