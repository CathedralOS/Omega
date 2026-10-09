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

pub(super) fn disposition_actions(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    disposition: &checked_trees::ContextualCleanupDisposition,
    source: CheckedUnitStructuralArgumentSourcePlan,
    place_path: Option<&[checked_trees::CheckedUnitStructuralPathSegment]>,
) -> Option<Vec<CheckedNormalCleanupAction>> {
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
    let selected = crate::flow::selected_cleanup_leaves(program, facts, disposition)?;
    let contexts = facts
        .flow
        .control
        .states
        .iter()
        .find(|(_, state)| {
            state.machine_symbol == disposition.machine && state.state_symbol == disposition.state
        })
        .and_then(|(_, state)| {
            crate::facts::contextual_cleanup::occurrence_contexts(
                program,
                facts,
                state,
                disposition,
            )
        });
    let case_reachability = |segments: &[facts::PlaceSegment]| {
        use validation::CleanupCaseReachability;
        if !segments
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
        {
            return Some(CleanupCaseReachability::Selected);
        }
        let contexts = contexts.as_ref()?;
        if contexts.is_empty() {
            return None;
        }
        let subject = validation::CanonicalPlace {
            root: disposition.root,
            segments: segments.to_vec(),
        };
        if contexts.iter().all(|(statement, contexts)| {
            crate::flow::place_case_path_is_inactive(
                program,
                &facts.semantic,
                contexts,
                disposition.machine,
                disposition.state,
                *statement,
                &subject,
            )
        }) {
            return Some(CleanupCaseReachability::Inactive);
        }
        let selected = contexts.iter().all(|(statement, contexts)| {
            crate::flow::place_cases_are_selected(
                program,
                &facts.semantic,
                contexts,
                disposition.machine,
                disposition.state,
                *statement,
                &subject,
            )
        });
        Some(if selected {
            CleanupCaseReachability::Selected
        } else {
            CleanupCaseReachability::Runtime
        })
    };
    let place = validation::CanonicalPlace {
        root: disposition.root,
        segments: facts
            .flow
            .ownership
            .segments
            .span(disposition.path)?
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
        )?
    };
    let type_identity = shapes.add_type(reference, &binders, &[])?;
    // A caller that already holds the dying place in its own operand
    // namespace supplies it: a runtime-selected element has no static
    // argument path, but its store names the same place.
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
        )?
        .1
    };
    let mut moved = Vec::new();
    for removed in facts
        .contextual_cleanup
        .exclusions
        .span(disposition.exclusions)?
    {
        let excluded = facts.contextual_cleanup.segments.span(removed.segments)?;
        match case_reachability(excluded)? {
            validation::CleanupCaseReachability::Inactive => continue,
            validation::CleanupCaseReachability::Selected => {}
            // An uncertain earlier transfer needs its own selected custody
            // join. A tag condition alone cannot establish that transfer.
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
        let (reference, relative_path) =
            crate::execution::terminal_unit::calls::projected_argument_path(
                program,
                disposition.state,
                statement_index,
                &validation::CanonicalPlace {
                    root: facts::PlaceRoot::TypeReference(reference),
                    segments: relative_segments.to_vec(),
                },
            )?;
        moved.push((relative_path, shapes.add_type(reference, &binders, &[])?));
    }
    let mut hooks = Vec::new();
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
                )?;
            let selected_type = shapes.add_type(selected_reference, &binders, &[])?;
            let mut selected_path = path.clone();
            selected_path.extend(relative_path);
            hooks.push(selected_nominal(
                program,
                targets,
                hook,
                source.clone(),
                selected_path,
                selected_type,
            )?);
        }
    }
    // Geometry chooses the live complement, not whether it is cleanup-free.
    // Hook receivers are intact disposition boundaries. A parent's hook is
    // followed by structural children, not substituted for them. Refine only
    // the paths that reach hooks, preserving maximal ordinary residual
    // subtrees and the same reverse declaration/index order for every kind.
    let residuals = if moved.is_empty() && hooks.is_empty() {
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
            .map(|(path, identity)| (path.as_slice(), identity.as_str()))
            .collect::<Vec<_>>();
        let boundaries = hooks
            .iter()
            .map(|hook| {
                Some((
                    hook.path.strip_prefix(path.as_slice())?,
                    hook.type_identity.as_str(),
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        let types = shapes
            .types
            .iter()
            .map(|(identity, plan)| (identity.as_str(), plan))
            .collect();
        validation::reconstruct_case_selected_regions(
            &types,
            &source,
            &type_identity,
            &moved,
            &boundaries,
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
    if selected_hooks != hooks.len() {
        return None;
    }
    Some(actions)
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
    selected_case: &str,
) -> Option<Vec<facts::PlaceSegment>> {
    use checked_trees::CheckedUnitStructuralPathSegment;
    use typed_trees::data::DataMember;

    let mut place = validation::CanonicalPlace {
        root: facts::PlaceRoot::TypeReference(root_type),
        segments: Vec::new(),
    };
    let mut payload_fields = None;
    for segment in path
        .iter()
        .chain(std::iter::once(&CheckedUnitStructuralPathSegment::Case(
            selected_case.to_owned(),
        )))
    {
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
                    fields
                        .iter()
                        .find(|field| field.path_identity() == *identity)?
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
                            DataMember::Field(field) if field.path_identity() == *identity => {
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
                        DataMember::Variant(case) if case.path_identity() == *identity => {
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
    type_identity: String,
) -> Option<CheckedUnitNominalAffineCleanupPlan> {
    let machine = program.machine_by_symbol(hook)?;
    let state = program.machine_states(machine).first()?;
    let target = super::callable_target(targets, hook)?;
    if !super::nominal_cleanup_target_matches(program, machine, state, &target, &type_identity) {
        return None;
    }
    Some(CheckedUnitNominalAffineCleanupPlan {
        source,
        path,
        type_identity,
        cleanup_machine: hook,
        cleanup_state: state.symbol,
        cleanup_contract_report_fingerprint: target.contract_report_fingerprint,
        case_conditions: Default::default(),
    })
}
