/*
    Nominal and partial-affine Unit cleanup planning.

    This file retains nominal caller metadata from exact checked dispositions.
    `nominal_requirements.rs` derives nominal cleanup requirements and their
    diagnostics, `partial_affine_cleanup.rs` builds the partial affine
    cleanup machine and `cleanup_evidence.rs` recognizes content evidence,
    empty service reach and exact affine discards.
    `actions` selects the ordered live complement and hooks from contextual
    dispositions. `normal_edges` uses that selection at state exits;
    `temporary_actions` completes call-continuation rosters after the executable
    target catalog exists. Neither caller reconstructs a separate cleanup policy.
    Lifetime binders retain checked loan relations but add no runtime parameters;
    they do not prevent an ordinary callable from supplying a cleanup action.
*/

mod actions {

    /*
        Executable action selection from an already checked contextual disposition.
        State exits and temporary call continuations share this geometry and hook
        selection. Storage namespaces differ; cleanup order and prerequisites do not.
        A receiver's nominal hook precedes its structural children; nested hook
        paths are ordered obligations, not overlapping attempts to consume a root.
        Translate descendants relative to the disposed value's type, then attach
        the retained receiver path. Re-resolving a runtime-selected prefix would
        either decline an executable destination or observe it again after the RHS.
        Effect discovery includes every possible case. Executable regions instead
        consume the exact disposition's edge facts: an inactive payload has neither
        a moved region nor a hook. An unknown tag retains a runtime selection
        condition; it never becomes static authority for every possible payload.
    */
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use crate::execution::terminal_unit::types::{ShapeCollector, machine_binders};
    use checked_trees::{
        CheckFacts, CheckedNormalCleanupAction as CleanupAction, CheckedUnitCallableEntry,
        CheckedUnitNominalAffineCleanupPlan, CheckedUnitPartialAffineDiscardPlan,
        CheckedUnitStructuralArgumentSourcePlan as SourcePlan,
    };

    pub(super) struct DispositionActions {
        pub actions: Vec<CleanupAction>,
        pub partition: arena::Handle<checked_trees::CheckedCleanupPartitionPlan>,
    }

    pub(super) fn partition_refs(
        control: &mut checked_trees::CheckedCleanupControlPlans,
        partitions: impl IntoIterator<Item = arena::Handle<checked_trees::CheckedCleanupPartitionPlan>>,
    ) -> arena::HandleSpan<checked_trees::CheckedCleanupPartitionRef> {
        control.partition_refs.insert_many(
            partitions
                .into_iter()
                .map(|partition| checked_trees::CheckedCleanupPartitionRef { partition }),
        )
    }

    /*
        The surrounding dying-owner selection, not a selected subtree path, owns
        home retirement. Resolve that original home independently of each disposed
        child's type, including a partially constructed owner with no whole value.
    */
    pub(super) fn home_retirement(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        disposition: &checked_trees::ContextualCleanupDisposition,
        source: SourcePlan,
    ) -> Option<CleanupAction> {
        let machine = program.machine_by_symbol(disposition.machine)?;
        let statement_index = match disposition.source {
            language_semantics::PermissionEventSource::Call {
                statement_index, ..
            }
            | language_semantics::PermissionEventSource::Statement { statement_index } => {
                statement_index
            }
            _ => disposition.exit.statement_index,
        };
        let reference = validation::canonical_place_type_reference(
            program,
            disposition.state,
            statement_index,
            &validation::CanonicalPlace {
                root: disposition.root,
                segments: Vec::new(),
            },
        )?;
        let type_identity = shapes.add_type(reference, &machine_binders(program, machine), &[])?;
        Some(CleanupAction::RetireHome {
            source,
            type_identity,
        })
    }

    /*
        Keep retirement after all of this home's region actions. A whole discard
        already consumes its binding; do not publish a second retirement for it.
    */
    pub(super) fn finish_home(actions: &mut Vec<CleanupAction>, retirement: CleanupAction) {
        let source = retirement.source();
        if actions.iter().any(|action| {
            action.source() == source
                && matches!(action, CleanupAction::Discard(discard) if discard.path.is_empty() && discard.case_conditions.is_empty())
        }) {
            return;
        }
        let mut selected = actions.iter().filter(|action| action.source() == source);
        if matches!(selected.next(), Some(CleanupAction::InvokeNominal(cleanup)) if cleanup.path.is_empty() && cleanup.case_conditions.is_empty())
            && selected.next().is_none()
        {
            return;
        }
        let position = actions
            .iter()
            .rposition(|action| action.source() == source)
            .map_or(actions.len(), |position| position + 1);
        actions.insert(position, retirement);
    }

    /*
        Keep the exact candidate region beside its ordered actions. The source
        occurrence supplies permission; the emitted frontier later selects liveness.
    */
    pub(super) fn selected_region(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &mut ShapeCollector<'_>,
        handle: arena::Handle<checked_trees::ContextualCleanupDisposition>,
        source: SourcePlan,
        captured_path: Option<&[checked_trees::CheckedUnitStructuralPathSegment]>,
    ) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
        let disposition = facts.contextual_cleanup.dispositions.get(handle);
        let machine = program.machine_by_symbol(disposition.machine)?;
        let statement = match disposition.source {
            language_semantics::PermissionEventSource::Statement { statement_index }
            | language_semantics::PermissionEventSource::Call {
                statement_index, ..
            } => statement_index,
            _ => disposition.exit.statement_index,
        };
        let place = validation::CanonicalPlace {
            root: disposition.root,
            segments: facts
                .flow
                .ownership
                .segments
                .span_or_empty(disposition.path)
                .to_vec(),
        };
        let reference = if disposition.disposed_type.is_valid() {
            disposition.disposed_type
        } else {
            validation::canonical_place_type_reference(
                program,
                disposition.state,
                statement,
                &place,
            )?
        };
        let path = if let Some(path) = captured_path {
            path.to_vec()
        } else if place.segments.is_empty() {
            Vec::new()
        } else {
            crate::execution::terminal_unit::calls::projected_argument_path(
                program,
                disposition.state,
                statement,
                &place,
                None,
                None,
            )?
            .path
        };
        Some(checked_trees::CheckedUnitStructuralArgumentPlan {
            projection_evaluations: Default::default(),
            source,
            path,
            type_identity: shapes.add_stored_type(
                reference,
                &machine_binders(program, machine),
                &[],
            )?,
            access: checked_trees::CheckedStructuralAccess::Owned,
        })
    }

    pub(super) fn disposition_actions(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
        disposition_handle: arena::Handle<checked_trees::ContextualCleanupDisposition>,
        source: SourcePlan,
        place_path: Option<&[checked_trees::CheckedUnitStructuralPathSegment]>,
    ) -> Result<DispositionActions, &'static str> {
        disposition_actions_at(
            program,
            facts,
            targets,
            shapes,
            disposition_handle,
            source,
            place_path,
            None,
        )
    }

    /// The exact returned arm supplies its completed storage cut; all other deaths
    /// retain their original occurrence. Geometry and hook selection share it.
    pub(super) fn disposition_actions_at(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
        disposition_handle: arena::Handle<checked_trees::ContextualCleanupDisposition>,
        source: SourcePlan,
        place_path: Option<&[checked_trees::CheckedUnitStructuralPathSegment]>,
        completed_result: Option<&crate::flow::CompletedCleanupResult<'_>>,
    ) -> Result<DispositionActions, &'static str> {
        let disposition = facts
            .contextual_cleanup
            .dispositions
            .get(disposition_handle);
        let selected_disposition = facts
            .contextual_cleanup
            .is_selected_disposition(disposition_handle);
        let machine = program
            .machine_by_symbol(disposition.machine)
            .ok_or("cleanup disposition has no owning machine")?;
        let binders = machine_binders(program, machine);
        let statement_index = match disposition.source {
            language_semantics::PermissionEventSource::Call {
                statement_index, ..
            }
            | language_semantics::PermissionEventSource::Statement { statement_index } => {
                statement_index
            }
            _ => disposition.exit.statement_index,
        };
        let case_evidence =
            crate::flow::CleanupCaseEvidence::new(program, facts.view(), disposition)
                .ok_or("cleanup has invalid occurrence case evidence")?;
        let case_evidence = if let Some(completed) = completed_result {
            case_evidence
                .at_completed_result(completed)
                .ok_or("cleanup result belongs to another immutable checking context")?
        } else {
            case_evidence
        };
        let selected =
            crate::flow::selected_cleanup_leaves_in(program, facts, disposition, &case_evidence)
                .ok_or("cleanup cannot select its remaining owned leaves")?;
        let case_reachability =
            |segments: &[facts::PlaceSegment]| case_evidence.reachability(segments);
        let place = validation::CanonicalPlace {
            root: disposition.root,
            segments: facts
                .flow
                .ownership
                .segments
                .span(disposition.path)
                .ok_or("cleanup has an invalid owned path")?
                .to_vec(),
        };
        let reference = if disposition.disposed_type.is_valid() {
            disposition.disposed_type
        } else {
            validation::canonical_place_type_reference(
                program,
                disposition.state,
                statement_index,
                &place,
            )
            .ok_or("cleanup cannot resolve its disposed type")?
        };
        let type_identity = shapes
            .add_stored_type(reference, &binders, &[])
            .ok_or("cleanup cannot lower its stored type")?;
        let path = if let Some(place_path) = place_path {
            place_path.to_vec()
        } else if place.segments.is_empty() {
            Vec::new()
        } else {
            crate::execution::terminal_unit::calls::projected_argument_path(
                program,
                disposition.state,
                statement_index,
                &place,
                None,
                None,
            )
            .ok_or("cleanup cannot lower its disposed path")?
            .path
        };
        if case_reachability(&place.segments)
            .ok_or("cleanup cannot establish its carrier case reachability")?
            == validation::CleanupCaseReachability::Inactive
        {
            if !selected.is_empty() {
                return Err("inactive cleanup carrier still has selected hooks");
            }
            let nodes = [validation::CleanupPartitionNode::Scope {
                path: Vec::new(),
                type_identity,
                conditions: Vec::new(),
                steps: Vec::new(),
            }];
            let mut actions = Vec::new();
            let partition = retain_partition(
                shapes,
                disposition_handle,
                source,
                &path,
                type_identity,
                &nodes,
                &mut actions,
            )?;
            return Ok(DispositionActions { actions, partition });
        }
        let mut moved = Vec::new();
        for removed in facts
            .contextual_cleanup
            .exclusions
            .span(disposition.exclusions)
            .ok_or("cleanup has invalid exclusions")?
        {
            let excluded = facts
                .contextual_cleanup
                .segments
                .span(removed.segments)
                .ok_or("cleanup exclusion has an invalid path")?;
            if !excluded.starts_with(place.segments.as_slice())
                && !place.segments.starts_with(excluded)
            {
                continue;
            }
            match case_reachability(excluded)
                .ok_or("cleanup cannot establish excluded case reachability")?
            {
                validation::CleanupCaseReachability::Inactive => continue,
                validation::CleanupCaseReachability::Selected => {}
                validation::CleanupCaseReachability::Runtime => {
                    if removed.cause
                        != checked_trees::ContextualCleanupExclusionCause::Unestablished
                        && !case_evidence.definitely_transferred(excluded)
                    {
                        return Err("cleanup exclusion has no selected transfer custody");
                    }
                }
            }
            let Some(relative_segments) = excluded.strip_prefix(place.segments.as_slice()) else {
                if place.segments.starts_with(excluded) {
                    return Err("cleanup disposition overlaps a removed ancestor");
                }
                continue;
            };
            if relative_segments.is_empty() {
                if removed.root != disposition.root || !selected.is_empty() {
                    return Err("cleanup whole-region exclusion does not identify an empty owner");
                }
                let nodes = [validation::CleanupPartitionNode::Scope {
                    path: Vec::new(),
                    type_identity,
                    conditions: Vec::new(),
                    steps: Vec::new(),
                }];
                let mut actions = Vec::new();
                let partition = retain_partition(
                    shapes,
                    disposition_handle,
                    source,
                    &path,
                    type_identity,
                    &nodes,
                    &mut actions,
                )?;
                return Ok(DispositionActions { actions, partition });
            }
            let ProjectedArgumentPath {
                storage_type: reference,
                path: relative_path,
                ..
            } = crate::execution::terminal_unit::calls::projected_argument_path(
                program,
                disposition.state,
                statement_index,
                &validation::CanonicalPlace {
                    root: facts::PlaceRoot::TypeReference(reference),
                    segments: relative_segments.to_vec(),
                },
                None,
                None,
            )
            .ok_or("cleanup cannot lower its excluded relative path")?;
            moved.push((
                relative_path,
                shapes
                    .add_stored_type(reference, &binders, &[])
                    .ok_or("cleanup cannot lower its excluded type")?,
            ));
        }
        let mut hooks = Vec::new();
        for leaf in selected {
            let crate::flow::CleanupLeaf::Nominal {
                machine: hook,
                path: selected_path,
            } = leaf
            else {
                return Err("cleanup has an uninstantiated symbolic hook");
            };
            let relative_path = selected_path
                .strip_prefix(place.segments.as_slice())
                .ok_or("cleanup hook lies outside its disposed region")?;
            for relative_segments in crate::flow::concrete_cleanup_paths(relative_path)
                .ok_or("cleanup cannot expand its selected hook path")?
            {
                let mut segments = place.segments.clone();
                segments.extend_from_slice(&relative_segments);
                if case_reachability(&segments)
                    .ok_or("cleanup cannot establish hook case reachability")?
                    == validation::CleanupCaseReachability::Inactive
                {
                    continue;
                }
                let selected_place = validation::CanonicalPlace {
                    root: facts::PlaceRoot::TypeReference(reference),
                    segments: relative_segments,
                };
                let ProjectedArgumentPath {
                    storage_type: selected_reference,
                    path: relative_path,
                    ..
                } = crate::execution::terminal_unit::calls::projected_argument_path(
                    program,
                    disposition.state,
                    statement_index,
                    &selected_place,
                    None,
                    None,
                )
                .ok_or("cleanup cannot lower its selected hook path")?;
                let selected_type = shapes
                    .add_type(selected_reference, &binders, &[])
                    .ok_or("cleanup cannot lower its selected hook type")?;
                let mut selected_path = path.clone();
                selected_path.extend(relative_path);
                let mut cleanup = selected_nominal(
                    program,
                    targets,
                    hook,
                    source.clone(),
                    selected_path,
                    selected_type,
                )
                .ok_or("cleanup hook has no executable selection")?;
                cleanup.consumed_claims =
                    consumed_claims(program, facts, disposition, selected_disposition, &segments)
                        .ok_or("cleanup hook has no exact consumed claims")?;
                hooks.push(cleanup);
            }
        }
        let mut partition_paths = Vec::new();
        if disposition.selection.is_valid() {
            let ownership = &facts.flow.ownership;
            let receipt = ownership.owned_selections.get(disposition.selection);
            for transfer in ownership
                .selection_transfers
                .span_or_empty(receipt.transfers)
            {
                if !transfer.source.is_valid() {
                    continue;
                }
                let candidate = ownership.selection_sources.get(transfer.source);
                if Some(candidate.symbol) != disposition.root.as_symbol() {
                    continue;
                }
                let transferred = ownership.segments.span_or_empty(transfer.path);
                let Some(relative) = transferred.strip_prefix(place.segments.as_slice()) else {
                    continue;
                };
                if relative.is_empty() {
                    continue;
                }
                let ProjectedArgumentPath {
                    path: selected_path,
                    ..
                } = crate::execution::terminal_unit::calls::projected_argument_path(
                    program,
                    disposition.state,
                    statement_index,
                    &validation::CanonicalPlace {
                        root: facts::PlaceRoot::TypeReference(reference),
                        segments: relative.to_vec(),
                    },
                    None,
                    None,
                )
                .ok_or("cleanup cannot lower its selected transfer path")?;
                crate::shared::collections::push_unique(&mut partition_paths, selected_path);
            }
        }
        let partition = if moved.is_empty() && hooks.is_empty() && partition_paths.is_empty() {
            validation::CaseSelectedCleanupPartition {
                regions: vec![validation::CaseSelectedCleanupRegion {
                    disposition: CheckedUnitPartialAffineDiscardPlan {
                        partition_leaf: Default::default(),
                        source: source.clone(),
                        path: Vec::new(),
                        type_identity,
                        case_conditions: Default::default(),
                    },
                    conditions: Vec::new(),
                }],
                nodes: vec![
                    validation::CleanupPartitionNode::Scope {
                        path: Vec::new(),
                        type_identity,
                        conditions: Vec::new(),
                        steps: vec![1],
                    },
                    validation::CleanupPartitionNode::Leaf { region_ordinal: 0 },
                ],
            }
        } else {
            let moved = moved
                .iter()
                .map(|(path, identity)| (path.as_slice(), *identity))
                .collect::<Vec<_>>();
            let boundaries = hooks
                .iter()
                .map(|hook| Some((hook.path.strip_prefix(path.as_slice())?, hook.type_identity)))
                .collect::<Option<Vec<_>>>()
                .ok_or("cleanup hook boundary lies outside its disposed path")?;
            let types = shapes
                .types
                .iter()
                .map(|(identity, plan)| (*identity, plan))
                .collect();
            validation::reconstruct_case_selected_cleanup_partition(
                &types,
                &source,
                type_identity,
                &moved,
                &boundaries,
                &partition_paths,
                usize::MAX,
                &|prefix, _, case| {
                    let mut segments = place.segments.clone();
                    segments.extend(checked_case_segments(
                        program,
                        disposition.state,
                        statement_index,
                        reference,
                        prefix,
                        case,
                    )?);
                    case_reachability(&segments)
                },
            )
            .ok_or("cleanup cannot reconstruct its remaining region partition")?
        };
        hooks.sort_unstable_by(|left, right| left.path.cmp(&right.path));
        let mut selected_hooks = 0;
        let mut actions = partition
            .regions
            .into_iter()
            .map(|region| {
                let conditions = region
                    .conditions
                    .into_iter()
                    .map(|condition| {
                        let mut condition_path = path.clone();
                        condition_path.extend(condition.path);
                        checked_trees::CheckedCleanupCaseCondition {
                            path: shapes.cleanup_control.paths.insert_many(condition_path),
                            type_identity: condition.type_identity,
                            case_identity: condition.case_identity,
                        }
                    })
                    .collect::<Vec<_>>();
                let case_conditions = shapes.cleanup_control.conditions.insert_many(conditions);
                let mut residual = region.disposition;
                residual.case_conditions = case_conditions;
                let mut full_path = path.clone();
                full_path.append(&mut residual.path);
                residual.path = full_path;
                if let Ok(position) = hooks.binary_search_by(|hook| hook.path.cmp(&residual.path)) {
                    selected_hooks += 1;
                    let mut hook = hooks[position].clone();
                    hook.case_conditions = case_conditions;
                    CleanupAction::InvokeNominal(hook)
                } else {
                    CleanupAction::Discard(residual)
                }
            })
            .collect::<Vec<_>>();
        if selected_hooks != hooks.len() {
            return Err("cleanup region partition did not select every hook");
        }
        let partition = retain_partition(
            shapes,
            disposition_handle,
            source,
            &path,
            type_identity,
            &partition.nodes,
            &mut actions,
        )?;
        Ok(DispositionActions { actions, partition })
    }

    fn retain_partition(
        shapes: &mut ShapeCollector<'_>,
        disposition: arena::Handle<checked_trees::ContextualCleanupDisposition>,
        source: SourcePlan,
        prefix: &[checked_trees::CheckedUnitStructuralPathSegment],
        type_identity: typed_trees::type_identity::TypeIdentityId,
        nodes: &[validation::CleanupPartitionNode],
        actions: &mut [CleanupAction],
    ) -> Result<arena::Handle<checked_trees::CheckedCleanupPartitionPlan>, &'static str> {
        use checked_trees::{CheckedCleanupPartitionNode as Node, CheckedCleanupPartitionPlan};
        if !matches!(nodes.first(), Some(validation::CleanupPartitionNode::Scope { path, type_identity: root_type, conditions, .. }) if path.is_empty() && *root_type == type_identity && conditions.is_empty())
        {
            return Err("cleanup partition has no exact root scope");
        }
        let control = &mut shapes.cleanup_control;
        let handles = nodes
            .iter()
            .map(|_| control.partition_nodes.insert(Node::default()))
            .collect::<Vec<_>>();
        let mut covered = vec![false; actions.len()];
        for (position, node) in nodes.iter().enumerate() {
            let retained = match node {
                validation::CleanupPartitionNode::Scope {
                    path,
                    type_identity,
                    conditions,
                    steps,
                } => Node::Scope {
                    path: control
                        .paths
                        .insert_many(prefix.iter().chain(path).cloned()),
                    type_identity: *type_identity,
                    conditions: partition_conditions(control, prefix, conditions),
                    steps: partition_steps(control, &handles, steps)?,
                },
                validation::CleanupPartitionNode::Leaf { region_ordinal } => {
                    let visited = covered
                        .get_mut(*region_ordinal)
                        .ok_or("cleanup partition leaf exceeds its authorized action span")?;
                    if std::mem::replace(visited, true) {
                        return Err("cleanup partition repeats an authorized action leaf");
                    }
                    Node::Leaf {
                        ordinal: u32::try_from(*region_ordinal)
                            .map_err(|_| "cleanup partition action ordinal exceeds u32")?,
                    }
                }
                validation::CleanupPartitionNode::Cases {
                    path,
                    type_identity,
                    arms,
                    common,
                } => {
                    let arms = arms
                        .iter()
                        .map(|arm| {
                            Ok(checked_trees::CheckedCleanupPartitionArm {
                                case_identity: arm.case_identity.clone(),
                                reachability: match arm.reachability {
                                    validation::CleanupCaseReachability::Inactive => {
                                        checked_trees::CheckedCleanupCaseReachability::Inactive
                                    }
                                    validation::CleanupCaseReachability::Selected => {
                                        checked_trees::CheckedCleanupCaseReachability::Selected
                                    }
                                    validation::CleanupCaseReachability::Runtime => {
                                        checked_trees::CheckedCleanupCaseReachability::Runtime
                                    }
                                },
                                steps: partition_steps(control, &handles, &arm.steps)?,
                            })
                        })
                        .collect::<Result<Vec<_>, &'static str>>()?;
                    Node::Cases {
                        path: control
                            .paths
                            .insert_many(prefix.iter().chain(path).cloned()),
                        type_identity: *type_identity,
                        arms: control.partition_arms.insert_many(arms),
                        common: partition_steps(control, &handles, common)?,
                    }
                }
            };
            *control.partition_nodes.get_mut(handles[position]) = retained;
        }
        if covered.iter().any(|visited| !visited) {
            return Err("cleanup partition omits an authorized action leaf");
        }
        let path = control.paths.insert_many(prefix.iter().cloned());
        let owner = control.partitions.insert(CheckedCleanupPartitionPlan {
            disposition,
            source,
            path,
            type_identity,
            root: handles[0],
            actions: Default::default(),
        });
        for (position, action) in actions.iter_mut().enumerate() {
            let leaf = checked_trees::CheckedCleanupPartitionLeaf {
                partition: owner,
                ordinal: u32::try_from(position)
                    .map_err(|_| "cleanup partition action ordinal exceeds u32")?,
            };
            match action {
                CleanupAction::Discard(discard) => discard.partition_leaf = leaf,
                CleanupAction::InvokeNominal(cleanup) => cleanup.partition_leaf = leaf,
                CleanupAction::RetireHome { .. } => {
                    return Err("cleanup partition includes a separate home retirement");
                }
            }
        }
        let retained_actions = control.actions.insert_many(actions.iter().cloned());
        control.partitions.get_mut(owner).actions = retained_actions;
        Ok(owner)
    }

    fn partition_steps(
        control: &mut checked_trees::CheckedCleanupControlPlans,
        nodes: &[arena::Handle<checked_trees::CheckedCleanupPartitionNode>],
        steps: &[usize],
    ) -> Result<arena::HandleSpan<checked_trees::CheckedCleanupPartitionStep>, &'static str> {
        let rows = steps
            .iter()
            .map(|position| {
                Ok(checked_trees::CheckedCleanupPartitionStep {
                    node: *nodes
                        .get(*position)
                        .ok_or("cleanup partition child exceeds its owner node roster")?,
                })
            })
            .collect::<Result<Vec<_>, &'static str>>()?;
        Ok(control.partition_steps.insert_many(rows))
    }

    fn partition_conditions(
        control: &mut checked_trees::CheckedCleanupControlPlans,
        prefix: &[checked_trees::CheckedUnitStructuralPathSegment],
        conditions: &[validation::CleanupCaseCondition],
    ) -> arena::HandleSpan<checked_trees::CheckedCleanupCaseCondition> {
        let rows = conditions
            .iter()
            .map(|condition| checked_trees::CheckedCleanupCaseCondition {
                path: control
                    .paths
                    .insert_many(prefix.iter().chain(&condition.path).cloned()),
                type_identity: condition.type_identity,
                case_identity: condition.case_identity.clone(),
            })
            .collect::<Vec<_>>();
        control.conditions.insert_many(rows)
    }

    /*
        Join normalized cleanup geometry back to the exact checked source place.
        Matching declaration identities is required: same-shaped fields or case
        payloads are not interchangeable evidence namespaces. Geometry generates
        only fields, cases and fixed indices beneath the retained disposed value;
        dynamic destination coordinates stay in the caller's captured prefix.
    */
    fn checked_case_segments(
        program: &dyn ::validation::CheckingInput,
        state: symbols::SymbolHandle,
        statement_index: usize,
        root_type: typed_trees::types::TypeReferenceHandle,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
        selected_case: &checked_trees::CheckedMemberIdentity,
    ) -> Option<Vec<facts::PlaceSegment>> {
        let case = checked_trees::CheckedUnitStructuralPathSegment::Case(selected_case.clone());
        checked_path_segments(
            program,
            state,
            statement_index,
            root_type,
            path.iter().chain(std::iter::once(&case)),
        )
    }

    pub(in crate::execution::terminal_unit) fn checked_path_segments<'a>(
        program: &dyn validation::CheckingInput,
        state: symbols::SymbolHandle,
        statement_index: usize,
        root_type: typed_trees::types::TypeReferenceHandle,
        path: impl IntoIterator<Item = &'a checked_trees::CheckedUnitStructuralPathSegment>,
    ) -> Option<Vec<facts::PlaceSegment>> {
        use checked_trees::CheckedUnitStructuralPathSegment;
        use typed_trees::data::DataMember;

        let mut place = validation::CanonicalPlace {
            root: facts::PlaceRoot::TypeReference(root_type),
            segments: Vec::new(),
        };
        let mut payload_fields = None;
        for segment in path {
            match segment {
                CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                    if payload_fields.is_some() {
                        return None;
                    }
                    place.segments.push(facts::PlaceSegment::FixedIndex {
                        index: usize::try_from(*index).ok()?,
                    });
                }
                CheckedUnitStructuralPathSegment::Field(identity) => {
                    let field = if let Some(fields) = payload_fields.take() {
                        let fields: &[typed_trees::data::DataField] = fields;
                        fields.iter().find(|field| {
                            checked_trees::CheckedMemberIdentity::of_member(
                                field.identity,
                                field.name.clone(),
                            ) == *identity
                        })?
                    } else {
                        let reference = validation::canonical_place_type_reference(
                            program,
                            state,
                            statement_index,
                            &place,
                        )?;
                        let owner = program.closed_data_definition(
                            validation::unwrapped_type_reference(program, reference)?,
                        )?;
                        program
                            .data_members(owner)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Field(field)
                                    if checked_trees::CheckedMemberIdentity::of_member(
                                        field.identity,
                                        field.name.clone(),
                                    ) == *identity =>
                                {
                                    Some(field)
                                }
                                _ => None,
                            })?
                    };
                    if field.relevance.is_erased() || !field.symbol.is_valid() {
                        return None;
                    }
                    place.segments.push(facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                }
                CheckedUnitStructuralPathSegment::Case(identity) => {
                    if payload_fields.is_some() {
                        return None;
                    }
                    let reference = validation::canonical_place_type_reference(
                        program,
                        state,
                        statement_index,
                        &place,
                    )?;
                    let owner = program.closed_data_definition(
                        validation::unwrapped_type_reference(program, reference)?,
                    )?;
                    let case =
                        program
                            .data_members(owner)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Variant(case)
                                    if checked_trees::CheckedMemberIdentity::of_member(
                                        case.identity,
                                        case.name.clone(),
                                    ) == *identity =>
                                {
                                    Some(case)
                                }
                                _ => None,
                            })?;
                    if !case.symbol.is_valid() {
                        return None;
                    }
                    place.segments.push(facts::PlaceSegment::Case {
                        variant: case.symbol,
                    });
                    payload_fields = Some(program.data_payload_fields(case));
                }
                _ => return None,
            }
        }
        Some(place.segments)
    }

    fn selected_nominal(
        program: &dyn ::validation::CheckingInput,
        targets: &[CheckedUnitCallableEntry<'_>],
        hook: symbols::SymbolHandle,
        source: SourcePlan,
        path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        type_identity: typed_trees::type_identity::TypeIdentityId,
    ) -> Option<CheckedUnitNominalAffineCleanupPlan> {
        let machine = program.machine_by_symbol(hook)?;
        let state = program.machine_states(machine).first()?;
        let target = super::callable_target(targets, hook)?;
        if !super::nominal_cleanup_target_matches(program, machine, state, &target, type_identity) {
            return None;
        }
        Some(CheckedUnitNominalAffineCleanupPlan {
            partition_leaf: Default::default(),
            consumed_claims: Vec::new(),
            source,
            path,
            type_identity,
            cleanup_machine: hook,
            cleanup_state: state.symbol,
            cleanup_contract_report_fingerprint: target.contract_report_fingerprint,
            case_conditions: Default::default(),
        })
    }

    /*
        Keep the source-authorized claim names attached to the exact hook region.
        The emitter resolves these names in the current invocation; neither type
        multiplicity nor a later storage lookup may manufacture a consumption.
    */
    fn consumed_claims(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        disposition: &checked_trees::ContextualCleanupDisposition,
        selected_disposition: bool,
        path: &[facts::PlaceSegment],
    ) -> Option<Vec<language_semantics::PermissionClaimIdentity>> {
        use language_semantics::{
            Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
            PermissionEventSource, PermissionProvenance,
        };
        let evaluation = if disposition.source == PermissionEventSource::StateExit {
            let state = program.state_by_symbol(disposition.state)?;
            Some(match disposition.exit.kind {
                checked_trees::FlowClaimJoinExitKind::TransitionTarget
                | checked_trees::FlowClaimJoinExitKind::TransitionContinuation => {
                    let transition = program
                        .statement_table
                        .statement_at(state, disposition.exit.statement_index)?
                        .as_transition()?;
                    checked_trees::FlowPermissionEvaluation::TransitionTarget(
                        if disposition.exit.kind
                            == checked_trees::FlowClaimJoinExitKind::TransitionTarget
                        {
                            transition.target
                        } else {
                            transition.continuation
                        },
                    )
                }
                _ => checked_trees::FlowPermissionEvaluation::Statement,
            })
        } else {
            None
        };
        let mut claims = Vec::new();
        let permissions = if selected_disposition {
            &facts.flow.ownership.selected_cleanup_permissions
        } else {
            &facts.flow.ownership.permissions
        };
        for (_, event) in permissions.iter().filter(|(_, event)| {
            event.machine_symbol == disposition.machine
                && event.state_symbol == disposition.state
                && event.source == disposition.source
                && evaluation.is_none_or(|evaluation| event.evaluation == evaluation)
                && event.root == disposition.root
                && event.kind == PermissionEventKind::Consume
                && event.multiplicity == Multiplicity::Linear
                && event.access == PermissionAccess::Owned
                && facts.flow.ownership.segments.span(event.segments) == Some(path)
        }) {
            if event.claim_identity == PermissionClaimIdentity::Unknown
                || event.provenance == PermissionProvenance::Unknown
                || event.obligation_live
                || claims.contains(&event.claim_identity)
            {
                return None;
            }
            claims.push(event.claim_identity);
        }
        Some(claims)
    }
}
pub(in crate::execution::terminal_unit) use actions::checked_path_segments;
pub(super) mod anonymous {

