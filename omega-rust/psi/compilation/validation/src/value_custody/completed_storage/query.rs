//! Demand-driven storage lookup visits each relevant immutable predecessor once.
//! The query-local annotations use full generational identity, independently of
//! the producer's paged allocation. Results name flat Store events, so a shared
//! join tail cannot expand into an exponentially cloned alternatives tree.

use super::{StoredObservation, StoredOperand};
use crate::CanonicalPlace;
use arena::Handle;
use checked_trees::{FlowControlFacts, FlowStorageOperation, FlowStorageSnapshot};
use std::collections::BTreeMap;

type Snapshot = Handle<FlowStorageSnapshot>;

#[derive(Clone)]
enum Resolution {
    Incoming,
    Unavailable,
    Sources {
        incoming: bool,
        stores: Vec<Snapshot>,
    },
}

enum Annotation {
    Active,
    Complete(Resolution),
}

fn identity(snapshot: Snapshot) -> (u32, u32) {
    (snapshot.generation(), snapshot.arena_index())
}

fn resolved(annotations: &BTreeMap<(u32, u32), Annotation>, snapshot: Snapshot) -> Resolution {
    if !snapshot.is_valid() {
        return Resolution::Incoming;
    }
    match annotations.get(&identity(snapshot)) {
        Some(Annotation::Complete(value)) => value.clone(),
        _ => Resolution::Unavailable,
    }
}

fn matches_store(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    source: &CanonicalPlace,
    destination: checked_trees::FlowOperandReferent,
) -> Option<bool> {
    let path = control
        .operand_referent_segments
        .span(destination.segments)?;
    Some(
        destination.root == source.root
            && path.len() <= source.segments.len()
            && path.iter().zip(&source.segments).all(|(left, right)| {
                crate::storage_path_relations::canonical_place_segments_equal(
                    program, *left, *right,
                )
            }),
    )
}

fn preserved_binding_matches(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    source: &CanonicalPlace,
    destination: checked_trees::FlowOperandReferent,
    observation: StoredObservation,
) -> Option<bool> {
    if !matches!(observation, StoredObservation::Binding) {
        return Some(false);
    }
    let path = control
        .operand_referent_segments
        .span(destination.segments)?;
    Some(
        path.len() == source.segments.len()
            && matches_store(program, control, source, destination)?,
    )
}

enum SourceBoundary<'query> {
    Stores,
    ReadCuts(&'query std::collections::BTreeSet<(u32, u32)>),
}

/// Every predecessor must reach an admitted read of these contents. The caller
/// establishes the same typed value at each cut; this query proves only that
/// intervening storage history preserves it. Unknown, missing and unanchored
/// arrivals establish nothing, and later stores cannot become read evidence.
pub fn contents_preserved_from(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    snapshot: Snapshot,
    source: &CanonicalPlace,
    cuts: &[Snapshot],
    observation: StoredObservation,
) -> bool {
    if cuts.is_empty()
        || cuts
            .iter()
            .any(|cut| !control.storage_snapshots.is_valid(*cut))
    {
        return false;
    }
    let cuts = cuts.iter().map(|cut| identity(*cut)).collect();
    matches!(resolve_snapshot(program, control, snapshot, source, observation, SourceBoundary::ReadCuts(&cuts)),
        Some(Resolution::Sources { incoming: false, stores }) if !stores.is_empty())
}

