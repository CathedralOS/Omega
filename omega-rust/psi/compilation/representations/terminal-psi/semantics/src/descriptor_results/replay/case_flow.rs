//! Current tag facts travel with the same storage snapshots as descriptor
//! contents. An exact constructor or selected edge can exclude a case; a
//! missing descriptor, unknown address, or unavailable replay cannot do so.

use super::replay::{Replay, State};
use super::*;
use semantic_vocabulary::StructuralCaseId;
use std::sync::Arc;

mod values;
pub(super) use values::{Storage, Transfer};

#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct CurrentCases {
    facts: BTreeMap<DescriptorLocation, BTreeSet<StructuralCaseId>>,
    copies: values::Copies,
}

pub(super) fn validate_graph(replay: &Replay<'_>, machine: &DescriptorMachine) -> Option<()> {
    let mut visited = BTreeSet::new();
    let mut pending = vec![machine.entry];
    while let Some(position) = pending.pop() {
        if !visited.insert(position) {
            continue;
        }
        let step = machine.flow.get(&position)?;
        pending.extend(step.successors.iter().map(|edge| edge.target));
    }
    if visited.len() != machine.flow.len() {
        return None;
    }
    for (position, predicates) in &machine.case_constructors {
        machine.flow.get(position)?;
        for predicate in predicates {
            valid_subject(replay, machine, predicate)?;
        }
    }
    for (position, predicates) in &machine.case_successors {
        if predicates.len() != machine.flow.get(position)?.successors.len() {
            return None;
        }
        for predicate in predicates {
            valid_subject(replay, machine, predicate)?;
        }
    }
    Some(())
}

fn valid_subject(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    predicate: &DescriptorCasePredicate,
) -> Option<()> {
    let ty = crate::descriptor_lifetimes::descriptor_location_type(
        replay.types,
        *machine.place_types.get(&predicate.subject.root)?,
        &predicate.subject.path,
    )?;
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &find_structural_type(replay.types, ty)?.shape
    else {
        return None;
    };
    terminal_psi::find_structural_case(cases, predicate.case).map(|_| ())
}