    /*
        Anonymous argument owners die at the consuming call's normal continuation.
        The nested schedule establishes each owner once; a projected argument names
        its subtree without establishing another owner. Borrowed owners retire whole,
        while partial transfers retire the maximal untouched complement, in reverse
        establishment order. Captured reference descriptors use the shared loan
        retirement schedule, which keeps them live through returned descendants.
        Loan capture records retained ancestry, not a completed physical release.
        Normal completion retires each eligible descriptor before its backing;
        a later loan schedule observes that actual event rather than duplicating it.
        Checking selects the dying owner's complement once. Value planning retains
        that home's completion request; action finalization consumes the contextual
        disposition after executable hook targets exist. It does not reconstruct a
        second permission partition or dispose a selected live linear obligation.
    */
    use super::super::CheckedUnitStructuralResultBindingPlan;
    use super::{
        CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan as OperationPlan,
        CheckedUnitStructuralArgumentSourcePlan as SourcePlan, Multiplicity, PermissionEventSource,
    };
    use crate::execution::terminal_unit::types::{ShapeCollector, base_type_identity};
    use crate::flow::indexed_statements;
    use checked_trees::CheckedNormalCleanupAction;

    /*
        Retain the original anonymous owner's completion request. Finalization
        resolves its checked contextual disposition in the actual call namespace;
        a selected subtree never becomes a second whole temporary.
    */
    pub(crate) fn projected_temporary_cleanup(
        program: &dyn ::validation::CheckingInput,
        owner_type: typed_trees::types::TypeReferenceHandle,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    ) -> Option<Vec<CheckedNormalCleanupAction>> {
        let SourcePlan::StructuralExpression { .. } = argument.source else {
            return None;
        };
        if argument.access != CheckedStructuralAccess::Owned
            || argument.path.is_empty()
            || program.type_multiplicity(owner_type) != Multiplicity::Affine
            || !validation::has_contents(
                program,
                owner_type,
                &[],
                validation::ContentsRequirement::BorrowFree,
            )
        {
            return None;
        }
        let mut shapes = ShapeCollector::new(program);
        let identity = shapes.add_type(owner_type, &[], &[])?;
        Some(vec![CheckedNormalCleanupAction::RetireHome {
            source: argument.source.clone(),
            type_identity: identity,
        }])
    }

    /*
        Dispose anonymous argument owners at the call's normal continuation.
        The argument's access determines whether the whole owner or its maximal
        complement dies. Captured reference descriptors instead keep their one
        release in the loan schedule, including any returned descendant's lifetime.
        Results follow establishment order, so reverse traversal also orders mixed
        borrowed owners and partially transferred owners without regrouping them.
    */
    #[allow(clippy::too_many_arguments)]
    pub(in crate::execution::terminal_unit) fn insert_continuation(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
        insertion: usize,
        results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
        operations: &mut Vec<OperationPlan>,
        trace: &super::super::control::LocalConstructionTrace,
    ) -> Option<()> {
        let invocation = operations
            .get(..insertion)?
            .iter()
            .rev()
            .find(|operation| operation.invocation_coordinate() == Some(coordinate))?
            .clone();
        let completions = normal_call_completion(
            program,
            facts,
            shapes,
            machine,
            state,
            results,
            operations,
            &invocation,
            trace,
        )?;
        operations.splice(insertion..insertion, completions);
        Some(())
    }

    /*
        Flat statement calls and nested value calls publish the same normal-return
        transaction: release eligible operand descriptors, then dispose the exact
        backing owners selected by checking. The result's retained inputs stay live.
    */
    #[allow(clippy::too_many_arguments)]
    pub(in crate::execution::terminal_unit) fn normal_call_completion(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
        operations: &mut [OperationPlan],
        invocation: &OperationPlan,
        trace: &super::super::control::LocalConstructionTrace,
    ) -> Option<Vec<OperationPlan>> {
        let caller_trace = trace.mark();
        let coordinate = invocation.invocation_coordinate()?;
        let completion_phase = |phase| {
            trace.phase(phase);
            trace.statement(Some(coordinate.statement_index));
        };
        if !invocation.has_normal_continuation() {
            return Some(Vec::new());
        }
        completion_phase("normal completion: operand descriptors");
        let mut completions = super::temporary_descriptors::completions(
            program,
            facts,
            machine.symbol,
            state.symbol,
            results,
            invocation,
            operations,
            shapes,
        )?;

        completion_phase("normal completion: selected descriptor producers");
        let mut selected_descriptors = Vec::new();
        completion_phase("normal completion: descriptor producer");
        for completion in completions {
            let OperationPlan::ReleaseReference {
                source: source @ SourcePlan::StructuralResult { binding_ordinal },
                ..
            } = &completion
            else {
                return None;
            };
            if operations
                .iter()
                .flat_map(OperationPlan::with_value_calls)
                .any(|operation| {
                    matches!(operation, OperationPlan::ReleaseReference {
                    source: released, path, defer_to_exit: false, ..
                } if released == source && path.is_empty())
                })
            {
                continue;
            }
            {
                let mut producers = operations
                    .iter()
                    .flat_map(OperationPlan::with_value_calls)
                    .filter_map(OperationPlan::structural_result_binding)
                    .filter(|binding| binding.binding_ordinal == *binding_ordinal);
                producers.next()?;
                if producers.next().is_some() {
                    return None;
                }
            }
            for operation in operations.iter_mut() {
                if let Some((_, discard)) =
                    operation.discardable_structural_result_by_ordinal_mut(*binding_ordinal)
                {
                    *discard = false;
                }
            }
            selected_descriptors.push(completion);
        }
        completions = selected_descriptors;
        completion_phase("normal completion: temporary owner roster");
        let statement = coordinate.statement_index;
        let values = &facts.values.structural_values;
        let mut dying = Vec::new();
        facts.contextual_cleanup.for_each_state_disposition(
            super::owning_cleanup_machine(program, state.symbol),
            state.symbol,
            |_, disposition| {
                if !matches!(
                    disposition.source,
                    PermissionEventSource::Call {
                        statement_index,
                        call_ordinal,
                        ..
                    } if statement_index == statement as usize
                        && call_ordinal == coordinate.call_ordinal as usize
                ) {
                    return;
                }
                let Some(expression) = disposition.root.as_expression() else {
                    return;
                };
                crate::shared::collections::push_unique(&mut dying, expression);
            },
        );
        if dying.is_empty() {
            completion_phase("normal completion: finished");
            trace.restore(&caller_trace);
            return Some(completions);
        }

        completion_phase("normal completion: retained operand dependencies");
        let mut retained = Vec::new();
        for argument in invocation.structural_operands() {
            completion_phase("normal completion: operand result bindings");
            let root = if let Some(ordinal) = argument.source_structural_result_binding_ordinal() {
                let mut bindings = results
                    .iter()
                    .filter(|(result, _)| result.binding_ordinal == ordinal);
                let (_, root) = bindings.next()?;
                if bindings.next().is_some() {
                    return None;
                }
                *root
            } else if let Some(root) = argument.structural_storage_root() {
                root
            } else {
                continue;
            };
            let Some(expression) = root.as_expression() else {
                continue;
            };
            let Some(root) = values.root_for_expression(state.symbol, statement, expression) else {
                continue;
            };
            completion_phase("normal completion: operand value graph");
            retained.extend(values.value_dependencies(root.root)?);
        }
        completion_phase("normal completion: invocation lookup");
        let flow = facts.flow.control.state(machine.symbol, state.symbol)?;
        let mut source_calls = facts
            .flow
            .control
            .calls
            .span(flow.calls)?
            .iter()
            .filter(|source| {
                source.statement_index == statement as usize
                    && source.call_ordinal == coordinate.call_ordinal as usize
            });
        let source_call = source_calls.next()?;
        if source_calls.next().is_some() {
            return None;
        }
        completion_phase("normal completion: input lifetime roots");
        let mut retained_owners = Vec::new();
        completion_phase("normal completion: input lifetimes");
        for operand in facts.flow.control.operands.span(source_call.operands)? {
            let roots = crate::flow::captured_lifetime_roots(program, &facts.flow.control, operand);
            if roots.is_none() && ::validation::trace_flags::check_sites() {
                eprintln!(
                    "CHECKSITE normal-completion-input-lifetime machine={} state={} call={:?} operand={:?} profile={:?}",
                    machine.name,
                    state.name,
                    source_call,
                    operand,
                    facts
                        .flow
                        .control
                        .operand_retentions
                        .is_valid(operand.retention)
                        .then(|| facts.flow.control.operand_retentions.get(operand.retention)),
                );
            }
            for root in roots? {
                crate::shared::collections::push_unique(&mut retained_owners, root);
            }
        }
        completion_phase("normal completion: completed expression lookup");
        let (_, statement_node) = indexed_statements(program, state.statement_nodes)
            .find(|(position, _)| *position == statement as usize)?;
        let completed = crate::checks::ownership_occurrences::completed_expressions(
            program,
            machine,
            state,
            statement as usize,
            statement_node,
            &facts.operators,
        );
        if dying.iter().any(|expression| {
            completed
                .iter()
                .filter(|value| *value == expression)
                .count()
                != 1
        }) {
            return None;
        }
        dying.sort_by_key(|expression| {
            std::cmp::Reverse(completed.iter().position(|value| value == expression))
        });
        let mut cleanup_actions = Vec::new();
        for expression in dying {
            completion_phase("normal completion: owner value graph");
            let root = values.root_for_expression(state.symbol, statement, expression)?;
            completion_phase("normal completion: owner retained association");
            if root.machine != machine.symbol
                || (!retained.contains(&root.root)
                    && !retained_owners.contains(&facts::PlaceRoot::Expression(expression)))
            {
                return None;
            }
            completion_phase("normal completion: owner result binding");
            let mut bindings = results.iter().filter(|(result, source)| {
                result.statement_index == statement
                    && *source == facts::PlaceRoot::Expression(expression)
            });
            let source = if let Some((result, _)) = bindings.next() {
                if bindings.next().is_some() {
                    return None;
                }
                let source = SourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                };
                if operations
                    .iter()
                    .flat_map(OperationPlan::with_value_calls)
                    .chain(completions.iter())
                    .any(|operation| {
                        matches!(operation, OperationPlan::ReleaseReference {
                            source: released, path, defer_to_exit: false, ..
                        } if *released == source && path.is_empty())
                    })
                {
                    continue;
                }
                completion_phase("normal completion: owner producer lookup");
                {
                    let mut producers = operations
                        .iter()
                        .flat_map(OperationPlan::with_value_calls)
                        .filter_map(OperationPlan::discardable_structural_result)
                        .filter(|(produced, _)| produced.binding_ordinal == result.binding_ordinal);
                    let (produced, _) = producers.next()?;
                    completion_phase("normal completion: owner producer agreement");
                    if producers.next().is_some() || *produced != *result {
                        return None;
                    }
                }
                let (_, discard_on_return) = operations.iter_mut().find_map(|operation| {
                    operation.discardable_structural_result_by_ordinal_mut(result.binding_ordinal)
                })?;
                *discard_on_return = false;
                SourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                }
            } else {
                SourcePlan::StructuralExpression {
                    value_dependency: None,
                    expression,
                    logical_root_type_identity: None,
                }
            };
            completion_phase("normal completion: owner prior continuation");
            if operations.iter().flat_map(OperationPlan::with_value_calls).any(|operation| matches!(operation,
                OperationPlan::ContinuationCleanup { cleanup_actions, .. }
                    if cleanup_actions.iter().any(|action| action.source() == &source && action.path().is_empty())))
            { return None; }
            completion_phase("normal completion: owner cleanup type");
            cleanup_actions.push(CheckedNormalCleanupAction::RetireHome {
                source,
                type_identity: base_type_identity(
                    program,
                    root.type_reference,
                    &crate::execution::terminal_unit::types::machine_binders(program, machine),
                    &[],
                )?,
            });
        }
        if !cleanup_actions.is_empty() {
            completions.push(OperationPlan::ContinuationCleanup {
                partitions: Default::default(),
                selected_regions: Vec::new(),
                boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate),
                cleanup_actions,
            });
        }
        completion_phase("normal completion: finished");
        trace.restore(&caller_trace);
        Some(completions)
    }
}
pub(crate) use anonymous::projected_temporary_cleanup;
mod cleanup_evidence {
    /*
        Content evidence, empty service reach and exact affine discards.
    */
    use crate::execution::permission_events;
    use crate::execution::terminal_unit::{
        CheckFacts, Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
        PermissionEventSource, StateParameter, SymbolHandle,
    };