fn resolve_snapshot(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    snapshot: Snapshot,
    source: &CanonicalPlace,
    observation: StoredObservation,
    boundary: SourceBoundary<'_>,
) -> Option<Resolution> {
    if !snapshot.is_valid() {
        return None;
    }
    // Each reachable node is entered/completed once; every predecessor edge
    // contributes at most one additional enter. A malformed graph fails closed
    // within its own representation's size, without recursive host-stack use.
    let Some(mut remaining) = control
        .storage_snapshots
        .len()
        .checked_mul(3)
        .and_then(|nodes| {
            control
                .storage_predecessors
                .len()
                .checked_mul(2)
                .and_then(|edges| nodes.checked_add(edges))
        })
        .and_then(|work| work.checked_add(1))
    else {
        return Some(Resolution::Unavailable);
    };
    let mut annotations = BTreeMap::new();
    let mut pending = vec![(snapshot, false)];
    while let Some((current, complete)) = pending.pop() {
        let Some(next) = remaining.checked_sub(1) else {
            return Some(Resolution::Unavailable);
        };
        remaining = next;
        if !current.is_valid() {
            continue;
        }
        if !control.storage_snapshots.is_valid(current) {
            return Some(Resolution::Unavailable);
        }
        let event = control.storage_snapshots.get(current);
        let key = identity(current);
        if let SourceBoundary::ReadCuts(cuts) = boundary
            && cuts.contains(&key)
        {
            annotations.insert(
                key,
                Annotation::Complete(Resolution::Sources {
                    incoming: false,
                    stores: vec![current],
                }),
            );
            continue;
        }
        if complete {
            let result = match event.operation {
                FlowStorageOperation::Join { predecessors } => {
                    let Some(predecessors) = control.storage_predecessors.span(predecessors) else {
                        return Some(Resolution::Unavailable);
                    };
                    let mut stores = Vec::new();
                    let mut incoming = false;
                    let mut unavailable = predecessors.is_empty();
                    for predecessor in predecessors {
                        match resolved(&annotations, *predecessor) {
                            Resolution::Incoming => incoming = true,
                            Resolution::Unavailable => unavailable = true,
                            Resolution::Sources {
                                incoming: entry,
                                stores: values,
                            } => {
                                incoming |= entry;
                                stores.extend(values);
                            }
                        }
                    }
                    stores.sort_unstable_by_key(|handle| identity(*handle));
                    stores.dedup();
                    if unavailable
                        || (incoming
                            && !stores.is_empty()
                            && !matches!(observation, StoredObservation::Binding))
                    {
                        Resolution::Unavailable
                    } else if stores.is_empty() {
                        Resolution::Incoming
                    } else {
                        Resolution::Sources { incoming, stores }
                    }
                }
                FlowStorageOperation::PreserveBinding {
                    destination,
                    source: entry,
                } if preserved_binding_matches(
                    program,
                    control,
                    source,
                    destination,
                    observation,
                ) == Some(true) =>
                {
                    resolved(&annotations, entry)
                }
                _ => resolved(&annotations, event.previous),
            };
            annotations.insert(key, Annotation::Complete(result));
            continue;
        }
        match annotations.get(&key) {
            Some(Annotation::Complete(_)) => continue,
            Some(Annotation::Active) => return Some(Resolution::Unavailable),
            None => {}
        }
        let terminal = match event.operation {
            FlowStorageOperation::Unknown => Some(Resolution::Unavailable),
            FlowStorageOperation::Store { destination, .. }
                if matches!(boundary, SourceBoundary::ReadCuts(_)) =>
            {
                let path = control
                    .operand_referent_segments
                    .span(destination.segments)?;
                (destination.root == source.root
                    && crate::storage_path_relations::contents_overlap(program, source, path))
                .then_some(Resolution::Unavailable)
            }
            FlowStorageOperation::Store {
                destination, value, ..
            } => match matches_store(program, control, source, destination) {
                None => Some(Resolution::Unavailable),
                Some(false) => None,
                Some(true) if control.operands.is_valid(value) => Some(Resolution::Sources {
                    incoming: false,
                    stores: vec![current],
                }),
                Some(true) => Some(Resolution::Unavailable),
            },
            FlowStorageOperation::Writes {
                bindings,
                contents,
                bindings_complete,
                contents_complete,
            } => {
                let (writes, complete) = match observation {
                    StoredObservation::Binding | StoredObservation::InlineContents => {
                        (bindings, bindings_complete)
                    }
                    StoredObservation::Contents => (contents, contents_complete),
                };
                let stable = complete
                    && control
                        .operand_origin_writes
                        .span(writes)
                        .is_some_and(|writes| {
                            writes.iter().all(|write| {
                                write.root != source.root
                                    || control
                                        .operand_referent_segments
                                        .span(write.segments)
                                        .is_some_and(|segments| !match observation {
                                            StoredObservation::Binding => {
                                                crate::storage_path_relations::binding_overwrite(
                                                    program, source, segments,
                                                )
                                            }
                                            StoredObservation::InlineContents
                                            | StoredObservation::Contents => {
                                                crate::storage_path_relations::contents_overlap(
                                                    program, source, segments,
                                                )
                                            }
                                        })
                            })
                        });
                (!stable).then_some(Resolution::Unavailable)
            }
            FlowStorageOperation::PreserveBinding { destination, .. } => {
                preserved_binding_matches(program, control, source, destination, observation)
                    .is_none()
                    .then_some(Resolution::Unavailable)
            }
            FlowStorageOperation::Join { .. } => None,
        };
        if let Some(result) = terminal {
            if matches!(result, Resolution::Unavailable) {
                trace_query_refusal(control, current, source, observation);
            }
            annotations.insert(key, Annotation::Complete(result));
        } else {
            annotations.insert(key, Annotation::Active);
            pending.push((current, true));
            if let FlowStorageOperation::Join { predecessors } = event.operation {
                let Some(predecessors) = control.storage_predecessors.span(predecessors) else {
                    return Some(Resolution::Unavailable);
                };
                pending.extend(predecessors.iter().rev().map(|handle| (*handle, false)));
            } else if let FlowStorageOperation::PreserveBinding {
                destination,
                source: entry,
            } = event.operation
                && preserved_binding_matches(program, control, source, destination, observation)
                    == Some(true)
            {
                pending.push((entry, false));
            } else {
                pending.push((event.previous, false));
            }
        }
    }
    Some(resolved(&annotations, snapshot))
}

