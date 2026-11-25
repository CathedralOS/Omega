use super::engine::{DescriptorReadDepth, Replay, State};
use super::*;
use crate::descriptor_lifetimes::DescriptorFlowSource;
use semantic_vocabulary::StructuralCaseId;
use std::sync::Arc;

pub(crate) mod values;
pub(crate) use values::{Storage, Transfer, contains_case};
pub(crate) mod result;
pub(crate) use result::ResultCases;

#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct CurrentCases {
    facts: LocationMap<BTreeSet<StructuralCaseId>>,
    copies: values::Copies,
}

pub(crate) fn validate_graph(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
) -> Option<Vec<DescriptorPosition>> {
    machine.validate_invocation_owners()?;
    let order = crate::control_graph::finished_order(machine.entry, |position, index| {
        machine
            .flow
            .get(&position)?
            .successors
            .get(index)
            .map(|edge| edge.target)
    });
    if order.len() != machine.flow.len()
        || order.iter().any(|point| !machine.flow.contains_key(point))
    {
        return None;
    }
    for (position, predicates) in &machine.case_constructors {
        machine.flow.get(position)?;
        for predicate in predicates {
            valid_subject(replay, machine, predicate)?;
            predicate.subject.exact_location()?;
        }
    }
    for (position, predicates) in &machine.case_successors {
        if predicates.len() != machine.flow.get(position)?.successors.len() {
            return None;
        }
        for (predicate, _) in predicates {
            valid_subject(replay, machine, predicate)?;
        }
    }
    Some(order)
}

fn valid_subject(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    predicate: &DescriptorCasePredicate,
) -> Option<StructuralTypeId> {
    let ty = match &predicate.subject {
        DescriptorFlowSource::Location(location) => {
            machine.location_type_indexed(&replay.type_index, location, None)?
        }
        DescriptorFlowSource::Contents(query) => {
            let root = *machine.place_types.get(&query.root)?;
            query.selected_type_using(replay.types, root, |carrier, path, original| {
                if original {
                    Some(machine.location_type_indexed(
                        &replay.type_index,
                        &DescriptorLocation {
                            root: query.root,
                            path: path.to_vec(),
                        },
                        None,
                    ))
                } else {
                    replay
                        .type_index
                        .resolved_tip(carrier, path, |_, _, _| None)
                }
            })??
        }
        DescriptorFlowSource::Unavailable(_) => return None,
    };
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &replay.type_index.find(ty)?.shape
    else {
        return None;
    };
    terminal_psi::find_structural_case(cases, predicate.case).map(|_| ty)
}

pub(crate) fn establish(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    state: &mut State,
    predicate: &DescriptorCasePredicate,
) -> Option<()> {
    valid_subject(replay, machine, predicate)?;
    let storage =
        replay.canonical_storage(machine, state, predicate.subject.exact_location()?, None)?;
    let subjects = state
        .cases
        .copies
        .equivalents(&storage, machine.flow.len().saturating_mul(4096).max(4096))?;
    let cases = Arc::make_mut(&mut state.cases);
    for subject in subjects {
        cases
            .facts
            .insert(subject, BTreeSet::from([predicate.case]));
    }
    Some(())
}

enum Selection {
    Unknown,
    Exact {
        subjects: BTreeSet<DescriptorLocation>,
        possible: Vec<StructuralCaseId>,
    },
}

fn selection(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    state: &State,
    predicate: &DescriptorCasePredicate,
    truth: bool,
) -> Option<Selection> {
    let ty = valid_subject(replay, machine, predicate)?;
    // A nonsingular query validates the selected carrier but cannot name
    // one cell whose case can be narrowed. Keep every branch feasible.
    let Some(subject) = predicate.subject.exact_location() else {
        return Some(Selection::Unknown);
    };
    let Some(storage) = replay.canonical_storage(machine, state, subject, None) else {
        return Some(Selection::Unknown);
    };
    let subjects = state
        .cases
        .copies
        .equivalents(&storage, machine.flow.len().saturating_mul(4096).max(4096))?;
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &replay.type_index.find(ty)?.shape
    else {
        return None;
    };
    let mut possible = cases
        .iter()
        .map(|case| case.id)
        .filter(|case| (*case == predicate.case) == truth)
        .collect::<Vec<_>>();
    possible.sort_unstable();
    possible.dedup();
    for subject in &subjects {
        if let Some(known) = state.cases.facts.get(subject) {
            possible.retain(|case| known.contains(case));
        }
    }
    Some(Selection::Exact { subjects, possible })
}

pub(crate) fn select(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    state: &mut State,
    predicate: &DescriptorCasePredicate,
    truth: bool,
) -> Option<bool> {
    let Selection::Exact { subjects, possible } =
        selection(replay, machine, state, predicate, truth)?
    else {
        return Some(true);
    };
    if possible.is_empty() {
        return Some(false);
    }
    let possible = possible.into_iter().collect::<BTreeSet<_>>();
    let cases = Arc::make_mut(&mut state.cases);
    for subject in subjects {
        cases.facts.insert(subject, possible.clone());
    }
    Some(true)
}

