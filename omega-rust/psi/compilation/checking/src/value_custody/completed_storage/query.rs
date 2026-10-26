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

pub fn at_snapshot(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    snapshot: Snapshot,
    source: &CanonicalPlace,
    observation: StoredObservation,
) -> Option<StoredOperand> {
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
        return Some(StoredOperand::Unavailable);
    };
    let mut annotations = BTreeMap::new();
    let mut pending = vec![(snapshot, false)];
    while let Some((current, complete)) = pending.pop() {
        let Some(next) = remaining.checked_sub(1) else {
            return Some(StoredOperand::Unavailable);
        };
        remaining = next;
        if !current.is_valid() {
            continue;
        }
        if !control.storage_snapshots.is_valid(current) {
            return Some(StoredOperand::Unavailable);
        }
        let event = control.storage_snapshots.get(current);
        let key = identity(current);
        if complete {
            let result = match event.operation {
                FlowStorageOperation::Join { predecessors } => {
                    let Some(predecessors) = control.storage_predecessors.span(predecessors) else {
                        return Some(StoredOperand::Unavailable);
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
                            && matches!(observation, StoredObservation::Contents))
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
            Some(Annotation::Active) => return Some(StoredOperand::Unavailable),
            None => {}
        }
        let terminal = match event.operation {
            FlowStorageOperation::Unknown => Some(Resolution::Unavailable),
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
                    StoredObservation::Binding => (bindings, bindings_complete),
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
                                            StoredObservation::Contents => {
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
            annotations.insert(key, Annotation::Complete(result));
        } else {
            annotations.insert(key, Annotation::Active);
            pending.push((current, true));
            if let FlowStorageOperation::Join { predecessors } = event.operation {
                let Some(predecessors) = control.storage_predecessors.span(predecessors) else {
                    return Some(StoredOperand::Unavailable);
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
    match resolved(&annotations, snapshot) {
        Resolution::Incoming => None,
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
                values.push(StoredOperand::Value {
                    capture: value,
                    reference: control.operands.get(value).type_reference,
                    projection,
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