    pub(crate) fn machine_has_content_evidence(
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> bool {
        facts
            .qualifications
            .content
            .identity_reshuffles
            .iter()
            .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
            || facts
                .qualifications
                .content
                .partition_returns
                .iter()
                .any(|fact| fact.plan.owner == machine && fact.plan.callable == state)
    }

    pub(crate) fn has_exact_root_affine_discard(
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameter: &StateParameter,
    ) -> bool {
        has_exact_symbol_affine_discard(
            facts,
            machine,
            state,
            parameter.symbol,
            language_semantics::PermissionProvenance::Unknown,
        )
    }

    pub(crate) fn has_exact_symbol_affine_discard(
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        symbol: SymbolHandle,
        provenance: language_semantics::PermissionProvenance,
    ) -> bool {
        let matching = permission_events(facts)
            .filter(|(_, event)| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.source == PermissionEventSource::StateExit
                    && event.kind == PermissionEventKind::AffineDrop
                    && event.access == PermissionAccess::Owned
                    && event.multiplicity == Multiplicity::Affine
                    && event.claim_identity == PermissionClaimIdentity::Unknown
                    && event.provenance == provenance
                    && !event.obligation_live
                    && event.root == facts::PlaceRoot::Symbol(symbol)
            })
            .map(|(_, event)| event)
            .collect::<Vec<_>>();
        let [event] = matching.as_slice() else {
            return false;
        };
        facts
            .flow
            .ownership
            .segments
            .span_or_empty(event.segments)
            .is_empty()
    }

    /*
        An empty consuming body must dispose of the same whole claim that entered.
        Type-owner selection and contextual hook admission remain separate checks;
        this correspondence prevents a cleanup plan from silently losing linear debt.
    */
    pub(super) fn has_exact_root_nominal_disposition(
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameter: &StateParameter,
        multiplicity: Multiplicity,
        parameter_index: u32,
        entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
    ) -> bool {
        let claims = entry_claims
            .iter()
            .filter(|claim| claim.parameter_index == parameter_index)
            .collect::<Vec<_>>();
        if multiplicity == Multiplicity::Affine {
            return claims.is_empty()
                && has_exact_root_affine_discard(facts, machine, state, parameter);
        }
        let [claim] = claims.as_slice() else {
            return false;
        };
        if multiplicity != Multiplicity::Linear
            || !claim.path.is_empty()
            || claim.claim_identity == PermissionClaimIdentity::Unknown
        {
            return false;
        }
        let root = facts::PlaceRoot::Symbol(parameter.symbol);
        let mut matching = permission_events(facts).filter(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.root == root
                && event.source == PermissionEventSource::StateExit
        });
        let Some((_, event)) = matching.next() else {
            return false;
        };
        matching.next().is_none()
            && event.evaluation == checked_trees::FlowPermissionEvaluation::Statement
            && event.kind == PermissionEventKind::Consume
            && event.access == PermissionAccess::Owned
            && event.multiplicity == Multiplicity::Linear
            && event.claim_identity == claim.claim_identity
            && event.provenance != language_semantics::PermissionProvenance::Unknown
            && event.segments.is_empty()
            && !event.obligation_live
    }
}
mod descriptor_retirement {
    /*
        Intrinsic descriptor lifetime at an admitted consumer's normal completion.

        Calls and scalar observations consume the same fresh permission carrier.
        Logical copyability does not keep its physical lifetime open until return.
        This selects disposal of that carrier only, never its referent or a nominal
        backing owner's hook. Ordinary source loan checking still owns availability.
    */
    pub(crate) fn descriptor_retirement(
        program: &dyn validation::CheckingInput,
        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<Vec<checked_trees::CheckedNormalCleanupAction>> {
        validation::reference_shell(program, reference)?;
        match validation::reference_result_custody::result_multiplicity(program, reference) {
            language_semantics::Multiplicity::Unrestricted => Some(Vec::new()),
            language_semantics::Multiplicity::Affine => {
                Some(vec![checked_trees::CheckedNormalCleanupAction::Discard(
                    checked_trees::CheckedUnitPartialAffineDiscardPlan {
                        partition_leaf: Default::default(),
                        source,
                        path: Vec::new(),
                        type_identity: validation::structural_type_identity(
                            program,
                            reference,
                            &[],
                            &[],
                        ),
                        case_conditions: Default::default(),
                    },
                )])
            }
            language_semantics::Multiplicity::Linear => None,
        }
    }
}
pub(crate) use descriptor_retirement::descriptor_retirement;
mod nominal_requirements {
    /*
        Nominal cleanup Boolean and caller requirements with their
        diagnostics.
    */
    use crate::execution::terminal_unit::types::terminal_field_identity;
    use crate::execution::terminal_unit::{
        CheckFacts, CheckedUnitNominalAffineCallerRequirementPlan as RequirementPlan,
        CheckedUnitNominalAffineCleanupRequirementPlan, ContractProofFactKind,
        ContractProofFactOwner, DataMember, Diagnostic, ExpressionNode, PrimitiveType,
        StateParameter, SymbolHandle,
    };
    use crate::published::places::place_at;

    pub(crate) fn nominal_cleanup_boolean_requirements(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        cleanup_machine: &typed_trees::machine::Machine,
        cleanup_state: &typed_trees::state::State,
        cleanup_receiver: &StateParameter,
    ) -> Option<Vec<CheckedUnitNominalAffineCleanupRequirementPlan>> {
        let checked_requires = checked_requires_expressions(
            program,
            facts,
            cleanup_machine.symbol,
            cleanup_state.symbol,
        )?;
        let requirements = checked_requires
            .into_iter()
            .map(|expression| {
                direct_boolean_field_requirement(
                    program,
                    cleanup_state.symbol,
                    cleanup_receiver,
                    expression,
                )
            })
            .collect::<Option<Vec<_>>>()?;
        Some(canonical_nominal_cleanup_requirements(requirements))
    }

    pub(crate) fn nominal_cleanup_caller_boolean_requirements(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        caller_machine: &typed_trees::machine::Machine,
        caller_state: &typed_trees::state::State,
        source_parameters: &[StateParameter],
    ) -> Option<Vec<RequirementPlan>> {
        let caller_requires = checked_requires_expressions(
            program,
            facts,
            caller_machine.symbol,
            caller_state.symbol,
        )?;
        let mut structural_requirements = Vec::new();
        for expression in caller_requires {
            if let Some(requirement) = source_parameters.iter().enumerate().find_map(
                |(source_parameter_index, source_parameter)| {
                    let source_parameter_index = u32::try_from(source_parameter_index).ok()?;
                    direct_boolean_field_requirement(
                        program,
                        caller_state.symbol,
                        source_parameter,
                        expression,
                    )
                    .map(|requirement| RequirementPlan {
                        source_parameter_index,
                        field_identity: requirement.field_identity,
                        expected: requirement.expected,
                    })
                },
            ) {
                structural_requirements.push(requirement);
                continue;
            }
            return None;
        }
        let contextual = &facts.contextual_cleanup.entry_requirements;
        for (_, requirement) in contextual.rows.iter() {
            if requirement.machine != caller_machine.symbol {
                continue;
            }
            if requirement.antecedent != checked_trees::CheckedBooleanExpression::Constant(true) {
                return None;
            }
            let (parameter_position, path, expected) = requirement.direct_boolean_subject()?;
            let [checked_trees::CheckedStructuralPredicatePathSegment::Field(field_identity)] =
                path
            else {
                return None;
            };
            structural_requirements.push(RequirementPlan {
                source_parameter_index: parameter_position,
                field_identity: field_identity.clone(),
                expected,
            });
        }
        structural_requirements.sort_by(|left, right| {
            left.source_parameter_index
                .cmp(&right.source_parameter_index)
                .then(left.field_identity.cmp(&right.field_identity))
                .then(left.expected.cmp(&right.expected))
        });
        structural_requirements.dedup();
        Some(structural_requirements)
    }

    pub(crate) fn nominal_cleanup_missing_requirement(
        source_parameter_index: u32,
        caller_requirements: &[RequirementPlan],
        required: &[CheckedUnitNominalAffineCleanupRequirementPlan],
    ) -> Option<CheckedUnitNominalAffineCleanupRequirementPlan> {
        required
            .iter()
            .find(|requirement| {
                !caller_requirements.iter().any(|caller| {
                    caller.source_parameter_index == source_parameter_index
                        && caller.field_identity == requirement.field_identity
                        && caller.expected == requirement.expected
                })
            })
            .cloned()
    }

    pub(crate) fn canonical_nominal_cleanup_requirements(
        mut requirements: Vec<CheckedUnitNominalAffineCleanupRequirementPlan>,
    ) -> Vec<CheckedUnitNominalAffineCleanupRequirementPlan> {
        requirements.sort_by(|left, right| {
            left.field_identity
                .cmp(&right.field_identity)
                .then(left.expected.cmp(&right.expected))
        });
        requirements.dedup();
        requirements
    }

    pub(crate) fn nominal_cleanup_missing_requirement_diagnostic(
        program: &dyn ::validation::CheckingInput,
        caller_machine: &typed_trees::machine::Machine,
        caller_state: &typed_trees::state::State,
        source_parameter: &StateParameter,
        cleanup_machine: &typed_trees::machine::Machine,
        missing: CheckedUnitNominalAffineCleanupRequirementPlan,
    ) -> Diagnostic {
        let edge = format!(
            "automatic cleanup requires at Unit return edge from {} state {} after statement 0",
            crate::published::labels::machine_name(program, caller_machine.symbol),
            crate::published::labels::symbol_name(program, caller_state.symbol),
        );
        Diagnostic::error(format!(
            "cannot prove {edge}: missing {}.{} == {} required by {}",
            source_parameter.name.as_str(),
            missing.field_identity.spelling(),
            missing.expected,
            crate::published::labels::machine_name(program, cleanup_machine.symbol),
        ))
    }

    pub(crate) fn checked_requires_expressions(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Option<Vec<typed_trees::expression::ExpressionHandle>> {
        checked_contract_expressions(
            program,
            facts,
            machine,
            state,
            ContractProofFactKind::Requires,
        )
    }

    pub(crate) fn checked_contract_expressions(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        kind: ContractProofFactKind,
    ) -> Option<Vec<typed_trees::expression::ExpressionHandle>> {
        let mut expressions = Vec::new();
        for (_, checked) in facts.proof.contract_facts.iter().filter(|(_, checked)| {
            matches!(checked.owner, ContractProofFactOwner::Machine { machine_symbol } if machine_symbol == machine)
                || matches!(checked.owner, ContractProofFactOwner::MachineState { machine_symbol, state_symbol } if machine_symbol == machine && state_symbol == state)
        }) {
            if checked.kind != kind {
                continue;
            }
            let expression = program.proof_facts.get(checked.fact).as_expression()?;
            expressions.push(*expression);
        }
        Some(expressions)
    }

    pub(crate) fn direct_boolean_field_requirement(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        root_parameter: &StateParameter,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<CheckedUnitNominalAffineCleanupRequirementPlan> {
        use typed_trees::expression::{BinaryOperator, UnaryOperator};

        let (field_expression, expected) = match program.expression_table.expression(expression) {
            ExpressionNode::Member(_) => (expression, true),
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                if !matches!(
                    program.expression_table.expression(unary.operand),
                    ExpressionNode::Member(_)
                ) {
                    return None;
                }
                (unary.operand, false)
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) =>
            {
                let (field, literal) = match (
                    program.expression_table.expression(binary.left),
                    program.expression_table.expression(binary.right),
                ) {
                    (ExpressionNode::Boolean(literal), ExpressionNode::Member(_)) => {
                        (binary.right, *literal)
                    }
                    (ExpressionNode::Member(_), ExpressionNode::Boolean(literal)) => {
                        (binary.left, *literal)
                    }
                    _ => return None,
                };
                (
                    field,
                    if binary.operator == BinaryOperator::Equal {
                        literal
                    } else {
                        !literal
                    },
                )
            }
            _ => return None,
        };
        let place = place_at(program, state, 0, field_expression)?;
        let [facts::PlaceSegment::Field { symbol }] = place.segments.as_slice() else {
            return None;
        };
        if place.root != facts::PlaceRoot::Symbol(root_parameter.symbol)
            || !program.data_definitions().iter().any(|data| {
                program.data_members(data).iter().any(|member| {
                    matches!(member, DataMember::Field(field)
                        if field.symbol == *symbol
                            && program.primitive_type_reference(field.type_reference)
                                == Some(PrimitiveType::Bool))
                })
            })
        {
            return None;
        }
        Some(CheckedUnitNominalAffineCleanupRequirementPlan {
            field_identity: terminal_field_identity(program, *symbol)?,
            expected,
        })
    }
}
pub(super) mod normal_edges {

    /*
        Selected normal-edge custody, expressed in the ordinary state value namespace.
        The shared ownership selection supplies order and exact outgoing coordinates;
        hook requirements remain obligations at that edge, never new unconditional facts.
    */
    mod exits {

        /*
            Exact normal completion coordinates retained by checked control flow.

            Machine-boundary completions and same-machine edges have distinct flow
            channels. Cleanup and result custody must consume the same selected exit;
            the end of the lexical body is not a terminal call's completion occurrence.
        */
        use checked_trees::{CheckFacts, FlowClaimJoinExit};
        use symbols::SymbolHandle;

        pub(super) struct StateExit {
            pub(super) exit: FlowClaimJoinExit,
            completed_storage: arena::Handle<checked_trees::FlowStorageSnapshot>,
            pub(super) returned_value: arena::Handle<checked_trees::FlowOperandFact>,
        }

        pub(in crate::execution::terminal_unit) fn state_exits(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            machine: SymbolHandle,
            state: &typed_trees::state::State,
        ) -> Result<Vec<FlowClaimJoinExit>, &'static str> {
            let mut exits = Vec::new();
            for row in state_exit_rows(program, facts, machine, state)? {
                if !exits.contains(&row.exit) {
                    exits.push(row.exit);
                }
            }
            Ok(exits)
        }

        /*
            Named transitions return no value, but cleanup still needs the storage
            left by their operands. Keep every storage snapshot for the exact selected edge; an
            unavailable alternative cannot borrow another alternative's snapshot.
        */
        pub(in crate::execution::terminal_unit) fn state_exit_storage(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            machine: SymbolHandle,
            state: &typed_trees::state::State,
            exit: FlowClaimJoinExit,
        ) -> Result<Vec<arena::Handle<checked_trees::FlowStorageSnapshot>>, &'static str> {
            let mut snapshots = Vec::new();
            for row in state_exit_rows(program, facts, machine, state)? {
                if row.exit != exit {
                    continue;
                }
                if !facts
                    .flow
                    .control
                    .storage_snapshots
                    .is_valid(row.completed_storage)
                {
                    return Err(
                        "completion cleanup: the selected exit has no completed storage snapshot",
                    );
                }
                if !snapshots.contains(&row.completed_storage) {
                    snapshots.push(row.completed_storage);
                }
            }
            if snapshots.is_empty() {
                return Err(
                    "completion cleanup: the selected exit has no retained flow occurrence",
                );
            }
            Ok(snapshots)
        }

