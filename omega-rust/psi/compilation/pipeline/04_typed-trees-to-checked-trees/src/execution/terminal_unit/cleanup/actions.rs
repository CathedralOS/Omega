//! Executable action selection from an already checked contextual disposition.
//! State exits and temporary call continuations share this geometry and hook
//! selection. Storage namespaces differ; cleanup order and prerequisites do not.
//! A receiver's nominal hook precedes its structural children; nested hook
//! paths are ordered obligations, not overlapping attempts to consume a root.
//! Translate descendants relative to the disposed value's type, then attach
//! the retained receiver path. Re-resolving a runtime-selected prefix would
//! either decline an executable destination or observe it again after the RHS.
//! Effect discovery includes every possible case. Executable regions instead
//! consume the exact disposition's edge facts: an inactive payload has neither
//! a moved region nor a hook. An unknown tag retains a runtime selection
//! condition; it never becomes static authority for every possible payload.

use crate::execution::terminal_unit::types::{ShapeCollector, machine_binders};
use checked_trees::{
    CheckFacts, CheckedNormalCleanupAction, CheckedUnitCallableEntry,
    CheckedUnitNominalAffineCleanupPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};

/// The surrounding dying-owner selection, not a selected subtree path, owns
/// home retirement. Resolve that original home independently of each disposed
/// child's type, including a partially constructed owner with no whole value.
pub(super) fn home_retirement(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    disposition: &checked_trees::ContextualCleanupDisposition,
    source: CheckedUnitStructuralArgumentSourcePlan,
) -> Option<CheckedNormalCleanupAction> {
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
    Some(CheckedNormalCleanupAction::RetireHome {
        source,
        type_identity,
    })
}

/// Keep retirement after all of this home's region actions. A whole discard
/// already consumes its binding; do not publish a second retirement for it.
pub(super) fn finish_home(
    actions: &mut Vec<CheckedNormalCleanupAction>,
    retirement: CheckedNormalCleanupAction,
) {
    let source = retirement.source();
    if actions.iter().any(|action| {
        action.source() == source
            && matches!(action, CheckedNormalCleanupAction::Discard(discard) if discard.path.is_empty() && discard.case_conditions.is_empty())
    }) {
        return;
    }
    let mut selected = actions.iter().filter(|action| action.source() == source);
    if matches!(selected.next(), Some(CheckedNormalCleanupAction::InvokeNominal(cleanup)) if cleanup.path.is_empty() && cleanup.case_conditions.is_empty())
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

/// Keep the exact candidate region beside its ordered actions. The source
/// occurrence supplies permission; the emitted frontier later selects liveness.
pub(super) fn selected_region(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    handle: arena::Handle<checked_trees::ContextualCleanupDisposition>,
    source: CheckedUnitStructuralArgumentSourcePlan,
    captured_path: Option<&[checked_trees::CheckedUnitStructuralPathSegment]>,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let mut phase = "cleanup disposition entry";
    let selected = (|| {
        let disposition = facts.contextual_cleanup.dispositions.get(handle);
        phase = "disposed owner machine";
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
        phase = "disposed source type";
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
        phase = "typed selected region path";
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
            .1
        };
        phase = "selected region stored shape";
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
    })();
    if selected.is_none() && std::env::var_os("OMEGA_LOCAL_PRODUCTION_TRACE").is_some() {
        trace_disposition_refusal(program, facts, handle, "selected_region", phase);
    }
    selected
}