pub(crate) fn established_case(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    state: &State,
    position: DescriptorPosition,
    successor: Option<(usize, semantic_vocabulary::EdgeId)>,
    subject: &DescriptorLocation,
) -> Option<StructuralCaseId> {
    let storage = replay.canonical_storage(machine, state, subject, None)?;
    fn unique(mut cases: impl Iterator<Item = StructuralCaseId>) -> Option<StructuralCaseId> {
        let first = cases.next()?;
        cases.next().is_none().then_some(first)
    }
    if let Some((ordinal, edge)) = successor {
        if machine.flow.get(&position)?.successors.get(ordinal)?.edge != Some(edge) {
            return None;
        }
        if let Some((predicate, truth)) = machine
            .case_successors
            .get(&position)
            .and_then(|rows| rows.get(ordinal))
        {
            let Selection::Exact { subjects, possible } =
                selection(replay, machine, state, predicate, *truth)?
            else {
                return None;
            };
            if subjects.contains(&storage) {
                return unique(possible.into_iter());
            }
        }
    }
    unique(state.cases.facts.get(&storage)?.iter().copied())
}

pub(crate) fn membership(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    state: &State,
    predicate: &DescriptorCasePredicate,
) -> FlowFeasibility {
    if valid_subject(replay, machine, predicate).is_none() {
        return FlowFeasibility::Unknown;
    }
    let Some(subject) = predicate.subject.exact_location() else {
        return FlowFeasibility::Unknown;
    };
    let Some(storage) = replay.canonical_storage(machine, state, subject, None) else {
        return FlowFeasibility::Unknown;
    };
    match state.cases.facts.get(&storage) {
        Some(cases) if !cases.contains(&predicate.case) => FlowFeasibility::ProvedImpossible,
        Some(_) => FlowFeasibility::Possible,
        None => FlowFeasibility::Unknown,
    }
}

pub(crate) fn established(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    state: &State,
    predicate: &DescriptorCasePredicate,
) -> bool {
    if valid_subject(replay, machine, predicate).is_none() {
        return false;
    }
    let Some(subject) = predicate.subject.exact_location() else {
        return false;
    };
    let Some(storage) = replay.canonical_storage(machine, state, subject, None) else {
        return false;
    };
    state
        .cases
        .facts
        .get(&storage)
        .is_some_and(|cases| cases.len() == 1 && cases.contains(&predicate.case))
}

pub(crate) fn written_queries(
    machine: &DescriptorMachine,
    position: DescriptorPosition,
) -> Option<Vec<&DescriptorContentsQuery>> {
    let mut queries = Vec::new();
    for query in machine
        .payload_invalidations
        .get(&position)
        .into_iter()
        .flatten()
    {
        queries.push(query.as_ref()?);
    }
    for transfer in machine
        .contents_transfers
        .get(&position)
        .into_iter()
        .flatten()
    {
        queries.push(&transfer.destination);
        if transfer.removes_source {
            queries.push(&transfer.source);
        }
    }
    Some(queries)
}

fn written_locations(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    before: &State,
    definitions: &[DescriptorLocation],
    position: Option<DescriptorPosition>,
    reads: &DescriptorReadDepth<'_>,
) -> Option<Vec<DescriptorLocation>> {
    let mut writes = definitions.to_vec();
    let Some(position) = position else {
        return Some(writes);
    };
    for query in written_queries(machine, position)? {
        let targets = replay.contents_targets_with_depth(machine, before, query, reads)?;
        if targets.is_empty() {
            return None;
        }
        for target in targets {
            let DescriptorPayloadTarget::Storage {
                location,
                completed: None,
            } = target
            else {
                return None;
            };
            writes.push(location);
        }
    }
    Some(writes)
}

fn survives(
    storage: &Storage,
    writes: Option<&[DescriptorLocation]>,
    subject: &DescriptorLocation,
) -> bool {
    let Some(writes) = writes else {
        return false;
    };
    writes.is_empty()
        || (storage.contains(subject)
            && writes
                .iter()
                .all(|write| storage.contains(write) && !values::overlaps(write, subject)))
}