        pub(super) fn state_exit_rows(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            machine: SymbolHandle,
            state: &typed_trees::state::State,
        ) -> Result<Vec<StateExit>, &'static str> {
            let flow_state = facts
                .flow
                .control
                .state(machine, state.symbol)
                .ok_or("normal cleanup state has no retained control flow")?;
            let mut rows = Vec::new();
            for outgoing in facts
                .flow
                .control
                .outgoing
                .span_or_empty(flow_state.outgoing)
            {
                let kind = if !outgoing.transition_target.is_valid() {
                    checked_trees::FlowClaimJoinExitKind::FinalExpression
                } else {
                    let transition = program
                        .statement_table
                        .statement_at(state, outgoing.statement_index)
                        .ok_or("normal cleanup edge has no source statement")?
                        .as_transition()
                        .ok_or("normal cleanup edge is not a direct transition")?;
                    if transition.target == outgoing.transition_target {
                        checked_trees::FlowClaimJoinExitKind::TransitionTarget
                    } else if transition.continuation == outgoing.transition_target {
                        checked_trees::FlowClaimJoinExitKind::TransitionContinuation
                    } else {
                        return Err("normal cleanup edge does not select the source transition");
                    }
                };
                let exit = checked_trees::FlowClaimJoinExit {
                    state_symbol: state.symbol,
                    statement_index: outgoing.statement_index,
                    kind,
                };
                rows.push(StateExit {
                    exit,
                    completed_storage: outgoing.completed_storage,
                    returned_value: outgoing.returned_value,
                });
            }
            for outgoing in facts.flow.control.exits.span_or_empty(flow_state.exits) {
                if outgoing.state_symbol != state.symbol {
                    return Err("normal cleanup exit belongs to another state");
                }
                let kind = if !outgoing.transition_target.is_valid() {
                    checked_trees::FlowClaimJoinExitKind::FinalExpression
                } else {
                    let transition = program
                        .statement_table
                        .statement_at(state, outgoing.statement_index)
                        .ok_or("normal cleanup edge has no source statement")?
                        .as_transition()
                        .ok_or("normal cleanup edge is not a direct transition")?;
                    let selected = if outgoing.is_continuation {
                        transition.continuation
                    } else {
                        transition.target
                    };
                    if selected != outgoing.transition_target {
                        return Err("normal cleanup edge does not select the source transition");
                    }
                    if outgoing.is_continuation {
                        checked_trees::FlowClaimJoinExitKind::TransitionContinuation
                    } else {
                        checked_trees::FlowClaimJoinExitKind::TransitionTarget
                    }
                };
                let exit = checked_trees::FlowClaimJoinExit {
                    state_symbol: state.symbol,
                    statement_index: outgoing.statement_index,
                    kind,
                };
                rows.push(StateExit {
                    exit,
                    completed_storage: outgoing.completed_storage,
                    returned_value: outgoing.returned_value,
                });
            }
            Ok(rows)
        }
    }
    pub(in crate::execution::terminal_unit) use exits::{state_exit_storage, state_exits};

    use crate::execution::terminal_unit::types::ShapeCollector;
    use checked_trees::{
        CheckFacts, CheckedComposedUnitControlStatePlan, CheckedUnitCallableEntry,
        CheckedUnitStructuralArgumentSourcePlan,
    };
    use facts::PlaceRoot;
    use language_semantics::PermissionEventSource;
    use symbols::SymbolHandle;

    /*
        The producing layer joins source invocation reachability with its completed
        physical control. Unknown source outcomes remain potentially normal.
    */
    pub(in crate::execution::terminal_unit) fn completed_body_outcome(
        program: &dyn validation::CheckingInput,
        state: &typed_trees::state::State,
        physical_continuation: bool,
    ) -> checked_trees::CheckedNormalCompletion {
        if physical_continuation
            && validation::normal_completion_possible(program, state) != Some(false)
        {
            checked_trees::CheckedNormalCompletion::Possible
        } else {
            checked_trees::CheckedNormalCompletion::NoNormal
        }
    }

    /* Mutable construction scratch; only the shared cleanup arena is published. */
    struct NormalExitConstruction {
        partitions: Vec<arena::Handle<checked_trees::CheckedCleanupPartitionPlan>>,
        exit: checked_trees::FlowClaimJoinExit,
        selected_regions: Vec<checked_trees::CheckedUnitStructuralArgumentPlan>,
        actions: Vec<checked_trees::CheckedNormalCleanupAction>,
        result_completion: arena::Handle<checked_trees::CheckedNormalResultCompletion>,
    }

    fn retain_normal_exits(
        shapes: &mut ShapeCollector<'_>,
        rows: Vec<NormalExitConstruction>,
        completion: checked_trees::CheckedNormalCompletion,
    ) -> checked_trees::CheckedNormalExitCleanups {
        let owner = &mut shapes.cleanup_control;
        let retained = rows
            .into_iter()
            .map(|row| checked_trees::CheckedNormalExitCleanupPlan {
                exit: row.exit,
                selected_regions: owner.normal_exit_regions.insert_many(row.selected_regions),
                actions: owner.actions.insert_many(row.actions),
                partitions: super::actions::partition_refs(owner, row.partitions),
                result_completion: row.result_completion,
            })
            .collect::<Vec<_>>();
        checked_trees::CheckedNormalExitCleanups {
            completion,
            rows: owner.normal_exit_rows.insert_many(retained),
        }
    }

    pub(in crate::execution::terminal_unit) fn state_cleanups(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
        state_plan: &CheckedComposedUnitControlStatePlan,
    ) -> Result<checked_trees::CheckedNormalExitCleanups, &'static str> {
        let (machine, state) = program
            .machines()
            .iter()
            .find_map(|machine| {
                program
                    .state_in_machine(machine, state_plan.state)
                    .map(|state| (machine, state))
            })
            .ok_or("normal cleanup state has no owning machine")?;
        let completion = state_plan.normal_exit_cleanups.completion;
        let exits = state_exits(program, facts, machine.symbol, state)?;
        if completion == checked_trees::CheckedNormalCompletion::NoNormal && exits.is_empty() {
            return Ok(checked_trees::CheckedNormalExitCleanups {
                completion,
                ..Default::default()
            });
        }
        if state_plan.selected_exit.is_some()
            && matches!(
                state_plan.terminator,
                checked_trees::CheckedComposedUnitControlTerminatorPlan::Crash { .. }
                    | checked_trees::CheckedComposedUnitControlTerminatorPlan::InvokeNoNormal { .. }
                    | checked_trees::CheckedComposedUnitControlTerminatorPlan::EvaluateNoNormal
            )
        {
            return Ok(checked_trees::CheckedNormalExitCleanups {
                completion: checked_trees::CheckedNormalCompletion::NoNormal,
                ..Default::default()
            });
        }
        let rows = cleanup_rows(
            program,
            facts,
            targets,
            shapes,
            machine.symbol,
            state,
            exits,
            state_plan.selected_exit,
            matches!(
                state_plan.terminator,
                checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                    completion: checked_trees::CheckedScalarReturnPlan::Exits(_)
                }
            ),
            |root| owner_source(program, facts, state, state_plan, root).map(Some),
        )?;
        Ok(retain_normal_exits(shapes, rows, completion))
    }

    /*
        Sequence completion disposes current named storage through the same checked
        exits as state graphs. Anonymous evaluation results keep their own producer
        disposition; named locals must never revive an overwritten birth result.
    */
    pub(in crate::execution::terminal_unit) fn sequence_cleanups(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
        plan: &checked_trees::CheckedUnitEffectMachinePlan,
    ) -> Option<checked_trees::CheckedNormalExitCleanups> {
        let machine = program.machine_by_symbol(plan.machine)?;
        let state = program.state_in_machine(machine, plan.state)?;
        let completion = completed_body_outcome(
            program,
            state,
            plan.operations
                .iter()
                .all(|operation| operation.has_normal_continuation()),
        );
        let exits = state_exits(program, facts, plan.machine, state).ok()?;
        if completion == checked_trees::CheckedNormalCompletion::NoNormal && exits.is_empty() {
            return Some(checked_trees::CheckedNormalExitCleanups {
                completion,
                ..Default::default()
            });
        }
        let rows = cleanup_rows(
            program,
            facts,
            targets,
            shapes,
            plan.machine,
            state,
            exits,
            None,
            plan.scalar_control.is_some(),
            |root| {
                let operations = &plan.operations;
                if root.as_expression().is_some() {
                    return expression_source(facts, state.symbol, operations, root).map(Some);
                }
                let position = program
                    .state_parameters(state)
                    .iter()
                    .position(|parameter| root == PlaceRoot::Symbol(parameter.symbol));
                let Some(position) = position else {
                    return Some(super::super::local_storage::source(
                        program,
                        state,
                        operations,
                        root.as_symbol()?,
                    ));
                };
                let Some(index) = plan
                    .structural_parameters
                    .iter()
                    .position(|parameter| parameter.position as usize == position)
                else {
                    return Some(None);
                };
                Some(Some(CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(index).ok()?,
                }))
            },
        )
        .ok()?;
        Some(retain_normal_exits(shapes, rows, completion))
    }

    /// The normal-result producer retains selected arm structure. Only actual
    /// result alternatives reach this tree; operand calls keep ordinary completion.
    fn cleanup_rows(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
        machine: SymbolHandle,
        state: &typed_trees::state::State,
        source_exits: Vec<checked_trees::FlowClaimJoinExit>,
        selected_exit: Option<checked_trees::FlowClaimJoinExit>,
        scalar_return: bool,
        mut source_for_owner: impl FnMut(
            PlaceRoot,
        )
            -> Option<Option<CheckedUnitStructuralArgumentSourcePlan>>,
    ) -> Result<Vec<NormalExitConstruction>, &'static str> {
        if !scalar_return {
            return cleanup_rows_at(
                program,
                facts,
                targets,
                shapes,
                machine,
                state,
                &source_exits,
                selected_exit,
                None,
                source_for_owner,
            );
        }
        fn key(exit: checked_trees::FlowClaimJoinExit) -> Result<(usize, u8), &'static str> {
            use checked_trees::FlowClaimJoinExitKind::*;
            Ok((
                exit.statement_index,
                match exit.kind {
                    FinalExpression => 0,
                    TransitionTarget => 1,
                    TransitionContinuation => 2,
                    _ => return Err("normal result has no authored outgoing coordinate"),
                },
            ))
        }
        let mut values = std::collections::BTreeMap::<_, Vec<_>>::new();
        for row in exits::state_exit_rows(program, facts, machine, state)? {
            values
                .entry(key(row.exit)?)
                .or_default()
                .push(row.returned_value);
        }
        let cases = crate::flow::CleanupCaseSelections::new(facts, machine, state.symbol)
            .ok_or("normal result has no complete case selection roster")?;
        let mut rows = Vec::new();
        for exit in source_exits.iter().copied() {
            if selected_exit.is_some_and(|selected| selected != exit) {
                continue;
            }
            let mut captures = values
                .remove(&key(exit)?)
                .ok_or("normal result has no completed capture")?;
            captures.sort_unstable_by_key(|value| (value.arena_index(), value.generation()));
            captures.dedup();
            let completed = crate::flow::cleanup_result_selections(
                program,
                facts,
                exit,
                captures,
                cases.clone(),
            )
            .ok_or("normal result has no complete selected capture graph")?;
            let role = if exit.kind == checked_trees::FlowClaimJoinExitKind::TransitionContinuation
            {
                checked_trees::CheckedScalarExpressionRole::ContinuationReturn
            } else {
                checked_trees::CheckedScalarExpressionRole::Return
            };
            let mut roots = facts.values.scalar_computations.roots_for_statement_role(
                state.symbol,
                u32::try_from(exit.statement_index)
                    .map_err(|_| "normal result statement exceeds u32")?,
                role,
            );
            let root = roots.next();
            if completed.nodes[0].branches.is_empty() || root.is_none() {
                rows.extend(cleanup_rows_at(
                    program,
                    facts,
                    targets,
                    shapes,
                    machine,
                    state,
                    &source_exits,
                    Some(exit),
                    None,
                    &mut source_for_owner,
                )?);
                continue;
            }
            let root = root.ok_or("normal result has no owning computation")?;
            if roots.next().is_some() || root.machine != machine {
                return Err("normal result has no unique owning computation");
            }
            let mut regions = None;
            let result_completion = normal_result_tree(
                program,
                facts,
                targets,
                shapes,
                machine,
                state,
                &source_exits,
                exit,
                &completed,
                0,
                root.root,
                &mut source_for_owner,
                &mut regions,
                &mut std::collections::BTreeSet::new(),
            )?;
            // These leaves add no action or owned-region specialization. Keep the
            // existing complete continuation owner, including reference endings.
            let result_completion = if regions.as_ref().is_some_and(Vec::is_empty)
                && shapes
                    .cleanup_control
                    .result_actions(result_completion)
                    .next()
                    .is_none()
            {
                arena::Handle::invalid()
            } else {
                result_completion
            };
            rows.push(NormalExitConstruction {
                partitions: Vec::new(),
                exit,
                // Every returning value leaf installs this roster. Its absence
                // means every retained leaf was declaration-proven NoNormal.
                selected_regions: regions.unwrap_or_default(),
                actions: Vec::new(),
                result_completion,
            });
        }
        Ok(rows)
    }

    /// Publish the exact checked result structure with each leaf transaction.
    /// Correspondence and complete coverage are decided here, before lowering.
    #[allow(clippy::too_many_arguments)]
    fn normal_result_tree(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
        machine: SymbolHandle,
        state: &typed_trees::state::State,
        source_exits: &[checked_trees::FlowClaimJoinExit],
        exit: checked_trees::FlowClaimJoinExit,
        completed: &crate::flow::CompletedCleanupResults<'_>,
        cut: usize,
        computation: checked_trees::CheckedScalarComputationHandle,
        source_for_owner: &mut impl FnMut(
            PlaceRoot,
        )
            -> Option<Option<CheckedUnitStructuralArgumentSourcePlan>>,
        regions: &mut Option<Vec<checked_trees::CheckedUnitStructuralArgumentPlan>>,
        active: &mut std::collections::BTreeSet<(u32, u32)>,
    ) -> Result<arena::Handle<checked_trees::CheckedNormalResultCompletion>, &'static str> {
        let plans = &facts.values.scalar_computations;
        let key = (computation.arena_index(), computation.generation());
        if !plans.nodes.is_valid(computation) || !active.insert(key) {
            return Err("normal result has a stale or cyclic computation");
        }
        let source = completed
            .nodes
            .get(cut)
            .ok_or("normal result lost its completed cut")?;
        let node = plans.nodes.get(computation);
        let result =
            if validation::expression_normal_completion_possible(program, source.expression)
                == Some(false)
            {
                checked_trees::CheckedNormalResultCompletion::NoNormal { computation }
            } else if let checked_trees::CheckedScalarComputationKind::Dispatch {
                source_expression,
                arms,
                ..
            } = &node.kind
                && !source.branches.is_empty()
            {
                if *source_expression != source.expression {
                    return Err("normal result selection changed its source expression");
                }
                let arm_span = *arms;
                let arms = plans
                    .dispatch_arms
                    .span(arm_span)
                    .filter(|arms| !arms.is_empty())
                    .ok_or("normal result selection lost its arm roster")?;
                let mut retained = Vec::with_capacity(arms.len());
                let mut seen = std::collections::BTreeSet::new();
                for (arm_handle, arm) in arm_span.handles().zip(arms) {
                    let key = (arm.source_arm.arena_index(), arm.source_arm.generation());
                    if !seen.insert(key) {
                        return Err("normal result selection repeats a source arm");
                    }
                    let child = *source
                        .branches
                        .get(&key)
                        .ok_or("normal result arm has no completed source capture")?;
                    let result = normal_result_tree(
                        program,
                        facts,
                        targets,
                        shapes,
                        machine,
                        state,
                        source_exits,
                        exit,
                        completed,
                        child,
                        arm.value,
                        source_for_owner,
                        regions,
                        active,
                    )?;
                    retained.push(checked_trees::CheckedNormalResultArm {
                        arm: arm_handle,
                        result,
                    });
                }
                // Flow records a completed result for every arm it walked,
                // including one the checker ruled out. The roster may leave
                // out exactly the arms the dispatch planner itself omits; any
                // other completed arm without a planned result is refused.
                if seen.len() != source.branches.len() {
                    let typed_trees::expression::ExpressionNode::Match(dispatch) =
                        program.expression_table.expression(source.expression)
                    else {
                        return Err("normal result selection omits a completed source arm");
                    };
                    let authored = program.expression_table.match_arms(dispatch.arms);
                    let mut accounted = seen.len();
                    for (source_arm, arm) in dispatch.arms.handles().zip(authored) {
                        let key = (source_arm.arena_index(), source_arm.generation());
                        if seen.contains(&key) || !source.branches.contains_key(&key) {
                            continue;
                        }
                        if !crate::execution::scalar_computations::structural_dispatch_omits_arm(
                            program,
                            machine,
                            state.symbol,
                            dispatch,
                            arm,
                        ) {
                            return Err("normal result selection omits a completed source arm");
                        }
                        accounted += 1;
                    }
                    if accounted != source.branches.len() {
                        return Err("normal result selection omits a completed source arm");
                    }
                }
                checked_trees::CheckedNormalResultCompletion::Dispatch {
                    computation,
                    arms: shapes
                        .cleanup_control
                        .normal_result_arms
                        .insert_many(retained),
                }
            } else {
                let mut rows = cleanup_rows_at(
                    program,
                    facts,
                    targets,
                    shapes,
                    machine,
                    state,
                    source_exits,
                    Some(exit),
                    Some(&source.result),
                    &mut *source_for_owner,
                )?;
                if rows.len() != 1 {
                    return Err("normal result leaf has no unique transaction");
                }
                let row = rows
                    .pop()
                    .ok_or("normal result leaf has no unique transaction")?;
                if let Some(expected) = regions.as_ref() {
                    if expected != &row.selected_regions {
                        return Err("normal result leaves disagree on their selected owned homes");
                    }
                } else {
                    *regions = Some(row.selected_regions);
                }
                checked_trees::CheckedNormalResultCompletion::Value {
                    computation,
                    actions: shapes.cleanup_control.actions.insert_many(row.actions),
                    partitions: super::actions::partition_refs(
                        &mut shapes.cleanup_control,
                        row.partitions,
                    ),
                }
            };
        active.remove(&key);
        Ok(shapes
            .cleanup_control
            .normal_result_completions
            .append(result))
    }

    fn cleanup_rows_at(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
        machine: SymbolHandle,
        state: &typed_trees::state::State,
        source_exits: &[checked_trees::FlowClaimJoinExit],
        selected_exit: Option<checked_trees::FlowClaimJoinExit>,
        completed_result: Option<&crate::flow::CompletedCleanupResult<'_>>,
        mut source_for_owner: impl FnMut(
            PlaceRoot,
        )
            -> Option<Option<CheckedUnitStructuralArgumentSourcePlan>>,
    ) -> Result<Vec<NormalExitConstruction>, &'static str> {
        let flow_state = facts
            .flow
            .control
            .state(machine, state.symbol)
            .ok_or("normal cleanup state has no retained control flow")?;
        if selected_exit.is_some_and(|selected| !source_exits.contains(&selected)) {
            return Err("selected normal cleanup has no exact source exit");
        }
        let mut rows = source_exits
            .iter()
            .copied()
            .filter(|exit| selected_exit.is_none_or(|selected| selected == *exit))
            .map(|exit| NormalExitConstruction {
                partitions: Vec::new(),
                exit,
                selected_regions: Vec::new(),
                actions: Vec::new(),
                result_completion: Default::default(),
            })
            .collect::<Vec<_>>();
        let mut retirements = Vec::new();
        let mut establishment_ordered = Vec::new();
        for disposition_handle in facts.contextual_cleanup.state_disposition_handles(
            super::owning_cleanup_machine(program, state.symbol),
            state.symbol,
        ) {
            let disposition = facts
                .contextual_cleanup
                .dispositions
                .get(disposition_handle);
            if disposition.source != PermissionEventSource::StateExit {
                continue;
            }
            if disposition.exit.state_symbol != state.symbol {
                return Err("normal cleanup disposition belongs to another state");
            }
            if selected_exit.is_some_and(|selected| disposition.exit != selected) {
                if !source_exits.contains(&disposition.exit) {
                    return Err("normal cleanup disposition has no matching exit");
                }
                continue;
            }
            let captured = if disposition.selection.is_valid() {
                let receipt = facts
                    .flow
                    .ownership
                    .owned_selections
                    .get(disposition.selection);
                if receipt.state != state.symbol || receipt.machine != machine {
                    return Err("normal cleanup receipt belongs to another state or machine");
                }
                let mut sources = facts
                    .flow
                    .ownership
                    .selection_sources
                    .span_or_empty(receipt.sources)
                    .iter()
                    .enumerate()
                    .filter(|(_, source)| Some(source.symbol) == disposition.root.as_symbol());
                let (ordinal, _) = sources
                    .next()
                    .ok_or("normal cleanup receipt has no source for its root")?;
                if sources.next().is_some() {
                    return Err("normal cleanup receipt has multiple sources for its root");
                }
                Some(CheckedUnitStructuralArgumentSourcePlan::CapturedStorage {
                    selection: disposition.selection,
                    source_ordinal: u32::try_from(ordinal)
                        .map_err(|_| "normal cleanup source ordinal exceeds u32")?,
                })
            } else {
                None
            };
            let source = match captured {
                Some(source) => Some(source),
                None => source_for_owner(disposition.root)
                    .ok_or("normal cleanup owner has no retained structural storage")?,
            };
            let Some(source) = source else {
                continue;
            };
            if disposition.establishment_ordered
                && !establishment_ordered.contains(&(disposition.exit, disposition.root))
            {
                establishment_ordered.push((disposition.exit, disposition.root));
            }
            if !retirements.iter().any(
                |(exit, retirement, _): &(_, checked_trees::CheckedNormalCleanupAction, _)| {
                    *exit == disposition.exit && retirement.source() == &source
                },
            ) {
                retirements.push((
                    disposition.exit,
                    super::actions::home_retirement(program, shapes, disposition, source.clone())
                        .ok_or("normal cleanup cannot realize the retiring home")?,
                    disposition.root,
                ));
            }
            let actions = super::actions::disposition_actions_at(
                program,
                facts,
                targets,
                shapes,
                disposition_handle,
                source.clone(),
                None,
                completed_result,
            )?;
            if let Some(row) = rows.iter_mut().find(|row| row.exit == disposition.exit) {
                if facts
                    .contextual_cleanup
                    .is_selected_disposition(disposition_handle)
                {
                    let region = super::actions::selected_region(
                        program,
                        facts,
                        shapes,
                        disposition_handle,
                        source.clone(),
                        None,
                    )
                    .ok_or("normal cleanup cannot realize its selected region")?;
                    crate::shared::collections::push_unique(&mut row.selected_regions, region);
                }
                row.partitions.push(actions.partition);
                row.actions.extend(actions.actions);
            } else {
                if ::validation::trace_flags::check_sites() {
                    eprintln!(
                        "CHECKSITE normal-cleanup-unmatched machine={machine:?} state={:?} disposition={disposition:?} exits={:?} outgoing={:?}",
                        state.symbol,
                        rows.iter().take(8).map(|row| row.exit).collect::<Vec<_>>(),
                        facts
                            .flow
                            .control
                            .outgoing
                            .span_or_empty(flow_state.outgoing)
                            .iter()
                            .take(8)
                            .collect::<Vec<_>>(),
                    );
                }
                return Err("normal cleanup disposition has no matching exit");
            }
        }
        for (exit, retirement, root) in retirements {
            let row = rows
                .iter_mut()
                .find(|row| row.exit == exit)
                .ok_or("normal cleanup retirement has no matching exit")?;
            if !establishment_ordered.contains(&(exit, root))
                && row
                    .selected_regions
                    .iter()
                    .any(|region| &region.source == retirement.source() && !region.path.is_empty())
            {
                super::order::merge(
                    program,
                    state.symbol,
                    exit.statement_index,
                    root,
                    &mut row.actions,
                    retirement.source(),
                )
                .ok_or("normal cleanup cannot order the selected region actions")?;
            }
            super::actions::finish_home(&mut row.actions, retirement);
        }
        Ok(rows)
    }

    fn owner_source(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &typed_trees::state::State,
        plan: &CheckedComposedUnitControlStatePlan,
        root: PlaceRoot,
    ) -> Option<CheckedUnitStructuralArgumentSourcePlan> {
        let operations = &plan.operations;
        if root.as_expression().is_some() {
            return expression_source(facts, state.symbol, operations, root);
        }
        let symbol = root.as_symbol()?;
        if let Some(position) = typed_trees::signature::state_parameter_position(
            program.state_parameters(state),
            symbol,
        ) {
            let index = plan
                .structural_parameters
                .iter()
                .position(|parameter| parameter.position as usize == position)?;
            return Some(CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(index).ok()?,
            });
        }
        super::super::local_storage::source(program, state, operations, symbol)
    }

    pub(in crate::execution::terminal_unit) fn expression_source(
        facts: &CheckFacts,
        state: SymbolHandle,
        operations: &[checked_trees::CheckedUnitEffectOperationPlan],
        root: PlaceRoot,
    ) -> Option<CheckedUnitStructuralArgumentSourcePlan> {
        let mut matching = operations.iter().filter_map(|operation| {
            let result = operation.structural_result_binding()?;
            (producer_root(facts, state, operation) == Some(root)).then_some(result)
        });
        let Some(result) = matching.next() else {
            let expression = root.as_expression()?;
            return facts
                .values
                .structural_values
                .nodes
                .iter()
                .any(|(_, value)| value.state == state && value.expression == expression)
                .then_some(
                    CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        value_dependency: None,
                        expression,
                        logical_root_type_identity: None,
                    },
                );
        };
        if matching.next().is_some() {
            return None;
        }
        Some(CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        })
    }

    /*
        Join a retained result to the source occurrence that established it. The
        call coordinate is authoritative; equal expressions or return types cannot
        identify a second evaluation as the owner of the first one's cleanup.
    */
    pub(in crate::execution::terminal_unit) fn producer_root(
        facts: &CheckFacts,
        state: SymbolHandle,
        operation: &checked_trees::CheckedUnitEffectOperationPlan,
    ) -> Option<PlaceRoot> {
        use checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitLocalBindingPlan};
        let result = operation.structural_result_binding()?;
        if result.local.is_named() {
            return Some(PlaceRoot::Symbol(result.local.symbol));
        }
        let expression = match operation {
            CheckedUnitEffectOperationPlan::EstablishLocal(
                CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
            ) => facts.values.structural_values.nodes.get(*value).expression,
            CheckedUnitEffectOperationPlan::Call(call) => {
                let flow = facts.flow.control.state_by_symbol(state)?;
                let mut calls = facts
                    .flow
                    .control
                    .calls
                    .span(flow.calls)?
                    .iter()
                    .filter(|row| {
                        row.statement_index == call.coordinate.statement_index as usize
                            && row.call_ordinal == call.coordinate.call_ordinal as usize
                    });
                let expression = calls.next()?.authored_expression;
                if calls.next().is_some() {
                    return None;
                }
                expression
            }
            _ => return None,
        };
        expression
            .is_valid()
            .then_some(PlaceRoot::Expression(expression))
    }
}
mod order {
    /*
        Merge independently selected partitions in the declared cleanup order.
        Only retained action paths are visited. Their exact typed coordinates join
        the same canonical ordering authority used to check hook prerequisites.
    */
    use super::actions::checked_path_segments;
    use checked_trees::{CheckedNormalCleanupAction, CheckedUnitStructuralArgumentSourcePlan};