pub(super) fn disposition_actions(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    disposition_handle: arena::Handle<checked_trees::ContextualCleanupDisposition>,
    source: CheckedUnitStructuralArgumentSourcePlan,
    place_path: Option<&[checked_trees::CheckedUnitStructuralPathSegment]>,
) -> Option<Vec<CheckedNormalCleanupAction>> {
    let mut phase = "cleanup disposition entry";
    let selected = (|| {
        let disposition = facts
            .contextual_cleanup
            .dispositions
            .get(disposition_handle);
        let selected_disposition = facts
            .contextual_cleanup
            .is_selected_disposition(disposition_handle);
        phase = "disposed owner machine";
        let machine = program.machine_by_symbol(disposition.machine)?;
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
        phase = "selected cleanup leaves";
        let selected = crate::flow::selected_cleanup_leaves(program, facts, disposition)?;
        phase = "cleanup case evidence";
        let case_evidence = crate::flow::CleanupCaseEvidence::new(program, facts, disposition)?;
        let case_reachability =
            |segments: &[facts::PlaceSegment]| case_evidence.reachability(segments);
        phase = "disposed path";
        let place = validation::CanonicalPlace {
            root: disposition.root,
            segments: facts
                .flow
                .ownership
                .segments
                .span(disposition.path)?
                .to_vec(),
        };
        phase = "disposed source type";
        let reference = if disposition.disposed_type.is_valid() {
            disposition.disposed_type
        } else {
            validation::canonical_place_type_reference(
                program,
                disposition.state,
                statement_index,
                &place,
            )?
        };
        // A disposed reference is its stored descriptor, not the referee beneath
        // its access shell. Use the same storage identity as fields and receiving
        // slots; ordinary value types retain their existing shape.
        phase = "disposed stored shape";
        let type_identity = shapes.add_stored_type(reference, &binders, &[])?;
        // A caller that already holds the dying place in its own operand
        // namespace supplies it: a runtime-selected element has no static
        // argument path, but its store names the same place.
        phase = "typed disposed region path";
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
            )?
            .1
        };
        let mut moved = Vec::new();
        phase = "excluded region correspondence";
        for removed in facts
            .contextual_cleanup
            .exclusions
            .span(disposition.exclusions)?
        {
            let excluded = facts.contextual_cleanup.segments.span(removed.segments)?;
            phase = "excluded case reachability";
            // The shared selected-leaf query already checked disjointness
            // from exclusions outside this exact dying subtree. Sibling
            // geometry must not ask for an unrelated case observation.
            if !excluded.starts_with(place.segments.as_slice())
                && !place.segments.starts_with(excluded)
            {
                continue;
            }
            match case_reachability(excluded)? {
                validation::CleanupCaseReachability::Inactive => continue,
                validation::CleanupCaseReachability::Selected => {}
                validation::CleanupCaseReachability::Runtime
                    if removed.cause
                        == checked_trees::ContextualCleanupExclusionCause::Unestablished => {}
                // Only a positive, all-path receiving absence can exclude a
                // value without selecting its case. This remains runtime tag
                // geometry, never membership or whole-owner disposal proof.
                // Uncertain moves still need their selected custody join.
                validation::CleanupCaseReachability::Runtime => return None,
            }
            // The contextual row remembers earlier transfers under the original
            // owner. A selected residual ignores siblings outside its subtree;
            // an already removed ancestor cannot authorize another disposition.
            let Some(relative_segments) = excluded.strip_prefix(place.segments.as_slice()) else {
                if place.segments.starts_with(excluded) {
                    return None;
                }
                continue;
            };
            // An allocated home with no established cleanup-bearing region has
            // an exact whole-region exclusion. It owns no value disposal or hook:
            // the surrounding owner selection still publishes RetireHome. Do not
            // encode this empty complement as a partial moved-path reconstruction,
            // whose inputs must be strict nonempty subregions of a live value.
            if relative_segments.is_empty() {
                return (removed.root == disposition.root && selected.is_empty()).then(Vec::new);
            }
            phase = "typed moved region path";
            let (reference, relative_path) =
                crate::execution::terminal_unit::calls::projected_argument_path(
                    program,
                    disposition.state,
                    statement_index,
                    &validation::CanonicalPlace {
                        root: facts::PlaceRoot::TypeReference(reference),
                        segments: relative_segments.to_vec(),
                    },
                    None,
                    None,
                )?;
            moved.push((relative_path, shapes.add_type(reference, &binders, &[])?));
        }
        let mut hooks = Vec::new();
        phase = "selected cleanup leaf kind";
        for leaf in selected {
            let crate::flow::CleanupLeaf::Nominal {
                machine: hook,
                path: selected_path,
            } = leaf
            else {
                return None;
            };
            // The disposed value already fixes its receiver, including any view
            // range in that captured prefix. Expand only array children inside
            // the value; expanding the prefix again would turn one selected view
            // element into several unrelated receivers and lose its occurrence.
            phase = "nominal leaf correspondence";
            let relative_path = selected_path.strip_prefix(place.segments.as_slice())?;
            for relative_segments in crate::flow::concrete_cleanup_paths(relative_path)? {
                let mut segments = place.segments.clone();
                segments.extend_from_slice(&relative_segments);
                if case_reachability(&segments)? == validation::CleanupCaseReachability::Inactive {
                    continue;
                }
                let selected_place = validation::CanonicalPlace {
                    root: facts::PlaceRoot::TypeReference(reference),
                    segments: relative_segments,
                };
                let (selected_reference, relative_path) =
                    crate::execution::terminal_unit::calls::projected_argument_path(
                        program,
                        disposition.state,
                        statement_index,
                        &selected_place,
                        None,
                        None,
                    )?;
                let selected_type = shapes.add_type(selected_reference, &binders, &[])?;
                let mut selected_path = path.clone();
                selected_path.extend(relative_path);
                phase = "nominal cleanup target";
                let mut cleanup = selected_nominal(
                    program,
                    targets,
                    hook,
                    source.clone(),
                    selected_path,
                    selected_type,
                )?;
                phase = "nominal consumed claims";
                cleanup.consumed_claims =
                    consumed_claims(program, facts, disposition, selected_disposition, &segments)?;
                hooks.push(cleanup);
            }
        }
        // A receipt may conditionally move only a child. Keep its exact typed
        // boundary in the normal-death partition; it is neither an unconditional
        // exclusion nor permission to dispose the siblings at the value join.
        let mut partition_paths = Vec::new();
        phase = "selected transfer partition";
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
                let (_, selected_path) =
                    crate::execution::terminal_unit::calls::projected_argument_path(
                        program,
                        disposition.state,
                        statement_index,
                        &validation::CanonicalPlace {
                            root: facts::PlaceRoot::TypeReference(reference),
                            segments: relative.to_vec(),
                        },
                        None,
                        None,
                    )?;
                if !partition_paths.contains(&selected_path) {
                    partition_paths.push(selected_path);
                }
            }
        }
        // Geometry chooses the live complement, not whether it is cleanup-free.
        // Hook receivers are intact disposition boundaries. A parent's hook is
        // followed by structural children, not substituted for them. Refine only
        // the paths that reach hooks, preserving maximal ordinary residual
        // subtrees and the same reverse declaration/index order for every kind.
        phase = "case-selected residual reconstruction";
        let residuals = if moved.is_empty() && hooks.is_empty() && partition_paths.is_empty() {
            vec![validation::CaseSelectedCleanupRegion {
                disposition: CheckedUnitPartialAffineDiscardPlan {
                    source: source.clone(),
                    path: Vec::new(),
                    type_identity: type_identity.clone(),
                    case_conditions: Default::default(),
                },
                conditions: Vec::new(),
            }]
        } else {
            let moved = moved
                .iter()
                .map(|(path, identity)| (path.as_slice(), *identity))
                .collect::<Vec<_>>();
            let boundaries = hooks
                .iter()
                .map(|hook| Some((hook.path.strip_prefix(path.as_slice())?, hook.type_identity)))
                .collect::<Option<Vec<_>>>()?;
            let types = shapes
                .types
                .iter()
                .map(|(identity, plan)| (*identity, plan))
                .collect();
            validation::reconstruct_case_selected_regions(
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
            )?
        };
        // The effect traversal groups repeated array leaves by field. Index
        // their exact paths once; only residual geometry chooses action order.
        hooks.sort_unstable_by(|left, right| left.path.cmp(&right.path));
        let mut selected_hooks = 0;
        let actions = residuals
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
                    CheckedNormalCleanupAction::InvokeNominal(hook)
                } else {
                    CheckedNormalCleanupAction::Discard(residual)
                }
            })
            .collect::<Vec<_>>();
        phase = "complete nominal hook coverage";
        if selected_hooks != hooks.len() {
            return None;
        }
        Some(actions)
    })();
    if selected.is_none() && std::env::var_os("OMEGA_LOCAL_PRODUCTION_TRACE").is_some() {
        trace_disposition_refusal(
            program,
            facts,
            disposition_handle,
            "disposition_actions",
            phase,
        );
    }
    selected
}