pub fn at_snapshot(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    snapshot: Snapshot,
    source: &CanonicalPlace,
    observation: StoredObservation,
) -> Option<StoredOperand> {
    let resolved = resolve_snapshot(
        program,
        control,
        snapshot,
        source,
        observation,
        SourceBoundary::Stores,
    )?;
    if matches!(resolved, Resolution::Incoming) {
        None
    } else {
        sources_from_resolution(control, snapshot, source, resolved)
    }
}

/// Keep a proved unchanged entry value distinct from an invalid or unavailable
/// cut. Consumers joining conditional stores need every incoming alternative;
/// the older `at_snapshot` query deliberately omits a wholly unchanged entry.
pub fn sources_at_snapshot(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    snapshot: Snapshot,
    source: &CanonicalPlace,
    observation: StoredObservation,
) -> Option<StoredOperand> {
    let resolved = resolve_snapshot(
        program,
        control,
        snapshot,
        source,
        observation,
        SourceBoundary::Stores,
    )?;
    sources_from_resolution(control, snapshot, source, resolved)
}

fn sources_from_resolution(
    control: &FlowControlFacts,
    snapshot: Snapshot,
    source: &CanonicalPlace,
    resolved: Resolution,
) -> Option<StoredOperand> {
    match resolved {
        Resolution::Incoming => Some(StoredOperand::Incoming(source.clone())),
        Resolution::Unavailable => Some(StoredOperand::Unavailable),
        Resolution::Sources { incoming, stores } => {
            let mut values = Vec::new();
            if incoming {
                values.push(StoredOperand::Incoming(source.clone()));
            }
            for handle in stores {
                let FlowStorageOperation::Store {
                    destination,
                    value,
                    projection,
                } = control.storage_snapshots.get(handle).operation
                else {
                    return Some(StoredOperand::Unavailable);
                };
                let Some(path) = control.operand_referent_segments.span(destination.segments)
                else {
                    return Some(StoredOperand::Unavailable);
                };
                let Some(selected) = control.operand_retained_paths.span(projection) else {
                    return Some(StoredOperand::Unavailable);
                };
                let mut projection = selected.to_vec();
                projection.extend_from_slice(&source.segments[path.len()..]);
                if trace_query_enabled(source) {
                    eprintln!(
                        "STORAGE_QUERY selected-store query_snapshot={snapshot:?} store={handle:?} source={source:?} destination_root={:?} destination_path={path:?} value={value:?} type={:?} projection={projection:?}",
                        destination.root,
                        control.operands.get(value).type_reference
                    );
                }
                values.push(StoredOperand::Value {
                    capture: value,
                    reference: control.operands.get(value).type_reference,
                    projection,
                    slot: Some(source.clone()),
                });
            }
            if values.len() == 1 {
                values.pop()
            } else {
                Some(StoredOperand::Alternatives(values))
            }
        }
    }
}

fn trace_query_enabled(source: &CanonicalPlace) -> bool {
    if !crate::trace_flags::storage_query_trace() {
        return false;
    }
    std::env::var("OMEGA_STORAGE_QUERY_TRACE_ROOT")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .is_none_or(|root| {
            source
                .root
                .as_symbol()
                .is_some_and(|symbol| symbol.arena_index() == root)
        })
}

fn trace_query_refusal(
    control: &FlowControlFacts,
    snapshot: Snapshot,
    source: &CanonicalPlace,
    observation: StoredObservation,
) {
    if !trace_query_enabled(source) {
        return;
    }
    let event = control.storage_snapshots.get(snapshot);
    let mode = match observation {
        StoredObservation::Binding => "binding",
        StoredObservation::InlineContents => "inline-contents",
        StoredObservation::Contents => "contents",
    };
    eprintln!(
        "STORAGE_QUERY refused source={source:?} snapshot={snapshot:?} observation={mode} event={event:?}"
    );
    match event.operation {
        FlowStorageOperation::Store {
            destination,
            value,
            projection,
        } => {
            eprintln!(
                "STORAGE_QUERY refused-store destination_root={:?} destination_path={:?} value={value:?} type={:?} projection={:?}",
                destination.root,
                control.operand_referent_segments.span(destination.segments),
                control
                    .operands
                    .is_valid(value)
                    .then(|| control.operands.get(value).type_reference),
                control.operand_retained_paths.span(projection)
            );
        }
        FlowStorageOperation::Writes {
            bindings,
            contents,
            bindings_complete,
            contents_complete,
        } => {
            for (mode, writes, complete) in [
                ("binding", bindings, bindings_complete),
                ("contents", contents, contents_complete),
            ] {
                let writes = control.operand_origin_writes.span(writes).map(|rows| {
                    rows.iter()
                        .map(|write| {
                            (
                                write.root,
                                control.operand_referent_segments.span(write.segments),
                            )
                        })
                        .collect::<Vec<_>>()
                });
                eprintln!(
                    "STORAGE_QUERY refused-writes mode={mode} complete={complete} writes={writes:?}"
                );
            }
        }
        _ => {}
    }
}