    pub(super) fn merge(
        program: &dyn validation::CheckingInput,
        state: symbols::SymbolHandle,
        statement: usize,
        root: facts::PlaceRoot,
        actions: &mut [CheckedNormalCleanupAction],
        source: &CheckedUnitStructuralArgumentSourcePlan,
    ) -> Option<()> {
        let reference = validation::canonical_place_type_reference(
            program,
            state,
            statement,
            &validation::CanonicalPlace {
                root,
                segments: Vec::new(),
            },
        )?;
        let positions = actions
            .iter()
            .enumerate()
            .filter_map(|(position, action)| (action.source() == source).then_some(position))
            .collect::<Vec<_>>();
        let mut selected = positions
            .iter()
            .map(|position| {
                let action = actions[*position].clone();
                let path =
                    checked_path_segments(program, state, statement, reference, action.path())?;
                let key = crate::flow::cleanup_path_order(program, &path)?;
                Some((key, action))
            })
            .collect::<Option<Vec<_>>>()?;
        selected.sort_by(|left, right| left.0.cmp(&right.0));
        for (position, (_, action)) in positions.into_iter().zip(selected) {
            actions[position] = action;
        }
        Some(())
    }
}
mod partial_affine_cleanup {
    /*
        Residual reconstruction for the partial-affine return edge: the complement
        of one partially moved owned parameter, rebuilt from the checked type plan.
        The machines that owe it are published by `build_checked_unit_effect_plans`
        from the same ordinary body build as every other candidate.
    */
    use crate::execution::terminal_unit::cleanup::residuals;
    use crate::execution::terminal_unit::{
        CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
        CheckedUnitStructuralPathSegment,
    };
    use typed_trees::type_identity::TypeIdentityId;

    pub(crate) fn partial_affine_residuals(
        types: &crate::execution::terminal_unit::types::TypeCatalog,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        root_type: TypeIdentityId,
        moved_paths: &[(Vec<CheckedUnitStructuralPathSegment>, TypeIdentityId)],
    ) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
        let borrowed = moved_paths
            .iter()
            .map(|(path, moved_type)| (path.as_slice(), *moved_type))
            .collect::<Vec<_>>();
        residuals::reconstruct(types, source, root_type, &borrowed, usize::MAX)
    }
}
mod replacements {
    /*
        Select an assignment's old-value death before its establishing store.

        The ownership producer has already chosen the exact live subtrees. Resolve
        their ordinary state storage once and reuse the same action selection as
        state exits and temporary continuations. A borrowed replacement displaces
        the old value into a result; that changes its executable storage namespace,
        not which original death occurrence supplies its contracts or action order.
        A stored reference instead has a checked loan-carrier death. Its intrinsic
        descriptor disposal uses this same boundary without disposing the referent
        or manufacturing an ownership death for borrowed storage.
    */
    use crate::execution::terminal_unit::types::ShapeCollector;
    use crate::flow::state_statements;
    use crate::published::places::place_at;
    use checked_trees::{
        CheckFacts, CheckedNormalCleanupAction, CheckedUnitCallableEntry,
        CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
        CheckedUnitStructuralParameterPlan,
    };
    use facts::PlaceRoot;
    use language_semantics::PermissionEventSource;
    use symbols::SymbolHandle;

    pub(super) fn actions(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
        state: SymbolHandle,
        parameters: &[CheckedUnitStructuralParameterPlan],
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<super::temporary_actions::SelectedActions, &'static str> {
        (|| {
            let CheckedUnitEffectOperationPlan::ContinuationCleanup {
                boundary:
                    checked_trees::CheckedUnitCleanupBoundary::Replacement {
                        statement_index,
                        destination,
                        old_value,
                        ..
                    },
                ..
            } = operation
            else {
                return Err("continuation cleanup boundary is not a replacement");
            };
            let machine = super::owning_cleanup_machine(program, state);
            let source = &destination.source;
            let root = match *source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                    PlaceRoot::Symbol(symbol)
                }
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                    let parameter = parameters
                        .get(parameter_index as usize)
                        .ok_or("replacement parameter index exceeds its catalog")?;
                    let declaration = program
                        .state_by_symbol(state)
                        .ok_or("replacement state has no declaration")?;
                    PlaceRoot::Symbol(
                        program
                            .state_parameters(declaration)
                            .get(parameter.position as usize)
                            .ok_or("replacement parameter position exceeds its declaration")?
                            .symbol,
                    )
                }
                _ => return Err("replacement destination root is not a local or parameter"),
            };
            let displaced = old_value.source != *source;
            let replaced_place = match program.state_by_symbol(state).and_then(|declaration| {
                crate::flow::state_statements(program, declaration.statement_nodes)
                    .get(*statement_index as usize)
            }) {
                Some(typed_trees::statement::StatementNode::Assignment(assignment)) => {
                    place_at(program, state, *statement_index as usize, assignment.target)
                }
                _ => None,
            };
            let stored_place = replaced_place.as_ref().and_then(|place| {
                let declaration = program.state_by_symbol(state)?;
                let (root, segments) = super::super::borrowed_windows::resolve_storage_place(
                    program,
                    declaration,
                    state_statements(program, declaration.statement_nodes),
                    *statement_index as usize,
                    place.root.as_symbol()?,
                    &place.segments,
                );
                Some(validation::CanonicalPlace { root, segments })
            });
            let mut actions = Vec::new();
            let mut partitions = Vec::new();
            let mut selected_regions = Vec::new();
            let mut selected_disposition = false;
            let captured_path = destination
                .path
                .iter()
                .any(|segment| {
                    matches!(
                        segment,
                        checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                    )
                })
                .then(|| {
                    let declaration = program.state_by_symbol(state)?;
                    let assignment = program
                        .statement_table
                        .statement_at(declaration, *statement_index as usize)?
                        .as_assignment()?;
                    let owner = replaced_place.as_ref().filter(|place| place.root == root)?;
                    super::super::borrowed_windows::captured_assignment_path(
                        program,
                        facts,
                        program.machine_by_symbol(machine)?,
                        declaration,
                        *statement_index,
                        assignment.target,
                        owner,
                    )
                })
                .flatten();
            let retirement_path = captured_path.as_deref().unwrap_or(&destination.path);
            let transferred = replaced_place.as_ref().is_some_and(|place| {
                facts.contextual_cleanup.replaces_transferred_storage(
                    machine,
                    state,
                    PermissionEventSource::Statement {
                        statement_index: *statement_index as usize,
                    },
                    place.root,
                    &place.segments,
                )
            });
            let carrier_death = (!transferred)
                .then(|| {
                    validation::reference_result_custody::replaced_reference_carrier(
                        program,
                        facts,
                        machine,
                        program.state_by_symbol(state)?,
                        root,
                        *statement_index,
                        retirement_path,
                    )
                })
                .flatten();
            let carrier_discard = if let Some(carrier) = carrier_death {
                let declaration = shapes
                    .types
                    .get(&destination.type_identity)
                    .ok_or("replacement carrier has no shape declaration")?;
                let reference = validation::canonical_place_type_reference(
                    program,
                    state,
                    *statement_index as usize,
                    replaced_place
                        .as_ref()
                        .ok_or("replacement carrier names no replaced place")?,
                )
                .ok_or("replacement carrier place has no type reference")?;
                let shell = validation::reference_shell(program, reference)
                    .ok_or("replacement carrier reference has no shell")?;
                let source_matches = match declaration.shape {
                    checked_trees::CheckedUnitStructuralTypeShape::Reference {
                        referent_identity,
                        access,
                        ..
                    } => {
                        program.normalized_type_identity(shell.referee)
                            == carrier.source.type_identity
                            && validation::stored_structural_type_identity(
                                program,
                                shell.referee,
                                &[],
                                &[],
                            ) == referent_identity
                            && access == carrier.source.access
                            && carrier.source.path.strip_suffix(&[
                                checked_trees::CheckedUnitStructuralPathSegment::Referent,
                            ]) == Some(retirement_path)
                    }
                    checked_trees::CheckedUnitStructuralTypeShape::BorrowedSliceView { .. } => {
                        program.normalized_type_identity(reference) == carrier.source.type_identity
                            && carrier.source.path == retirement_path
                    }
                    checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(bytes)
                        if bytes.is_borrowed() =>
                    {
                        program.normalized_type_identity(reference) == carrier.source.type_identity
                            && carrier.source.path == retirement_path
                    }
                    _ => false,
                };
                let carrier_matches = source_matches
                    && checked_trees::CheckedStructuralAccess::from(shell.access)
                        == carrier.source.access
                    && validation::stored_structural_type_identity(program, reference, &[], &[])
                        == destination.type_identity;
                if !carrier_matches
                    || !declaration.descriptor_cleanup
                    || old_value.type_identity != destination.type_identity
                {
                    return Err("replacement carrier descriptor cannot be discarded");
                }
                Some(CheckedNormalCleanupAction::Discard(
                    checked_trees::CheckedUnitPartialAffineDiscardPlan {
                        partition_leaf: Default::default(),
                        source: old_value.source.clone(),
                        path: old_value.path.clone(),
                        type_identity: old_value.type_identity,
                        case_conditions: Default::default(),
                    },
                ))
            } else {
                None
            };
            for disposition_handle in facts
                .contextual_cleanup
                .state_disposition_handles(machine, state)
            {
                let disposition = facts
                    .contextual_cleanup
                    .dispositions
                    .get(disposition_handle);
                if !(disposition.root == root
                    || replaced_place
                        .as_ref()
                        .is_some_and(|replaced| replaced.root == disposition.root)
                    || stored_place
                        .as_ref()
                        .is_some_and(|stored| stored.root == disposition.root))
                    || disposition.source
                        != (PermissionEventSource::Statement {
                            statement_index: *statement_index as usize,
                        })
                {
                    continue;
                }
                let place = validation::CanonicalPlace {
                    root: disposition.root,
                    segments: facts
                        .flow
                        .ownership
                        .segments
                        .span(disposition.path)
                        .ok_or("replacement disposition path span missing")?
                        .to_vec(),
                };
                let replaced = replaced_place.as_ref() == Some(&place)
                    || stored_place.as_ref() == Some(&place);
                if disposition.root != root && !replaced {
                    continue;
                }
                let path = if replaced {
                    destination.path.clone()
                } else {
                    crate::execution::terminal_unit::calls::projected_argument_path(
                        program,
                        state,
                        *statement_index as usize,
                        &place,
                        None,
                        None,
                    )
                    .ok_or("replacement death place has no projected argument path")?
                    .path
                };
                if !path.starts_with(&destination.path) && !destination.path.starts_with(&path) {
                    continue;
                }
                if facts
                    .contextual_cleanup
                    .is_selected_disposition(disposition_handle)
                {
                    if displaced {
                        return Err("displaced old value already has a selected disposition");
                    }
                    let mut region = super::actions::selected_region(
                        program,
                        facts,
                        shapes,
                        disposition_handle,
                        source.clone(),
                        replaced.then_some(destination.path.as_slice()),
                    )
                    .ok_or("replacement selected region cannot be formed")?;
                    if destination.path.starts_with(&region.path) {
                        region = destination.clone();
                    }
                    crate::shared::collections::push_unique(&mut selected_regions, region);
                }
                // Bind the complete checked subtree directly to the saved old value.
                // Its cleanup paths and conditions then keep the same coordinates as
                // the existing displaced actions, without losing the partition tree.
                let captured_path = displaced
                    .then(|| path.strip_prefix(destination.path.as_slice()))
                    .flatten();
                let captured_partition = captured_path.is_some();
                let selected_actions = super::actions::disposition_actions(
                    program,
                    facts,
                    targets,
                    shapes,
                    disposition_handle,
                    if captured_partition {
                        old_value.source.clone()
                    } else {
                        source.clone()
                    },
                    captured_path.or_else(|| replaced.then_some(destination.path.as_slice())),
                )?;
                selected_disposition = true;
                if path.starts_with(&destination.path) {
                    partitions.push(selected_actions.partition);
                }
                for mut action in selected_actions.actions {
                    let action_path = match &action {
                        CheckedNormalCleanupAction::Discard(action) => &action.path,
                        CheckedNormalCleanupAction::InvokeNominal(action) => &action.path,
                        CheckedNormalCleanupAction::RetireHome { .. } => {
                            return Err("replacement cannot retire a home at its destination");
                        }
                    };
                    if !captured_partition && !action_path.starts_with(&destination.path) {
                        if !destination.path.starts_with(action_path) {
                            continue;
                        }
                        let CheckedNormalCleanupAction::Discard(_) = action else {
                            return Err("enclosing replacement action is not a discard");
                        };
                        action = CheckedNormalCleanupAction::Discard(
                            checked_trees::CheckedUnitPartialAffineDiscardPlan {
                                partition_leaf: Default::default(),
                                source: source.clone(),
                                path: destination.path.clone(),
                                type_identity: destination.type_identity,
                                case_conditions: action.case_conditions(),
                            },
                        );
                    }
                    if displaced && !captured_partition {
                        let conditions = shapes
                            .cleanup_control
                            .conditions
                            .span_or_empty(action.case_conditions())
                            .to_vec();
                        let conditions = conditions
                            .into_iter()
                            .map(|mut condition| {
                                let path =
                                    shapes.cleanup_control.paths.span_or_empty(condition.path);
                                let path = path.strip_prefix(destination.path.as_slice())?.to_vec();
                                condition.path = shapes.cleanup_control.paths.insert_many(path);
                                Some(condition)
                            })
                            .collect::<Option<Vec<_>>>()
                            .ok_or("replacement condition path escapes its destination")?;
                        let case_conditions =
                            shapes.cleanup_control.conditions.insert_many(conditions);
                        let (source, path) = match &mut action {
                            CheckedNormalCleanupAction::Discard(action) => {
                                action.case_conditions = case_conditions;
                                (&mut action.source, &mut action.path)
                            }
                            CheckedNormalCleanupAction::InvokeNominal(action) => {
                                action.case_conditions = case_conditions;
                                (&mut action.source, &mut action.path)
                            }
                            CheckedNormalCleanupAction::RetireHome { .. } => {
                                return Err("displaced replacement cannot retire a home");
                            }
                        };
                        *path = path
                            .strip_prefix(destination.path.as_slice())
                            .ok_or("displaced action path escapes its destination")?
                            .to_vec();
                        *source = old_value.source.clone();
                    }
                    if !path.starts_with(&destination.path) {
                        match &mut action {
                            CheckedNormalCleanupAction::Discard(row) => {
                                row.partition_leaf = Default::default()
                            }
                            CheckedNormalCleanupAction::InvokeNominal(row) => {
                                row.partition_leaf = Default::default()
                            }
                            CheckedNormalCleanupAction::RetireHome { .. } => {}
                        }
                    }
                    actions.push(action);
                }
            }
            if let Some(discard) = carrier_discard
                && !actions.iter().any(|action| {
                    action.source() == discard.source()
                        && action.path() == discard.path()
                        && action.type_identity() == discard.type_identity()
                })
            {
                actions.push(discard);
            }
            if actions.is_empty() && !selected_disposition {
                return Err("replacement selected no cleanup actions and no disposition");
            }
            if selected_regions
                .iter()
                .any(|region| !region.path.is_empty())
            {
                super::order::merge(
                    program,
                    state,
                    *statement_index as usize,
                    root,
                    &mut actions,
                    &old_value.source,
                )
                .ok_or("replacement action order cannot merge its destinations")?;
            }
            if displaced || destination.path.is_empty() {
                super::actions::finish_home(
                    &mut actions,
                    CheckedNormalCleanupAction::RetireHome {
                        source: old_value.source.clone(),
                        type_identity: destination.type_identity,
                    },
                );
            }
            Ok(super::temporary_actions::SelectedActions {
                partitions: super::actions::partition_refs(&mut shapes.cleanup_control, partitions),
                actions,
                selected_regions,
            })
        })()
    }
}
mod residuals {
    /*
        Type-directed maximal residual complement for finite structural transfers.

        Only the complement is validated: a partially touched node must be a
        record, array, or exactly selected case whose fields are partial-affine
        leaves; each untouched subtree must be a finite tree of them. Closed alternatives
        remain whole when every alternative is custody-free; this does not select
        a case or authorize disposal of resource-bearing case payloads. A touched
        case instead retains only its selected payload siblings and common fields.
        Membership and nominal-hook admission remain the caller's responsibility.
        A moved subtree transfers whole with its own custody, so its shape (references included) is the
        consumer's concern, not this discard roster's.
    */
    use super::{
        CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
        CheckedUnitStructuralPathSegment,
    };
    use typed_trees::type_identity::TypeIdentityId;

