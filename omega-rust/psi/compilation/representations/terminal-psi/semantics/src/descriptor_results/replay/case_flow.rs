//! Current tag facts travel with the same storage snapshots as descriptor
//! contents. An exact constructor or selected edge can exclude a case; a
//! missing descriptor, unknown address, or unavailable replay cannot do so.

use super::replay::{Replay, State};
use super::*;
use semantic_vocabulary::StructuralCaseId;
use std::sync::Arc;

pub(super) type CurrentCases = BTreeMap<DescriptorLocation, BTreeSet<StructuralCaseId>>;

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
    Arc::make_mut(&mut state.cases).insert(storage, BTreeSet::from([predicate.case]));
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
    if state
        .cases
        .get(&storage)
        .is_some_and(|cases| !cases.contains(&predicate.case))
    {
        return Some(false);
    }
    if state
        .cases
        .get(&storage)
        .is_some_and(|cases| cases.len() == 1)
    {
        return Some(true);
    }
    Arc::make_mut(&mut state.cases).insert(storage, BTreeSet::from([predicate.case]));
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
    match state.cases.get(&storage) {
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
        .get(&storage)
        .is_some_and(|cases| cases.len() == 1 && cases.contains(&predicate.case))
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
    position: Option<DescriptorPosition>,
) -> Arc<CurrentCases> {
    let mut derivations = derivations.peekable();
    if definitions.is_empty() && derivations.peek().is_none() {
        return before.cases.clone();
    }
    let replaces_storage = position.is_some_and(|position| {
        machine.contents_transfers.contains_key(&position)
            || definitions.iter().any(|definition| {
                machine.payload_producers.get(&position) != Some(&definition.root)
            })
    });
    // A write can reach the same backing through several carrier names. Until
    // that complete alias footprint is known, retire every old tag premise.
    // Exact value derivations below still establish the receiving value's tag.
    let mut cases = if replaces_storage {
        CurrentCases::new()
    } else {
        (*before.cases).clone()
    };
    // A replacement may change any containing tag. Conservatively forget an
    // ancestor even for a payload-only write; never preserve stale membership.
    cases.retain(|subject, _| {
        !definitions
            .iter()
            .any(|definition| definition.contains(subject) || subject.contains(definition))
    });
    let mut assigned = CurrentCases::new();
    for (source, destination) in derivations {
        if position.is_some_and(|position| {
            machine
                .storage_derivations
                .contains(&(position, source.clone(), destination.clone()))
        }) {
            // Taking an address does not copy the referent's tag into the
            // descriptor. Later queries cross that exact captured address.
            continue;
        }
        let Some(source) = replay.canonical_storage(machine, before, source) else {
            continue;
        };
        let destination = if position.is_some() {
            replay.canonical_storage(machine, before, destination)
        } else {
            Some(destination.clone())
        };
        let Some(destination) = destination else {
            continue;
        };
        for (subject, active) in before.cases.iter() {
            if let Some(target) = subject.transported(&source, &destination) {
                assigned.entry(target).or_default().extend(active);
            }
        }
    }
    cases.extend(assigned);
    if cases == *before.cases {
        before.cases.clone()
    } else {
        Arc::new(cases)
    }
}

pub(super) fn join(left: &CurrentCases, right: &CurrentCases) -> CurrentCases {
    // Every arrival must retain a tag constraint; absence means unknown. The
    // allowed cases are the union, not an intersection that invents dead flow.
    left.iter()
        .filter_map(|(subject, cases)| {
            let mut cases = cases.clone();
            cases.extend(right.get(subject)?);
            Some((subject.clone(), cases))
        })
        .collect()
}