pub(crate) fn transport<'locations>(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    states: (&State, &State),
    definitions: Option<&[DescriptorLocation]>,
    derivations: impl Iterator<
        Item = (
            &'locations DescriptorLocation,
            &'locations DescriptorLocation,
        ),
    >,
    transfer: Transfer<'_>,
    storage: &Storage,
) -> Option<Arc<CurrentCases>> {
    let (before, base) = states;
    let reads = transfer.reads();
    let position = transfer.position();
    let writes = definitions
        .and_then(|known| written_locations(replay, machine, before, known, position, reads));
    let mut derivations = derivations.peekable();
    if derivations.peek().is_none() && writes.as_ref().is_some_and(Vec::is_empty) {
        return Some(base.cases.clone());
    }
    let keep = |subject: &DescriptorLocation| survives(storage, writes.as_deref(), subject);
    let mut cases = (*base.cases).clone();
    cases.facts.retain(|subject, _| keep(subject));
    cases.copies.retain(keep);
    let derivations = derivations.collect::<Vec<_>>();
    let mut assigned = BTreeMap::<DescriptorLocation, BTreeSet<StructuralCaseId>>::new();
    let mut copies = Vec::new();
    for (source, destination) in &derivations {
        if !transfer.transports_value(machine, source, destination) {
            continue;
        }
        let Some(actual_source) = replay.canonical_storage(machine, before, source, None)
        else {
            continue;
        };
        let actual_destination = match position {
            Some(_) => replay.canonical_storage(machine, before, destination, None),
            None => Some((*destination).clone()),
        };
        let Some(actual_destination) = actual_destination else {
            continue;
        };
        for (subject, active) in &before.cases.facts {
            if let Some(target) = values::transported_subject(
                replay,
                machine,
                subject,
                &actual_source,
                &actual_destination,
            ) {
                assigned.entry(target).or_default().extend(active);
            }
        }
        let distinct_assignment =
            derivations.iter().all(|(other_source, other_destination)| {
                (*other_source == *source && *other_destination == *destination)
                    || !values::overlaps(other_destination, destination)
            });
        let source_home = DescriptorLocation::whole(actual_source.root);
        let established_source = before.slots.get(&source_home).is_some_and(|value| {
            value.as_ref().is_some_and(|origins| {
                !origins.is_empty()
                    && origins.iter().all(|origin| {
                        matches!(origin,
                    super::engine::Origin::Storage(home) if home == &source_home)
                    })
            })
        });
        if distinct_assignment
            && established_source
            && transfer.copies_value(machine, source, destination)
            && storage.copy_type(replay, machine, &actual_source, &actual_destination)
        {
            copies.push((
                before
                    .cases
                    .copies
                    .equivalents(&actual_source, storage.limit)?,
                actual_destination,
            ));
        }
    }
    cases.facts.extend(assigned);
    let mut captured = BTreeMap::<
        DescriptorLocation,
        (BTreeSet<DescriptorLocation>, BTreeSet<DescriptorLocation>),
    >::new();
    for (sources, destination) in copies {
        let representative = sources.first()?.clone();
        let (old_members, receivers) = captured.entry(representative).or_default();
        old_members.extend(sources);
        receivers.insert(destination);
    }
    for (_, (old_members, receivers)) in captured {
        let members = old_members
            .into_iter()
            .filter(|source| Arc::ptr_eq(&before.cases, &base.cases) && keep(source))
            .chain(receivers)
            .collect();
        cases.copies.insert_class(members, storage.limit)?;
    }
    Some(if cases == *base.cases {
        base.cases.clone()
    } else {
        Arc::new(cases)
    })
}

pub(crate) fn retire_location(state: &mut State, written: &DescriptorLocation) {
    let cases = Arc::make_mut(&mut state.cases);
    cases.facts.retain(|subject, _| !written.contains(subject));
    cases.copies.retain(|subject| !written.contains(subject));
}

pub(crate) fn retire_query(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    before: &State,
    after: &mut State,
    query: &DescriptorContentsQuery,
    storage: &Storage,
    reads: &DescriptorReadDepth<'_>,
) {
    let writes = replay
        .contents_targets_with_depth(machine, before, query, reads)
        .and_then(|targets| {
            if targets.is_empty() {
                return None;
            }
            targets
                .into_iter()
                .map(|target| match target {
                    DescriptorPayloadTarget::Storage {
                        location,
                        completed: None,
                    } => Some(location),
                    DescriptorPayloadTarget::Storage {
                        completed: Some(_), ..
                    } => None,
                    DescriptorPayloadTarget::Incoming(_) => None,
                })
                .collect::<Option<Vec<_>>>()
        });
    let cases = Arc::make_mut(&mut after.cases);
    cases
        .facts
        .retain(|subject, _| survives(storage, writes.as_deref(), subject));
    cases
        .copies
        .retain(|subject| survives(storage, writes.as_deref(), subject));
}

pub(crate) fn join(left: &CurrentCases, right: &CurrentCases) -> CurrentCases {
    CurrentCases {
        facts: left
            .facts
            .iter()
            .filter_map(|(subject, cases)| {
                let mut cases = cases.clone();
                cases.extend(right.facts.get(subject)?);
                Some((subject.clone(), cases))
            })
            .collect(),
        copies: left.copies.meet(&right.copies),
    }
}