    pub(super) fn reconstruct(
        types: &crate::execution::terminal_unit::types::TypeCatalog,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        root_type: TypeIdentityId,
        moved_paths: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
        max_residuals: usize,
    ) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
        let borrowed = types
            .iter()
            .map(|(identity, plan)| (*identity, plan))
            .collect();
        validation::reconstruct(
            &borrowed,
            source,
            root_type,
            moved_paths,
            &[],
            max_residuals,
        )
    }
}
pub(in crate::execution::terminal_unit) mod retained_owners {
    /*
        One retained value dependency roster for anonymous owner dispositions.
        Source checking owns eligibility and the exact call coordinate; the value
        graph identifies already-produced homes, independently of descriptor shape.
    */
    use super::actions::disposition_actions as selected_regions;
    use crate::execution::terminal_unit::types::ShapeCollector;
    use checked_trees::{
        CheckFacts, CheckedUnitCallCoordinate, CheckedUnitCallableEntry,
        CheckedUnitStructuralArgumentSourcePlan,
    };
    use language_semantics::PermissionEventSource;
    use symbols::SymbolHandle;
    pub(super) fn selected_actions(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
        state: SymbolHandle,
        coordinate: CheckedUnitCallCoordinate,
        target: SymbolHandle,
        root: facts::PlaceRoot,
        source: CheckedUnitStructuralArgumentSourcePlan,
    ) -> Result<super::temporary_actions::SelectedActions, &'static str> {
        let machine = super::owning_cleanup_machine(program, state);
        let mut actions = Vec::new();
        let mut partitions = Vec::new();
        let mut retirement = None;
        for disposition_handle in facts
            .contextual_cleanup
            .state_disposition_handles(machine, state)
        {
            let disposition = facts
                .contextual_cleanup
                .dispositions
                .get(disposition_handle);
            if disposition.root != root
                || !matches!(disposition.source, PermissionEventSource::Call { statement_index, call_ordinal, target_symbol }
                    if target_symbol == target && statement_index == coordinate.statement_index as usize
                        && call_ordinal == coordinate.call_ordinal as usize)
            {
                continue;
            }
            if retirement.is_none() {
                retirement = Some(
                    super::actions::home_retirement(program, shapes, disposition, source.clone())
                        .ok_or("retained owner home retirement cannot be formed")?,
                );
            }
            let selected = selected_regions(
                program,
                facts,
                targets,
                shapes,
                disposition_handle,
                source.clone(),
                None,
            )?;
            partitions.push(selected.partition);
            actions.extend(selected.actions);
        }
        if let Some(retirement) = retirement {
            super::actions::finish_home(&mut actions, retirement);
        }
        Ok(super::temporary_actions::SelectedActions {
            actions,
            partitions: super::actions::partition_refs(&mut shapes.cleanup_control, partitions),
            selected_regions: Vec::new(),
        })
    }

    pub(in crate::execution::terminal_unit) fn dependency_dispositions(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: SymbolHandle,
        root: checked_trees::CheckedStructuralValueHandle,
    ) -> Option<Vec<arena::Handle<checked_trees::ContextualCleanupDisposition>>> {
        let machine = super::owning_cleanup_machine(program, state);
        let mut selected = Vec::new();
        for value in facts.values.structural_values.value_dependencies(root)? {
            if value == root {
                continue;
            }
            let owner = facts::PlaceRoot::Expression(
                facts.values.structural_values.nodes.get(value).expression,
            );
            for (handle, row) in facts
                .contextual_cleanup
                .dispositions
                .iter()
                .filter(|(_, row)| {
                    row.machine == machine
                        && row.state == state
                        && row.root == owner
                        && matches!(row.source, PermissionEventSource::Call { .. })
                })
            {
                if !selected.iter().any(|handle| {
                    let existing = facts.contextual_cleanup.dispositions.get(*handle);
                    existing.root == row.root && existing.source == row.source
                }) {
                    selected.push(handle);
                }
            }
        }
        Some(selected)
    }

    pub(super) fn owner_group_actions(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
        disposition: arena::Handle<checked_trees::ContextualCleanupDisposition>,
        source: CheckedUnitStructuralArgumentSourcePlan,
    ) -> Result<super::temporary_actions::SelectedActions, &'static str> {
        let row = facts.contextual_cleanup.dispositions.get(disposition);
        let PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } = row.source
        else {
            return Err("retained owner disposition is not a call source");
        };
        selected_actions(
            program,
            facts,
            targets,
            shapes,
            row.state,
            CheckedUnitCallCoordinate {
                statement_index: u32::try_from(statement_index)
                    .map_err(|_| "retained owner statement index exceeds u32")?,
                call_ordinal: u32::try_from(call_ordinal)
                    .map_err(|_| "retained owner call ordinal exceeds u32")?,
            },
            target_symbol,
            row.root,
            source,
        )
    }
}
pub(super) mod temporary_actions {

    /*
        Finish temporary disposition actions after executable hook targets exist.
        Operand planning retains the exact dying owner. Source checking owns its
        maximal complement, including any transferred linear claim frontier.
        Completing that roster consumes the checker's contextual dispositions; it
        does not infer cleanup from a callee's name or fabricate another owner.
    */
    use super::actions::disposition_actions;
    use super::retained_owners::{owner_group_actions, selected_actions};
    use crate::execution::terminal_unit::types::ShapeCollector;
    use crate::flow::expressions;
    use crate::flow::indexed_statements;
    use checked_trees::{
        CheckFacts, CheckedNormalCleanupAction, CheckedUnitCallCoordinate,
        CheckedUnitCallableEntry, CheckedUnitEffectOperationPlan,
        CheckedUnitStructuralArgumentSourcePlan,
    };
    use language_semantics::PermissionEventSource;
    use symbols::SymbolHandle;

    #[cfg(test)]
    mod tests {

        use super::value_call_context;
        use checked_trees::{
            CheckFacts, CheckedComposedUnitControlMachinePlan,
            CheckedComposedUnitControlStatePlan as StatePlan,
            CheckedComposedUnitControlTerminatorPlan, CheckedControlResultPlan,
            CheckedStructuralAccess, CheckedUnitStructuralParameterPlan,
        };
        use symbols::SymbolHandle;

        fn body(state: SymbolHandle) -> StatePlan {
            StatePlan {
                state,
                structural_parameters: vec![CheckedUnitStructuralParameterPlan {
                    position: 0,
                    type_identity: Default::default(),
                    multiplicity: language_semantics::Multiplicity::Affine,
                    access: CheckedStructuralAccess::Owned,
                    primitive_referent: None,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                }],
                scalar_parameters: Vec::new(),
                parameter_storage: Vec::new(),
                erased_scalar_parameters: Vec::new(),
                erased_proof_parameters: Vec::new(),
                requires: Vec::new(),
                declaration_requires: None,
                entry_claims: Vec::new(),
                operations: Vec::new(),
                place_loans: Vec::new(),
                terminator: CheckedComposedUnitControlTerminatorPlan::ReturnUnit,
                terminator_statement_ordinal: 0,
                statement_count: 0,
                selected_exit: None,
                selected_copies: Vec::new(),
                normal_exit_cleanups: Default::default(),
            }
        }

        fn graph(owner: SymbolHandle, body: StatePlan) -> CheckedComposedUnitControlMachinePlan {
            CheckedComposedUnitControlMachinePlan {
                machine: owner,
                result: CheckedControlResultPlan::Unit,
                result_reference_sources: Vec::new(),
                natural_ranks: Vec::new(),
                rank_components: Default::default(),
                preserving_rank_edges: Vec::new(),
                body_qualifications: Vec::new(),
                contract_report_fingerprint: 0,
                contract_commitment: checked_trees::MachineContractCommitment::from_digest([0; 32]),
                contract_service_reach: Default::default(),
                service_reach: Default::default(),
                states: vec![body],
                declaration_requirements: Vec::new(),
            }
        }

        #[test]
        fn fused_body_uses_its_own_parameter_roster_not_the_graph_entry_owner() {
            let owner = SymbolHandle::from_arena_index(1);
            let state = SymbolHandle::from_arena_index(2);
            let mut facts = CheckFacts::default();
            facts
                .flow
                .terminal_unit_effects
                .composed_machines
                .push(graph(owner, body(state)));
            let context = value_call_context(
                &facts,
                &facts.flow.terminal_unit_effects.cleanup_control,
                state,
            )
            .expect("the source body survives inside another machine's graph");
            assert_eq!(context.parameters.len(), 1);
            assert_eq!(context.parameters[0].position, 0);
            assert!(context.release_owned.is_empty());
        }

        #[test]
        fn another_body_cannot_supply_a_missing_value_call_namespace() {
            let mut facts = CheckFacts::default();
            facts
                .flow
                .terminal_unit_effects
                .composed_machines
                .push(graph(
                    SymbolHandle::from_arena_index(1),
                    body(SymbolHandle::from_arena_index(2)),
                ));
            assert!(matches!(
                value_call_context(
                    &facts,
                    &facts.flow.terminal_unit_effects.cleanup_control,
                    SymbolHandle::from_arena_index(3)
                ),
                Err("missing executable parameter catalog")
            ));
        }

        #[test]
        fn repeated_fused_bodies_must_agree_on_the_shared_value_namespace() {
            let state = SymbolHandle::from_arena_index(3);
            let mut facts = CheckFacts::default();
            let plans = &mut facts.flow.terminal_unit_effects.composed_machines;
            plans.push(graph(SymbolHandle::from_arena_index(1), body(state)));
            plans.push(graph(SymbolHandle::from_arena_index(2), body(state)));
            assert!(
                value_call_context(
                    &facts,
                    &facts.flow.terminal_unit_effects.cleanup_control,
                    state
                )
                .is_ok()
            );
            facts.flow.terminal_unit_effects.composed_machines[1].states[0].structural_parameters
                [0]
            .position = 1;
            assert!(matches!(
                value_call_context(
                    &facts,
                    &facts.flow.terminal_unit_effects.cleanup_control,
                    state
                ),
                Err("surviving bodies disagree on the value-call operand namespace")
            ));
        }
    }

    /*
        Explain only the failed lifetime query using its completed capture graph.
        Unavailable graph rows remain unavailable; diagnostic traversal decides no
        custody, formation, lifetime coverage or cleanup disposition.
    */
    fn incomplete_lifetime_capture_diagnostic(
        program: &dyn validation::CheckingInput,
        control: &checked_trees::FlowControlFacts,
        operand: arena::Handle<checked_trees::FlowOperandFact>,
    ) -> String {
        let mut details = String::new();
        let mut pending = vec![operand];
        let mut visited = Vec::new();
        while let Some(handle) = pending.pop() {
            if !control.operands.is_valid(handle) || visited.contains(&handle) {
                continue;
            }
            visited.push(handle);
            let capture = control.operands.get(handle);
            details.push_str(&format!("\n  lifetime operand {handle:?}: {capture:?}"));
            if capture.type_reference.is_valid() {
                details.push_str(&format!(
                    "\n  declared lifetime frontier {:?}",
                    validation::borrow_contracts::borrow_carrying_owner_paths(
                        program,
                        capture.type_reference,
                    ),
                ));
            } else {
                details.push_str("\n  declared lifetime carrier unavailable");
            }
            pending.push(capture.backing);
            pending.extend(
                control
                    .operand_children
                    .span_or_empty(capture.constructed_children())
                    .iter()
                    .map(|child| child.value),
            );
            let mut references = control
                .captured_reference_leaves
                .span_or_empty(capture.reference_leaves)
                .to_vec();
            if let checked_trees::FlowOperandCompletion::Copied { copy, path } = capture.completion
            {
                if control.copied_values.is_valid(copy) {
                    let copy = control.copied_values.get(copy);
                    details.push_str(&format!(
                        "\n  copied selection {:?} from {:?}",
                        control.operand_child_paths.span_or_empty(path),
                        copy.source,
                    ));
                    pending.push(copy.source);
                    references.extend_from_slice(
                        control
                            .captured_reference_leaves
                            .span_or_empty(copy.references),
                    );
                } else {
                    details.push_str("\n  copied value capture unavailable");
                }
            }
            let mut retentions = vec![capture.retention];
            for leaf in references {
                details.push_str(&format!(
                    "\n  saved reference {:?}: {leaf:?}",
                    control.operand_child_paths.span_or_empty(leaf.path),
                ));
                retentions.push(leaf.retention);
            }
            for retention in retentions {
                if !control.operand_retentions.is_valid(retention) {
                    details.push_str("\n  lifetime retention unavailable");
                    continue;
                }
                let profile = control.operand_retentions.get(retention);
                details.push_str(&format!(
                    "\n  lifetime profile {retention:?}: {profile:?}; roots {:?}; projection {:?}",
                    control.operand_retained_roots.span_or_empty(profile.roots),
                    control
                        .operand_retained_paths
                        .span_or_empty(profile.stored_projection),
                ));
                pending.push(profile.stored_value);
                for leaf in control
                    .operand_retained_leaves
                    .span_or_empty(profile.leaves)
                {
                    details.push_str(&format!(
                        "\n  retained leaf {:?}: {leaf:?}; roots {:?}",
                        control.operand_retained_paths.span_or_empty(leaf.path),
                        control.retained_roots(leaf.retention),
                    ));
                }
            }
        }
        details
    }

    pub(in crate::execution::terminal_unit) struct SelectedActions {
        pub partitions: arena::HandleSpan<checked_trees::CheckedCleanupPartitionRef>,
        pub actions: Vec<CheckedNormalCleanupAction>,
        pub selected_regions: Vec<checked_trees::CheckedUnitStructuralArgumentPlan>,
    }

    /*
        The two storage owners expose one indexed operation schedule without
        flattening arena-backed operands or cloning nested call bodies.
    */
    #[derive(Clone, Copy)]
    enum OperationSequence<'plans> {
        Body(&'plans [CheckedUnitEffectOperationPlan]),
        Operands {
            sequence: &'plans checked_trees::CheckedOperandSequence,
            end: usize,
        },
    }

    impl<'plans> OperationSequence<'plans> {
        fn operands(sequence: &'plans checked_trees::CheckedOperandSequence) -> Self {
            Self::Operands {
                sequence,
                end: sequence.len(),
            }
        }

        fn len(self) -> usize {
            match self {
                Self::Body(operations) => operations.len(),
                Self::Operands { end, .. } => end,
            }
        }

        fn get(self, ordinal: usize) -> Option<&'plans CheckedUnitEffectOperationPlan> {
            match self {
                Self::Body(operations) => operations.get(ordinal),
                Self::Operands { sequence, end } => {
                    (ordinal < end).then(|| sequence.get(ordinal)).flatten()
                }
            }
        }