/// Join normalized cleanup geometry back to the exact checked source place.
/// Matching declaration identities is required: same-shaped fields or case
/// payloads are not interchangeable evidence namespaces. Geometry generates
/// only fields, cases and fixed indices beneath the retained disposed value;
/// dynamic destination coordinates stay in the caller's captured prefix.
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
                let case = program
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
    source: CheckedUnitStructuralArgumentSourcePlan,
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

/// Keep the source-authorized claim names attached to the exact hook region.
/// The emitter resolves these names in the current invocation; neither type
/// multiplicity nor a later storage lookup may manufacture a consumption.
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

/// Failure-only inventory of the existing selected occurrence. These queries
/// introduce no facts and cannot authorize an action or repair a missing row.
fn trace_disposition_refusal(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    handle: arena::Handle<checked_trees::ContextualCleanupDisposition>,
    owner: &str,
    phase: &str,
) {
    if !facts.contextual_cleanup.dispositions.is_valid(handle) {
        eprintln!("CLEANUP_DISPOSITION_REFUSAL owner={owner} phase={phase} stale={handle:?}");
        return;
    }
    let row = facts.contextual_cleanup.dispositions.get(handle);
    let path = facts.flow.ownership.segments.span(row.path);
    let evidence = crate::flow::CleanupCaseEvidence::new(program, facts, row);
    eprintln!(
        "CLEANUP_DISPOSITION_REFUSAL owner={owner} phase={phase} handle={handle:?} row={row:?} path={path:?} leaves={:?} contexts={:?}",
        crate::flow::selected_cleanup_leaves(program, facts, row),
        evidence.as_ref().map(|evidence| evidence.contexts()),
    );
    if let (Some(path), Some(evidence)) = (path, evidence) {
        if row.disposed_type.is_valid() {
            for claim in validation::linear_claim_frontier(program, row.disposed_type) {
                let mut selected = path.to_vec();
                selected.extend_from_slice(&claim.path);
                eprintln!(
                    "CLEANUP_DISPOSITION_CLAIM path={selected:?} conditional={} reachability={:?}",
                    claim.conditional,
                    evidence.reachability(&selected),
                );
            }
        }
        for removed in facts
            .contextual_cleanup
            .exclusions
            .span_or_empty(row.exclusions)
        {
            eprintln!(
                "CLEANUP_DISPOSITION_EXCLUSION root={:?} path={:?} reachability={:?}",
                removed.root,
                facts.contextual_cleanup.segments.span(removed.segments),
                facts
                    .contextual_cleanup
                    .segments
                    .span(removed.segments)
                    .and_then(|path| evidence.reachability(path)),
            );
        }
    }
}