pub(super) fn establish(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    state: &mut State,
    predicate: &DescriptorCasePredicate,
) -> Option<()> {
    valid_subject(replay, machine, predicate)?;
    let storage = replay.canonical_storage(machine, state, &predicate.subject)?;
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

/// False means a positive contradiction of the current tag and this exact
/// selected edge. Unresolved address transport leaves the edge executable.
pub(super) fn select(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    state: &mut State,
    predicate: &DescriptorCasePredicate,
) -> Option<bool> {
    valid_subject(replay, machine, predicate)?;
    let Some(storage) = replay.canonical_storage(machine, state, &predicate.subject) else {
        return Some(true);
    };
    let subjects = state
        .cases
        .copies
        .equivalents(&storage, machine.flow.len().saturating_mul(4096).max(4096))?;
    if subjects.iter().any(|subject| {
        state
            .cases
            .facts
            .get(subject)
            .is_some_and(|cases| !cases.contains(&predicate.case))
    }) {
        return Some(false);
    }
    let cases = Arc::make_mut(&mut state.cases);
    for subject in subjects {
        cases
            .facts
            .insert(subject, BTreeSet::from([predicate.case]));
    }
    Some(true)
}

pub(super) fn membership(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    state: &State,
    predicate: &DescriptorCasePredicate,
) -> FlowFeasibility {
    if valid_subject(replay, machine, predicate).is_none() {
        return FlowFeasibility::Unknown;
    }
    let Some(storage) = replay.canonical_storage(machine, state, &predicate.subject) else {
        return FlowFeasibility::Unknown;
    };
    match state.cases.facts.get(&storage) {
        Some(cases) if !cases.contains(&predicate.case) => FlowFeasibility::ProvedImpossible,
        Some(_) => FlowFeasibility::Possible,
        None => FlowFeasibility::Unknown,
    }
}

pub(super) fn established(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    state: &State,
    predicate: &DescriptorCasePredicate,
) -> bool {
    if valid_subject(replay, machine, predicate).is_none() {
        return false;
    }
    let Some(storage) = replay.canonical_storage(machine, state, &predicate.subject) else {
        return false;
    };
    state
        .cases
        .facts
        .get(&storage)
        .is_some_and(|cases| cases.len() == 1 && cases.contains(&predicate.case))
}

/// A typed write footprint applies to old values before simultaneous copies
/// are installed. An unresolved candidate invalidates the complete old relation;
/// no known candidate can hide an unknown alias alternative.
fn written_locations(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    before: &State,
    definitions: &[DescriptorLocation],
    position: Option<DescriptorPosition>,
) -> Option<Vec<DescriptorLocation>> {
    let mut writes = definitions.to_vec();
    let Some(position) = position else {
        return Some(writes);
    };
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
    for query in queries {
        let targets = replay.contents_targets(machine, before, query)?;
        if targets.is_empty() {
            return None;
        }
        for target in targets {
            let DescriptorPayloadTarget::Storage(location) = target else {
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

pub(super) fn transport<'locations>(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    before: &State,
    definitions: &[DescriptorLocation],
    derivations: impl Iterator<
        Item = (
            &'locations DescriptorLocation,
            &'locations DescriptorLocation,
        ),
    >,
    transfer: Transfer<'_>,
    storage: &Storage,
) -> Option<Arc<CurrentCases>> {
    let position = transfer.position();
    let writes = written_locations(replay, machine, before, definitions, position);
    let mut derivations = derivations.peekable();
    if derivations.peek().is_none() && writes.as_ref().is_some_and(Vec::is_empty) {
        return Some(before.cases.clone());
    }
    let keep = |subject: &DescriptorLocation| survives(storage, writes.as_deref(), subject);
    let mut cases = (*before.cases).clone();
    cases.facts.retain(|subject, _| keep(subject));
    cases.copies.retain(keep);
    // Assignments read only the pre-operation state. A destination renewed by
    // this step may share the captured value with other new destinations, but
    // never with the overwritten previous value of that same name.
    let derivations = derivations.collect::<Vec<_>>();
    let mut assigned = BTreeMap::<DescriptorLocation, BTreeSet<StructuralCaseId>>::new();
    let mut copies = Vec::new();
    for (source, destination) in &derivations {
        if !transfer.transports_value(machine, source, destination) {
            continue;
        }
        let Some(actual_source) = replay.canonical_storage(machine, before, source) else {
            continue;
        };
        let actual_destination = match position {
            Some(_) => replay.canonical_storage(machine, before, destination),
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
        let distinct_assignment = derivations.iter().all(|(other_source, other_destination)| {
            (*other_source == *source && *other_destination == *destination)
                || !values::overlaps(other_destination, destination)
        });
        let source_home = DescriptorLocation::whole(actual_source.root);
        let established_source = before.slots.get(&source_home).is_some_and(|value| {
            value.as_ref().is_some_and(|origins| {
                !origins.is_empty()
                    && origins.iter().all(|origin| {
                        matches!(origin,
                    super::replay::Origin::Storage(home) if home == &source_home)
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
    // Capture groups refer only to OLD members. A new destination that reuses
    // an old source name must not bridge this assignment to that old value.
    // Equivalent pre-step closures share their least current member; that
    // temporary grouping key is discarded before publishing the new rosters.
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
            .filter(|source| keep(source))
            .chain(receivers)
            .collect();
        cases.copies.insert_class(members, storage.limit)?;
    }
    Some(if cases == *before.cases {
        before.cases.clone()
    } else {
        Arc::new(cases)
    })
}

/// Call effects use the same old-snapshot footprint owner as stores. The call
/// result is independent of these writes; unknown effects retain no case fact.
pub(super) fn retire_query(
    replay: &Replay<'_>,
    machine: &DescriptorMachine,
    before: &State,
    after: &mut State,
    query: &DescriptorContentsQuery,
    storage: &Storage,
) {
    let writes = replay
        .contents_targets(machine, before, query)
        .and_then(|targets| {
            if targets.is_empty() {
                return None;
            }
            targets
                .into_iter()
                .map(|target| match target {
                    DescriptorPayloadTarget::Storage(location) => Some(location),
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

pub(super) fn join(left: &CurrentCases, right: &CurrentCases) -> CurrentCases {
    // Every arrival must retain a tag constraint and a current copy relation.
    // Missing facts or relationships are unknown, including loop renewal.
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