        fn prefix(self, end: usize) -> Option<Self> {
            if end > self.len() {
                return None;
            }
            Some(match self {
                Self::Body(operations) => Self::Body(&operations[..end]),
                Self::Operands { sequence, .. } => Self::Operands { sequence, end },
            })
        }
    }

    /*
        Construction scratch: selections retain only changed action streams and
        their exact schedule positions, not copied executable bodies.
    */
    #[derive(Default)]
    pub(in crate::execution::terminal_unit) struct OperationSelections {
        continuations: Vec<(usize, SelectedActions)>,
        constructions: Vec<(usize, Vec<(usize, CallSelections)>)>,
    }

    impl OperationSelections {
        fn is_empty(&self) -> bool {
            self.continuations.is_empty() && self.constructions.is_empty()
        }
    }

    pub(in crate::execution::terminal_unit) struct CallSelections {
        operands: OperationSelections,
        completions: OperationSelections,
    }

    impl CallSelections {
        fn is_empty(&self) -> bool {
            self.operands.is_empty() && self.completions.is_empty()
        }
    }

    /*
        Only attached releases publish delayed deaths. The collector's arena can
        also contain rows from a plan that subsequently failed construction.
    */
    fn release_owned_dispositions<'plans>(
        operations: impl IntoIterator<Item = &'plans CheckedUnitEffectOperationPlan>,
        control: &checked_trees::CheckedCleanupControlPlans,
    ) -> Option<Vec<arena::Handle<checked_trees::ContextualCleanupDisposition>>> {
        let mut dispositions = Vec::new();
        for operation in operations
            .into_iter()
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        {
            if let CheckedUnitEffectOperationPlan::ReleaseReference { owner_cleanups, .. } =
                operation
            {
                for owner in control.reference_owner_cleanups.span(*owner_cleanups)? {
                    crate::shared::collections::push_unique(&mut dispositions, owner.disposition);
                }
            }
        }
        Some(dispositions)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::execution::terminal_unit) fn operations(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
        operations: &[CheckedUnitEffectOperationPlan],
    ) -> Result<OperationSelections, &'static str> {
        let release_owned = release_owned_dispositions(operations, &shapes.cleanup_control)
            .ok_or("release-owned owner cleanup span missing")?;
        sequence_actions(
            program,
            facts,
            targets,
            shapes,
            machine,
            state,
            parameters,
            OperationSequence::Body(operations),
            None,
            &release_owned,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn sequence_actions(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
        sequence: OperationSequence<'_>,
        completion: Option<(&CheckedUnitEffectOperationPlan, OperationSequence<'_>)>,
        release_owned: &[arena::Handle<checked_trees::ContextualCleanupDisposition>],
    ) -> Result<OperationSelections, &'static str> {
        (|| {
            let mut selected = OperationSelections::default();
            for operation_index in 0..sequence.len() {
                let operation = sequence
                    .get(operation_index)
                    .ok_or("operation ordinal exceeds its sequence")?;
                if let CheckedUnitEffectOperationPlan::ReleaseReference { owner_cleanups, .. } =
                    operation
                {
                    let owners = shapes
                        .cleanup_control
                        .reference_owner_cleanups
                        .span(*owner_cleanups)
                        .ok_or("release reference owner cleanup span missing")?
                        .to_vec();
                    for (ordinal, owner) in owners.into_iter().enumerate() {
                        let actions = owner_group_actions(
                            program,
                            facts,
                            targets,
                            shapes,
                            owner.disposition,
                            owner.owner.source,
                        )?;
                        if actions.actions.is_empty() && actions.partitions.is_empty() {
                            return Err(
                                "release reference owner group selected no cleanup actions",
                            );
                        }
                        let partitions = actions.partitions;
                        let actions = shapes.cleanup_control.actions.insert_many(actions.actions);
                        let handle = owner_cleanups.handle_at(
                            u32::try_from(ordinal)
                                .map_err(|_| "release owner ordinal exceeds u32")?,
                        );
                        let owner = shapes
                            .cleanup_control
                            .reference_owner_cleanups
                            .get_mut(handle);
                        owner.actions = actions;
                        owner.partitions = partitions;
                    }
                    continue;
                }

                if let CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { calls, .. },
                )
                | CheckedUnitEffectOperationPlan::EvaluateStructuralValue { calls, .. } =
                    operation
                {
                    let mut nested = Vec::new();
                    for (call_index, call) in calls.iter().enumerate() {
                        let actions = value_call_actions(
                            program,
                            facts,
                            targets,
                            shapes,
                            machine,
                            state,
                            parameters,
                            call,
                            release_owned,
                        )?;
                        if !actions.is_empty() {
                            nested.push((call_index, actions));
                        }
                    }
                    if !nested.is_empty() {
                        selected.constructions.push((operation_index, nested));
                    }
                }
                match operation {
                    CheckedUnitEffectOperationPlan::ContinuationCleanup {
                        boundary: checked_trees::CheckedUnitCleanupBoundary::Replacement { .. },
                        ..
                    } => {
                        selected.continuations.push((
                            operation_index,
                            super::replacements::actions(
                                program, facts, targets, shapes, state, parameters, operation,
                            )?,
                        ));
                    }
                    CheckedUnitEffectOperationPlan::ContinuationCleanup {
                        boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate),
                        cleanup_actions,
                        ..
                    } => {
                        let prior = sequence
                            .prefix(operation_index)
                            .ok_or("call boundary prefix exceeds its sequence")?;
                        let mut local = None;
                        for position in 0..prior.len() {
                            for candidate in prior
                                .get(position)
                                .ok_or("producer prefix ordinal exceeds its sequence")?
                                .with_value_calls()
                            {
                                if candidate.invocation_coordinate() != Some(*coordinate) {
                                    continue;
                                }
                                if local.replace(candidate).is_some() {
                                    return Err("call boundary names two local producers");
                                }
                            }
                        }
                        let enclosing = completion.filter(|(consumer, _)| {
                            consumer.invocation_coordinate() == Some(*coordinate)
                        });
                        let (consumer, producers) = match (local, enclosing) {
                            (Some(_), Some(_)) => {
                                return Err(
                                    "call boundary has both a local and a completion producer",
                                );
                            }
                            (Some(consumer), None) => (consumer, prior),
                            (None, Some(completion)) => completion,
                            (None, None) => {
                                return Err(
                                    "call boundary has no producer in its prefix or completion",
                                );
                            }
                        };
                        let actions = call_actions(
                            program,
                            facts,
                            targets,
                            shapes,
                            state,
                            consumer,
                            producers,
                            cleanup_actions,
                            release_owned,
                        )?;
                        selected.continuations.push((operation_index, actions));
                    }
                    _ => {}
                }
            }
            Ok(selected)
        })()
    }

    #[allow(clippy::too_many_arguments)]
    fn value_call_actions(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
        call: &checked_trees::CheckedStructuralValueCall,
        release_owned: &[arena::Handle<checked_trees::ContextualCleanupDisposition>],
    ) -> Result<CallSelections, &'static str> {
        let preparation = OperationSequence::operands(call.operands());
        Ok(CallSelections {
            operands: sequence_actions(
                program,
                facts,
                targets,
                shapes,
                machine,
                state,
                parameters,
                preparation,
                None,
                release_owned,
            )?,
            completions: if let Some(completions) = call.completions() {
                sequence_actions(
                    program,
                    facts,
                    targets,
                    shapes,
                    machine,
                    state,
                    parameters,
                    OperationSequence::operands(completions),
                    call.operation().map(|operation| (operation, preparation)),
                    release_owned,
                )?
            } else {
                OperationSelections::default()
            },
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn call_actions(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
        state: SymbolHandle,
        consumer: &CheckedUnitEffectOperationPlan,
        producers: OperationSequence<'_>,
        cleanup_actions: &[CheckedNormalCleanupAction],
        release_owned: &[arena::Handle<checked_trees::ContextualCleanupDisposition>],
    ) -> Result<SelectedActions, &'static str> {
        (|| {
            let machine = super::owning_cleanup_machine(program, state);
            let coordinate = consumer
                .invocation_coordinate()
                .ok_or("call-boundary consumer has no invocation coordinate")?;
            let mut sources = Vec::new();
            let mut completed = Vec::new();
            let mut partitions = Vec::new();
            for action in cleanup_actions {
                let source = action.source();
                if sources.contains(source) {
                    continue;
                }
                sources.push(source.clone());
                if let CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } =
                    *source
                    && consumer
                        .structural_result_binding()
                        .is_some_and(|result| result.binding_ordinal == binding_ordinal)
                {
                    if cleanup_actions
                        .iter()
                        .filter(|action| action.source() == source)
                        .any(|action| !matches!(action, CheckedNormalCleanupAction::Discard(_)))
                    {
                        return Err("discarded call result also carries a non-discard cleanup");
                    }
                    completed.extend(
                        cleanup_actions
                            .iter()
                            .filter(|action| action.source() == source)
                            .cloned(),
                    );
                    continue;
                }
                let root = match *source {
                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => {
                        let mut producer = None;
                        for position in 0..producers.len() {
                            for candidate in producers
                                .get(position)
                                .ok_or("producer prefix ordinal exceeds its sequence")?
                                .with_value_calls()
                            {
                                if !candidate
                                    .structural_result_binding()
                                    .is_some_and(|result| result.binding_ordinal == binding_ordinal)
                                {
                                    continue;
                                }
                                if producer.replace(candidate).is_some() {
                                    return Err("cleanup source names two result producers");
                                }
                            }
                        }
                        let producer = producer.ok_or("cleanup source names no result producer")?;
                        super::normal_edges::producer_root(facts, state, producer)
                    }
                    CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        expression,
                        ..
                    } => Some(facts::PlaceRoot::Expression(expression)),
                    _ => return Err("cleanup source is neither a call result nor an expression"),
                };
                let contextual = root.is_some_and(|root| {
                    facts
                        .contextual_cleanup
                        .any_state_disposition(machine, state, |_, row| {
                            row.root == root
                                && matches!(
                                    row.source,
                                    PermissionEventSource::Call {
                                        statement_index,
                                        call_ordinal,
                                        ..
                                    } if statement_index == coordinate.statement_index as usize
                                        && call_ordinal == coordinate.call_ordinal as usize
                                )
                        })
                });
                if !contextual {
                    if cleanup_actions.iter().any(|action| {
                        action.source() == source
                            && matches!(action, CheckedNormalCleanupAction::RetireHome { .. })
                    }) {
                        return Err("home retirement without a contextual call disposition");
                    }
                    completed.extend(
                        cleanup_actions
                            .iter()
                            .filter(|action| action.source() == source)
                            .cloned(),
                    );
                    continue;
                }
                let consumer = facts
                    .flow
                    .control
                    .call_at(
                        state,
                        coordinate.statement_index as usize,
                        coordinate.call_ordinal as usize,
                    )
                    .ok_or("call boundary names no consumer in the call schedule")?;
                if release_owned.iter().any(|handle| {
                    let disposition = facts.contextual_cleanup.dispositions.get(*handle);
                    disposition.machine == machine
                        && disposition.state == state
                        && Some(disposition.root) == root
                        && matches!(disposition.source,
                            PermissionEventSource::Call { statement_index, call_ordinal, target_symbol }
                            if target_symbol == consumer.target_symbol
                                && statement_index == coordinate.statement_index as usize
                                && call_ordinal == coordinate.call_ordinal as usize)
                }) {
                    continue;
                }
                let selected = selected_actions(
                    program,
                    facts,
                    targets,
                    shapes,
                    state,
                    coordinate,
                    consumer.target_symbol,
                    root.ok_or("contextual cleanup source has no place root")?,
                    source.clone(),
                )?;
                partitions.extend(
                    shapes
                        .cleanup_control
                        .partition_refs
                        .span_or_empty(selected.partitions)
                        .iter()
                        .map(|row| row.partition),
                );
                completed.extend(selected.actions);
            }
            Ok(SelectedActions {
                partitions: super::actions::partition_refs(&mut shapes.cleanup_control, partitions),
                actions: completed,
                selected_regions: Vec::new(),
            })
        })()
    }

    enum OperationSequenceMut<'plans> {
        Body(&'plans mut [CheckedUnitEffectOperationPlan]),
        Operands(&'plans mut checked_trees::CheckedOperandSequence),
    }

    impl OperationSequenceMut<'_> {
        fn get_mut(&mut self, ordinal: usize) -> Option<&mut CheckedUnitEffectOperationPlan> {
            match self {
                Self::Body(operations) => operations.get_mut(ordinal),
                Self::Operands(sequence) => sequence.get_mut(ordinal),
            }
        }
    }

    pub(in crate::execution::terminal_unit) fn install(
        operations: &mut [CheckedUnitEffectOperationPlan],
        selected: OperationSelections,
    ) -> Result<(), &'static str> {
        install_sequence(OperationSequenceMut::Body(operations), selected)
    }

    fn install_sequence(
        mut operations: OperationSequenceMut<'_>,
        selected: OperationSelections,
    ) -> Result<(), &'static str> {
        for (position, actions) in selected.continuations {
            let CheckedUnitEffectOperationPlan::ContinuationCleanup {
                cleanup_actions,
                selected_regions,
                partitions,
                ..
            } = operations
                .get_mut(position)
                .ok_or("cleanup continuation position exceeds its sequence")?
            else {
                return Err("cleanup continuation position is not a continuation boundary");
            };
            *partitions = actions.partitions;
            *cleanup_actions = actions.actions;
            *selected_regions = actions.selected_regions;
        }
        for (position, selections) in selected.constructions {
            let calls = match operations
                .get_mut(position)
                .ok_or("cleanup construction position exceeds its sequence")?
            {
                CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { calls, .. },
                )
                | CheckedUnitEffectOperationPlan::EvaluateStructuralValue { calls, .. } => calls,
                _ => return Err("cleanup construction position has no value calls"),
            };
            for (call_index, selected) in selections {
                install_value_call(
                    calls
                        .get_mut(call_index)
                        .ok_or("cleanup call ordinal exceeds its value calls")?,
                    selected,
                )?;
            }
        }
        Ok(())
    }

    fn install_value_call(
        call: &mut checked_trees::CheckedStructuralValueCall,
        selected: CallSelections,
    ) -> Result<(), &'static str> {
        install_sequence(
            OperationSequenceMut::Operands(call.operands_mut()),
            selected.operands,
        )?;
        if let Some(completions) = call.completions_mut() {
            install_sequence(
                OperationSequenceMut::Operands(completions),
                selected.completions,
            )
        } else if selected.completions.is_empty() {
            Ok(())
        } else {
            Err("value call selected completion actions without a completion sequence")
        }
    }

    struct ValueCallContext<'plans> {
        parameters: &'plans [checked_trees::CheckedUnitStructuralParameterPlan],
        release_owned: Vec<arena::Handle<checked_trees::ContextualCleanupDisposition>>,
    }

    /*
        A fused graph's entry machine is not the source owner of every body it
        contains. The exact state identity selects the operand namespace. If several
        surviving graphs contain that body, their published parameter and release
        rosters must agree before the shared value-call schedule can use either.
    */
    fn value_call_context<'plans>(
        facts: &'plans CheckFacts,
        control: &checked_trees::CheckedCleanupControlPlans,
        state: SymbolHandle,
    ) -> Result<ValueCallContext<'plans>, &'static str> {
        let unit = &facts.flow.terminal_unit_effects;
        let bodies = unit
            .machines
            .iter()
            .chain(
                facts
                    .flow
                    .terminal_partial_affine_unit_cleanups
                    .machines
                    .iter()
                    .map(|plan| &plan.machine),
            )
            .filter(|plan| plan.state == state)
            .map(|plan| {
                (
                    plan.structural_parameters.as_slice(),
                    plan.operations.as_slice(),
                )
            })
            .chain(
                unit.composed_machines
                    .iter()
                    .flat_map(|plan| &plan.states)
                    .filter(|plan| plan.state == state)
                    .map(|plan| {
                        (
                            plan.structural_parameters.as_slice(),
                            plan.operations.as_slice(),
                        )
                    }),
            );
        let mut context: Option<ValueCallContext<'plans>> = None;
        for (parameters, operations) in bodies {
            let release_owned = release_owned_dispositions(operations, control)
                .ok_or("invalid retained owner release schedule")?;
            if let Some(existing) = &context {
                if existing.parameters != parameters || existing.release_owned != release_owned {
                    return Err("surviving bodies disagree on the value-call operand namespace");
                }
            } else {
                context = Some(ValueCallContext {
                    parameters,
                    release_owned,
                });
            }
        }
        context.ok_or("missing executable parameter catalog")
    }

    /*
        Complete reads, computed calls and retained value calls from their checked
        contextual dispositions.
        Action storage is rebuilt once: scratch geometry never remains as a second
        published cleanup stream beside the selected hooks.
    */
    pub(crate) fn finalize_completion_actions(
        program: &dyn ::validation::CheckingInput,
        facts: &mut CheckFacts,
    ) -> Result<(), String> {
        let mut shapes = ShapeCollector::new(program);
        shapes.cleanup_control =
            std::mem::take(&mut facts.flow.terminal_unit_effects.cleanup_control).into();
        let mut refusal = String::from("missing executable cleanup catalog");
        let replacements = (|| {
            let targets = facts
                .flow
                .terminal_unit_effects
                .machines
                .iter()
                .chain(
                    facts
                        .flow
                        .terminal_partial_affine_unit_cleanups
                        .machines
                        .iter()
                        .map(|plan| &plan.machine),
                )
                .map(checked_trees::CheckedUnitEffectMachinePlan::callable_entry)
                .chain(
                    facts
                        .flow
                        .terminal_unit_effects
                        .composed_machines
                        .iter()
                        .filter_map(
                            checked_trees::CheckedComposedUnitControlMachinePlan::callable_entry,
                        ),
                )
                .collect::<Vec<_>>();
            if let Err(reason) = finalize_value_completions(program, facts, &targets, &mut shapes) {
                refusal = reason;
                return None;
            }
            let mut value_calls = Vec::new();
            for (handle, call) in facts.values.structural_values.scheduled_calls() {
                let value = facts.values.structural_values.nodes.get(call.value);
                let context = match value_call_context(facts, &shapes.cleanup_control, value.state)
                {
                    Ok(context) => context,
                    Err(reason) => {
                        refusal = format!(
                            "value call in machine `{}`, state `{}`: {reason}",
                            program.symbols.name(value.machine),
                            program.symbols.name(value.state),
                        );
                        return None;
                    }
                };
                refusal = format!(
                    "value call in machine `{}`, state `{}`: missing operand or completion cleanup correspondence",
                    program.symbols.name(value.machine),
                    program.symbols.name(value.state),
                );
                let selected = value_call_actions(
                    program,
                    facts,
                    &targets,
                    &mut shapes,
                    value.machine,
                    value.state,
                    context.parameters,
                    call,
                    &context.release_owned,
                )
                .ok()?;
                if !selected.is_empty() {
                    value_calls.push((handle, selected));
                }
            }
            let plans = &facts.values.scalar_computations;
            let mut replacements = Vec::new();
            let mut observation_replacements = Vec::new();
            for (node_handle, node) in plans.nodes.iter() {
                if !node.normal_cleanup_actions.is_empty() {
                    let receiver_value = node.kind.observed_receiver();
                    if !facts
                        .values
                        .structural_values
                        .nodes
                        .is_valid(receiver_value)
                    {
                        refusal =
                            String::from("scalar observation cleanup lost its retained receiver");
                        return None;
                    }
                    let receiver = facts.values.structural_values.nodes.get(receiver_value);
                    let type_identity = shapes.add_type(receiver.type_reference, &[], &[])?;
                    let selected = plans
                        .temporary_cleanup_actions
                        .span(node.normal_cleanup_actions)?;
                    if selected
                        .iter()
                        .any(|action| action.type_identity() != type_identity)
                    {
                        return None;
                    }
                    observation_replacements.push((node_handle, selected.to_vec()));
                }
                let (source_call, operands, retained, authority) = match node.kind {
                    checked_trees::CheckedScalarComputationKind::Call {
                        source_call,
                        structural_arguments,
                        temporary_cleanups,
                        ref callee,
                        ..
                    } => (
                        source_call,
                        structural_arguments,
                        temporary_cleanups,
                        callee.authority_value(),
                    ),
                    _ => continue,
                };
                if retained.is_empty() {
                    continue;
                }
                let call = facts.flow.control.calls.get(source_call.statement()?);
                let state_flow = facts.flow.control.states.iter().find_map(|(_, row)| {
                    crate::flow::call_flows(&facts.flow.control, row.calls)
                        .iter()
                        .any(|candidate| std::ptr::eq(candidate, call))
                        .then_some(row)
                })?;
                let site = format!(
                    "machine `{}`, state `{}`, statement {}, call {}",
                    program.symbols.name(state_flow.machine_symbol),
                    program.symbols.name(state_flow.state_symbol),
                    call.statement_index,
                    call.call_ordinal
                );
                refusal = format!("{site}: missing checked operand/root/completion correspondence");
                let operands = plans.structural_arguments.span(operands)?;
                let coordinate = CheckedUnitCallCoordinate {
                    statement_index: u32::try_from(call.statement_index).ok()?,
                    call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                };
                let values = &facts.values.structural_values;
                struct ReservedOperand<'values> {
                    argument_ordinal: Option<u32>,
                    operand: arena::Handle<checked_trees::FlowOperandFact>,
                    root: &'values checked_trees::CheckedStructuralValueRoot,
                    dependencies: Vec<checked_trees::CheckedStructuralValueHandle>,
                    retained_owners: Vec<facts::PlaceRoot>,
                }
                let mut reserved = Vec::new();
                for row in plans.temporary_cleanups.span(retained)? {
                    let expression = match row.argument_ordinal {
                        Some(ordinal) => {
                            match operands.get(ordinal as usize)?.as_place()?.source.clone() {
                                CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                                    expression,
                                    ..
                                } => expression,
                                _ => return None,
                            }
                        }
                        None if values.nodes.is_valid(authority) => {
                            values.nodes.get(authority).expression
                        }
                        None => return None,
                    };
                    let root = values.root_for_expression(
                        state_flow.state_symbol,
                        coordinate.statement_index,
                        expression,
                    )?;
                    if !facts.flow.control.operands.is_valid(row.operand)
                        || !call
                            .operands
                            .handles()
                            .any(|operand| operand == row.operand)
                    {
                        refusal = format!("{site}: temporary owner has no captured call operand");
                        return None;
                    }
                    let retained_owners = crate::flow::captured_lifetime_roots(
                        program,
                        &facts.flow.control,
                        facts.flow.control.operands.get(row.operand),
                    );
                    let Some(retained_owners) = retained_owners else {
                        refusal = format!(
                            "{site}: temporary operand has incomplete backing lifetime coverage"
                        );
                        if ::validation::trace_flags::check_sites() {
                            refusal.push_str(&incomplete_lifetime_capture_diagnostic(
                                program,
                                &facts.flow.control,
                                row.operand,
                            ));
                        }
                        return None;
                    };
                    reserved.push(ReservedOperand {
                        argument_ordinal: row.argument_ordinal,
                        operand: row.operand,
                        root,
                        dependencies: values.value_dependencies(root.root)?,
                        retained_owners,
                    });
                }
                let machine = program.machine_by_symbol(state_flow.machine_symbol)?;
                let state = program.state_in_machine(machine, state_flow.state_symbol)?;
                let (_, statement) = indexed_statements(program, state.statement_nodes)
                    .find(|(statement_index, _)| *statement_index == call.statement_index)?;
                let completed = crate::checks::ownership_occurrences::completed_expressions(
                    program,
                    machine,
                    state,
                    call.statement_index,
                    statement,
                    &facts.operators,
                );
                let mut owners = Vec::new();
                for disposition_handle in facts
                    .contextual_cleanup
                    .state_disposition_handles(state_flow.machine_symbol, state_flow.state_symbol)
                {
                    let disposition = facts
                        .contextual_cleanup
                        .dispositions
                        .get(disposition_handle);
                    if !matches!(
                        disposition.source,
                        PermissionEventSource::Call {
                            statement_index,
                            call_ordinal,
                            ..
                        } if statement_index == call.statement_index
                            && call_ordinal == call.call_ordinal
                    ) {
                        continue;
                    }
                    let facts::PlaceRoot::Expression(expression) = disposition.root else {
                        continue;
                    };
                    let root = values.root_for_expression(
                        state_flow.state_symbol,
                        coordinate.statement_index,
                        expression,
                    );
                    let Some(root) = root else {
                        if reserved.iter().any(|operand| {
                            operand.retained_owners.contains(&disposition.root)
                                || operand
                                    .dependencies
                                    .iter()
                                    .any(|value| values.nodes.get(*value).expression == expression)
                        }) {
                            refusal = format!(
                                "{site}: anonymous backing {expression:?} has no published completed root"
                            );
                            return None;
                        }
                        continue;
                    };
                    let mut containing = reserved.iter().filter(|operand| {
                        operand.dependencies.contains(&root.root)
                            || operand.retained_owners.contains(&disposition.root)
                    });
                    let Some(first) = containing.next() else {
                        continue;
                    };
                    let exact = reserved
                        .iter()
                        .filter(|operand| operand.root.root == root.root)
                        .collect::<Vec<_>>();
                    let operand = match exact.as_slice() {
                        [slot] => *slot,
                        [] => first,
                        _ => {
                            refusal = format!(
                                "{site}: completed owner {expression:?} has ambiguous reserved operand storage"
                            );
                            return None;
                        }
                    };
                    if owners.iter().any(|(_, _, owner, _)| *owner == root.root) {
                        continue;
                    }
                    let actions = selected_actions(
                        program,
                        facts,
                        &targets,
                        &mut shapes,
                        state_flow.state_symbol,
                        coordinate,
                        call.target_symbol,
                        disposition.root,
                        CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                            value_dependency: None,
                            expression,
                            logical_root_type_identity: None,
                        },
                    )
                    .ok()?;
                    owners.push((
                        operand.argument_ordinal,
                        operand.operand,
                        root.root,
                        actions,
                    ));
                }
                for operand in &reserved {
                    let root = operand.root;
                    if owners.iter().any(|(_, _, owner, _)| *owner == root.root) {
                        continue;
                    }
                    if validation::reference_shell(program, root.type_reference).is_some() {
                        shapes.add_type(root.type_reference, &[], &[])?;
                        let selected = super::descriptor_retirement(
                            program,
                            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                                value_dependency: None,
                                expression: root.expression,
                                logical_root_type_identity: None,
                            },
                            root.type_reference,
                        )?;
                        if !selected.is_empty() {
                            owners.push((
                                operand.argument_ordinal,
                                operand.operand,
                                root.root,
                                SelectedActions {
                                    actions: selected,
                                    partitions: Default::default(),
                                    selected_regions: Vec::new(),
                                },
                            ));
                        }
                    }
                }
                for operand in &reserved {
                    if !owners
                        .iter()
                        .any(|(_, _, owner, _)| *owner == operand.root.root)
                    {
                        refusal = format!(
                            "{site}: completed operand `{}` ({:?}) has no selected normal death",
                            expressions(program).display_name(operand.root.expression),
                            operand.root.expression,
                        );
                        return None;
                    }
                }
                let mut ordered = Vec::new();
                for (ordinal, operand, owner, actions) in owners {
                    let expression = values.nodes.get(owner).expression;
                    let mut positions = completed
                        .iter()
                        .enumerate()
                        .filter(|(_, candidate)| **candidate == expression);
                    let Some((position, _)) = positions.next() else {
                        refusal =
                            format!("{site}: owner {expression:?} has no authored completion");
                        return None;
                    };
                    if positions.next().is_some() {
                        refusal = format!(
                            "{site}: owner {expression:?} has multiple authored completions"
                        );
                        return None;
                    }
                    ordered.push((position, ordinal, operand, owner, actions));
                }
                ordered.sort_by_key(|(position, ..)| std::cmp::Reverse(*position));
                replacements.push((node_handle, ordered));
            }
            Some((replacements, value_calls, observation_replacements))
        })();
        facts.flow.terminal_unit_effects.cleanup_control =
            std::mem::take(&mut shapes.cleanup_control).into_inner();
        let (replacements, value_calls, observation_replacements) = replacements.ok_or(refusal)?;
        for (handle, selected) in value_calls {
            install_value_call(
                facts
                    .values
                    .structural_values
                    .scheduled_call_mut(handle)
                    .ok_or("normal cleanup lost its scheduled value call")?,
                selected,
            )?;
        }
        let mut actions = arena::Arena::new();
        let mut cleanups = arena::Arena::new();
        for (node_handle, owners) in replacements {
            let groups =
                owners
                    .into_iter()
                    .map(|(_, argument_ordinal, operand, owner, selected)| {
                        checked_trees::CheckedScalarComputationTemporaryCleanup {
                            argument_ordinal,
                            operand,
                            owner,
                            partitions: selected.partitions,
                            actions: actions.insert_many(selected.actions),
                        }
                    });
            let checked_trees::CheckedScalarComputationKind::Call {
                temporary_cleanups, ..
            } = &mut facts
                .values
                .scalar_computations
                .nodes
                .get_mut(node_handle)
                .kind
            else {
                return Err("temporary cleanup finalization lost its checked call".into());
            };
            *temporary_cleanups = cleanups.insert_many(groups);
        }
        facts.values.scalar_computations.temporary_cleanups = cleanups;
        for (identity, selected) in observation_replacements {
            facts
                .values
                .scalar_computations
                .nodes
                .get_mut(identity)
                .normal_cleanup_actions = actions.insert_many(selected);
        }
        facts.values.scalar_computations.temporary_cleanup_actions = actions;
        facts
            .flow
            .terminal_unit_effects
            .adopt_structural_types(shapes.types.into_values())
            .map_err(|(identity, reason)| {
                format!("temporary cleanup has conflicting executable structural type {identity:?}: {reason}")
            })?;
        Ok(())
    }

    /*
        Preserve disposition order across the descriptor and its backing owners.
        Home retirement follows that home's last selected action, not the observer's
        result kind or an inferred call argument.
    */
    fn finalize_value_completions(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        targets: &[CheckedUnitCallableEntry<'_>],
        shapes: &mut ShapeCollector<'_>,
    ) -> Result<(), String> {
        struct Completion {
            plan: checked_trees::CheckedValueCompletionPlan,
            partitions: Vec<arena::Handle<checked_trees::CheckedCleanupPartitionPlan>>,
            actions: Vec<CheckedNormalCleanupAction>,
            retirements: Vec<CheckedNormalCleanupAction>,
        }
        let mut completions: Vec<Completion> = Vec::new();
        for (handle, disposition) in facts.contextual_cleanup.dispositions.iter() {
            if !disposition.completion.is_valid() {
                continue;
            }
            let site = format!(
                "value completion in machine `{}`, state `{}`, root {:?}",
                program.symbols.name(disposition.machine),
                program.symbols.name(disposition.state),
                disposition.root,
            );
            let PermissionEventSource::Statement { statement_index } = disposition.source else {
                return Err(format!("{site}: completion is not attached to a statement"));
            };
            let statement_index = u32::try_from(statement_index).map_err(|_| {
                format!("{site}: statement index exceeds the executable coordinate")
            })?;
            let expression = disposition
                .root
                .as_expression()
                .ok_or_else(|| format!("{site}: completed root is not an expression"))?;
            let source = CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                value_dependency: None,
                expression,
                logical_root_type_identity: None,
            };
            let selected = disposition_actions(
                program,
                facts,
                targets,
                shapes,
                handle,
                source.clone(),
                None,
            )
            .map_err(|reason| format!("{site}: {reason}"))?;
            let retirement = super::actions::home_retirement(program, shapes, disposition, source)
                .ok_or_else(|| {
                    format!("{site}: cannot resolve the retained home for retirement")
                })?;
            let position = completions
                .iter()
                .position(|completion| {
                    let plan = &completion.plan;
                    plan.machine == disposition.machine
                        && plan.state == disposition.state
                        && plan.statement_index == statement_index
                        && plan.expression == disposition.completion
                })
                .unwrap_or_else(|| {
                    completions.push(Completion {
                        plan: checked_trees::CheckedValueCompletionPlan {
                            machine: disposition.machine,
                            state: disposition.state,
                            statement_index,
                            expression: disposition.completion,
                            partitions: Default::default(),
                            actions: Default::default(),
                        },
                        partitions: Vec::new(),
                        actions: Vec::new(),
                        retirements: Vec::new(),
                    });
                    completions.len() - 1
                });
            let completion = &mut completions[position];
            completion.partitions.push(selected.partition);
            completion.actions.extend(selected.actions);
            if !completion
                .retirements
                .iter()
                .any(|existing| existing.source() == retirement.source())
            {
                completion.retirements.push(retirement);
            }
        }
        for mut completion in completions {
            for retirement in completion.retirements {
                super::actions::finish_home(&mut completion.actions, retirement);
            }
            if completion.actions.is_empty() {
                return Err("value completion has no disposition or retirement actions".into());
            }
            completion.plan.partitions =
                super::actions::partition_refs(&mut shapes.cleanup_control, completion.partitions);
            completion.plan.actions = shapes
                .cleanup_control
                .actions
                .insert_many(completion.actions);
            shapes
                .cleanup_control
                .value_completions
                .append(completion.plan);
        }
        Ok(())
    }
}
mod temporary_descriptors {
    /*
        End anonymous descriptor inputs at their borrowing call's normal return.
        Input custody distinguishes retained descriptor storage from its referent.
        A returned child loan retains ancestry after an anonymous parent descriptor
        retires; a result borrowing that descriptor's own storage keeps its home.
        Fresh affine carriers retire in reverse production order. Named storage
        and owned transfers retain their existing lifetime owners. The recursive
        type capability authorizes descriptor-only retirement, including containing
        aggregates, never disposal of borrowed contents or a nominal cleanup.
    */
    use super::super::types::ShapeCollector;
    use checked_trees::CheckedUnitEffectOperationPlan;
    use language_semantics::Multiplicity;

    #[allow(clippy::too_many_arguments)]
    pub(super) fn completions(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        results: &[(
            checked_trees::CheckedUnitStructuralResultBindingPlan,
            facts::PlaceRoot,
        )],
        invocation: &CheckedUnitEffectOperationPlan,
        preparation: &[CheckedUnitEffectOperationPlan],
        shapes: &ShapeCollector<'_>,
    ) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
        let mut completions = Vec::new();
        let arguments = invocation.structural_operands();
        let custody = invocation.call().and_then(|call| call.structural_custody());
        for (index, argument) in arguments.iter().enumerate() {
            let index = u32::try_from(index).ok()?;
            if argument.access == checked_trees::CheckedStructuralAccess::Owned
                || custody.is_some_and(|custody| {
                    custody.retained_reference_inputs.contains(&index)
                        && !argument
                            .call_projection()
                            .contains(&checked_trees::CheckedUnitStructuralPathSegment::Referent)
                })
            {
                continue;
            }
            let ordinal = argument.source_structural_result_binding_ordinal();
            let Some(binding) = preparation
                .iter()
                .filter_map(CheckedUnitEffectOperationPlan::structural_result_binding)
                .find(|binding| Some(binding.binding_ordinal) == ordinal)
            else {
                continue;
            };
            if binding.local.symbol.is_valid() || binding.multiplicity != Multiplicity::Affine {
                continue;
            }
            if custody.is_some_and(|custody| {
                custody.retained_reference_inputs.iter().any(|index| {
                    arguments.get(*index as usize).is_some_and(|source| {
                        source.source_structural_result_binding_ordinal()
                            == Some(binding.binding_ordinal)
                            && !source.call_projection().contains(
                                &checked_trees::CheckedUnitStructuralPathSegment::Referent,
                            )
                    })
                })
            }) {
                continue;
            }
            let declaration = shapes.types.get(&binding.type_identity)?;
            if !declaration.descriptor_cleanup {
                continue;
            }
            let source = checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: binding.binding_ordinal,
            };
            let loans = super::super::control::statement_sequence::captured_reference_result(
                program,
                facts,
                machine,
                state,
                results,
                preparation
                    .iter()
                    .find(|operation| operation.structural_result_binding() == Some(binding))?,
                preparation,
            )
            .map(|captured| captured.loans)
            .unwrap_or_default();
            if completions.iter().any(|(_, completion)| matches!(completion,
                CheckedUnitEffectOperationPlan::ReleaseReference { source: prior, .. } if *prior == source)) { continue; }
            completions.push((
                binding.binding_ordinal,
                CheckedUnitEffectOperationPlan::ReleaseReference {
                    statement_index: binding.statement_index,
                    source,
                    path: Vec::new(),
                    select_evaluated_storage: false,
                    defer_to_exit: false,
                    loans,
                    selection_complements: Vec::new(),
                    owner_cleanups: arena::HandleSpan::empty(),
                    case_conditions: Default::default(),
                },
            ));
        }
        completions.sort_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
        Some(
            completions
                .into_iter()
                .map(|(_, completion)| completion)
                .collect(),
        )
    }
}
pub(crate) use temporary_actions::finalize_completion_actions;

pub(crate) use checked_trees::is_bounded_nominal_cleanup_record;
pub(crate) use cleanup_evidence::machine_has_content_evidence;
pub(crate) use nominal_requirements::{
    checked_contract_expressions, checked_requires_expressions, direct_boolean_field_requirement,
    nominal_cleanup_boolean_requirements, nominal_cleanup_caller_boolean_requirements,
    nominal_cleanup_missing_requirement, nominal_cleanup_missing_requirement_diagnostic,
};
pub(crate) use partial_affine_cleanup::partial_affine_residuals;

use super::{
    CheckFacts, CheckedNominalAffineUnitCleanupMachinePlan, CheckedStructuralAccess,
    CheckedUnitEffectOperationPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment, Diagnostic,
    MachineSupplyMode, Multiplicity, PermissionEventSource, TypeReferenceNode,
};
use crate::execution::terminal_unit::calls::{entry_claims, free_structural_scalar_signature};
use crate::execution::terminal_unit::types::{
    ShapeCollector, is_unit, machine_binders, state_flow,
};
use checked_trees::CheckedUnitCallableEntry;

/*
    A composed graph's entry machine is not the owner of every state it
    contains: nested-machine bodies are keyed to the machine that declares
    their state. Contextual dispositions and storage establishments are
    recorded under that owning machine, never under the graph's entry machine.
*/
pub(in crate::execution::terminal_unit) fn owning_cleanup_machine(
    program: &dyn ::validation::CheckingInput,
    state: symbols::SymbolHandle,
) -> symbols::SymbolHandle {
    let mut scope = state;
    while scope.is_valid() {
        let parent = program.symbols.get(scope).parent;
        if parent.is_valid() && program.symbols.get(parent).kind == symbols::SymbolKind::Machine {
            return parent;
        }
        scope = parent;
    }
    state
}

/*
    Construction may temporarily retain both body descriptions of one entry.
    They must agree on the signature; ordinary overlap resolution and closure
    pruning still choose exactly one complete body before publication.
*/
fn callable_target<'plan>(
    entries: &[CheckedUnitCallableEntry<'plan>],
    machine: symbols::SymbolHandle,
) -> Option<CheckedUnitCallableEntry<'plan>> {
    let mut matches = entries.iter().filter(|entry| entry.machine == machine);
    let entry = *matches.next()?;
    matches
        .all(|candidate| *candidate == entry)
        .then_some(entry)
}

/*
    `plans` borrows the published ordinary and composed callable entries.
    Exact hook identity and caller prerequisites are retained as metadata;
    execution continues to use the source body's shared normal-exit owner.
*/
pub(super) fn build_nominal_affine_unit_cleanup_machine(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    plans: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedNominalAffineUnitCleanupMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if !program.machine_type_parameters(machine).is_empty()
        || !program.machine_owned_data(machine).is_empty()
        || !program.machine_trait_conformances(machine).is_empty()
        || !machine.conformance_bounds.is_empty()
        || !program.machine_invokes(machine).is_empty()
        || machine.suspends
        || machine.blocks
        || !is_unit(program, state.return_type)
        || !crate::flow::state_statements(program, state.statement_nodes).is_empty()
    {
        return None;
    }

    let binders = machine_binders(program, machine);
    let (structural_parameters, scalar) =
        free_structural_scalar_signature(program, shapes, state, &binders)?;
    if !scalar.is_empty() {
        return None;
    }
    let source_parameters = program.state_parameters(state);
    if source_parameters.is_empty() || source_parameters.len() != structural_parameters.len() {
        return None;
    }
    for (position, (source_parameter, checked_parameter)) in source_parameters
        .iter()
        .zip(&structural_parameters)
        .enumerate()
    {
        let parameter_data = program.closed_data_definition(source_parameter.type_reference)?;
        let parameter_shape = shapes.types.get(&checked_parameter.type_identity)?;
        if (source_parameter.is_const)
            || source_parameter.is_mutable
            || usize::try_from(checked_parameter.position).ok()? != position
            || !matches!(
                checked_parameter.multiplicity,
                Multiplicity::Affine | Multiplicity::Linear
            )
            || !checked_parameter.qualifications.is_empty()
            || !program.data_type_parameters(parameter_data).is_empty()
            || !is_bounded_nominal_cleanup_record(&parameter_shape.shape)
        {
            return None;
        }
    }

    let entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        program.state_parameters(state),
    )?;
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    if facts
        .qualifications
        .for_machine(machine.symbol)
        .is_some_and(|fact| !fact.body_committed.is_empty())
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
        || !crate::flow::call_flows(&facts.flow.control, state_flow.calls).is_empty()
        || !source_parameters
            .iter()
            .enumerate()
            .all(|(index, parameter)| {
                cleanup_evidence::has_exact_root_nominal_disposition(
                    facts,
                    machine,
                    state,
                    parameter,
                    structural_parameters[index].multiplicity,
                    index as u32,
                    &entry_claims,
                )
            })
    {
        return None;
    }
    let caller_requirements = nominal_cleanup_caller_boolean_requirements(
        program,
        facts,
        machine,
        state,
        source_parameters,
    )?;
    for (source_parameter, checked_parameter) in
        source_parameters.iter().zip(&structural_parameters).rev()
    {
        let (cleanup_machine, cleanup_state, cleanup_receiver) =
            nominal_cleanup_shape(program, source_parameter.type_reference)?;
        let cleanup_target = callable_target(plans, cleanup_machine.symbol)?;
        if !nominal_cleanup_target_matches(
            program,
            cleanup_machine,
            cleanup_state,
            &cleanup_target,
            checked_parameter.type_identity,
        ) {
            return None;
        }
        let cleanup_requirements = nominal_cleanup_boolean_requirements(
            program,
            facts,
            cleanup_machine,
            cleanup_state,
            cleanup_receiver,
        )?;
        if let Some(missing) = nominal_cleanup_missing_requirement(
            checked_parameter.position,
            &caller_requirements,
            &cleanup_requirements,
        ) {
            diagnostics.push(nominal_cleanup_missing_requirement_diagnostic(
                program,
                machine,
                state,
                source_parameter,
                cleanup_machine,
                missing,
            ));
            return None;
        }
    }

    Some(CheckedNominalAffineUnitCleanupMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        caller_requirements,
    })
}

/*
    The cleanup hook `dropped by` selects for a parameter's data type and its
    sole explicit parameter: the closed carrier owns exactly one designated
    checked-body cleanup machine. Regions do not alter its layout or replace
    the hook identity; type arguments must already have a closed definition.
*/
pub(crate) fn nominal_cleanup_shape(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<(
    &typed_trees::machine::Machine,
    &typed_trees::state::State,
    &typed_trees::signature::StateParameter,
)> {
    let cleanup_machine =
        validation::nominal_cleanup_machine_for_type(program, type_reference, &[])?;
    if cleanup_machine.supply_mode != MachineSupplyMode::CheckedBody {
        return None;
    }
    let cleanup_state = program.machine_states(cleanup_machine).first()?;
    let [cleanup_receiver] = program.state_parameters(cleanup_state) else {
        return None;
    };
    Some((cleanup_machine, cleanup_state, cleanup_receiver))
}

/*
    The selected owner hook is an ordinary Unit callable, whether its body is
    a sequence or a state graph. Source admission checks finite normal completion
    and propagates its actual reach; retaining a borrowed subject or effectful
    body does not create a different cleanup operation.
*/
pub(crate) fn nominal_cleanup_target_matches(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    plan: &CheckedUnitCallableEntry<'_>,
    type_identity: typed_trees::type_identity::TypeIdentityId,
) -> bool {
    let [receiver] = program.state_parameters(state) else {
        return false;
    };
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = program
        .type_reference_table
        .type_reference(receiver.type_reference)
    else {
        return false;
    };
    access.is_readable()
        && access.is_exclusive()
        && program.normalized_type_identity(*referee) == type_identity
        && !receiver.is_const
        && program.machine_type_parameters(machine).is_empty()
        && program.machine_owned_data(machine).is_empty()
        && program.machine_trait_conformances(machine).is_empty()
        && machine.conformance_bounds.is_empty()
        && program.machine_invokes(machine).is_empty()
        && !machine.suspends
        && !machine.blocks
        && is_unit(program, state.return_type)
        && plan.returns_unit
        && plan.machine == machine.symbol
        && plan.state == state.symbol
        && plan.scalar_parameters.is_empty()
        && plan.structural_parameters.len() <= 1
        && plan.structural_parameters.iter().all(|parameter| {
            parameter.position == 0
                && matches!(
                    parameter.access,
                    CheckedStructuralAccess::MutableBorrow
                        | CheckedStructuralAccess::WriteOnlyBorrow
                )
                && parameter.type_identity == type_identity
        })
}
