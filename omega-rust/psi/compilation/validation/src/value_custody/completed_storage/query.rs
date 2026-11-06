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
        return Resolution::Unavailable;
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

/// The same descriptor and its payload can share a source-level path.
/// Only the checked type walk can distinguish those endpoints; a mismatch
/// that proves no reference crossing remains unavailable.
fn store_endpoint_matches(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    destination: checked_trees::FlowOperandReferent,
    receiving: typed_trees::types::TypeReferenceHandle,
    observed_path: &[facts::PlaceSegment],
    expected: typed_trees::types::TypeReferenceHandle,
) -> Result<bool, &'static str> {
    if !program
        .type_reference_table
        .contains_type_reference(receiving)
        || !program
            .type_reference_table
            .contains_type_reference(expected)
    {
        return Err("the store or observation has no checked receiving endpoint");
    }
    let prefix = control
        .operand_referent_segments
        .span(destination.segments)
        .ok_or("the store destination has no complete path")?;
    let suffix = observed_path
        .get(prefix.len()..)
        .ok_or("the observation does not extend this store destination")?;
    let stored = crate::project_type_reference_from_segments(program, receiving, suffix)
        .ok_or("the stored endpoint has no complete selected type")?;
    if crate::declared_carriers_match(program, stored, expected) {
        return Ok(true);
    }
    if !suffix.is_empty() {
        return Err("the selected stored value and observation have different carriers");
    }
    let mut preceding = expected;
    let mut visited = Vec::new();
    while let Some(shell) = crate::reference_shell(program, preceding) {
        if visited.contains(&preceding) {
            return Err("the observation has a cyclic reference endpoint");
        }
        visited.push(preceding);
        preceding = shell.referee;
        if crate::declared_carriers_match(program, preceding, stored) {
            return Ok(false);
        }
    }
    Err("the store has no proved value or post-reference relation to this observation")
}

fn observation_endpoint(
    program: &dyn crate::CheckingInput,
    source: &CanonicalPlace,
    observation: StoredObservation,
    state: symbols::SymbolHandle,
    statement_index: usize,
) -> typed_trees::types::TypeReferenceHandle {
    // The actual store has completed its lexical declaration. This is a
    // declaration cut, not an execution successor or a later binding replay.
    let declared = statement_index
        .checked_add(1)
        .and_then(|after| crate::canonical_place_type_reference(program, state, after, source))
        .unwrap_or_default();
    match observation {
        StoredObservation::Binding | StoredObservation::InlineContents => declared,
        StoredObservation::Contents => {
            crate::reference_shell(program, declared).map_or(declared, |shell| shell.referee)
        }
    }
}

/// A write past a stored pointer changes its pointee, while the pointer's
/// inline binding survives. Every path and receiving type must still match;
/// an unknown or unrelated endpoint never proves preservation.
fn write_preserves_endpoint(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    write: checked_trees::FlowOperandReferent,
    receiving: typed_trees::types::TypeReferenceHandle,
    source: &CanonicalPlace,
    expected: typed_trees::types::TypeReferenceHandle,
) -> Result<bool, &'static str> {
    if matches_store(program, control, source, write) == Some(true) {
        return store_endpoint_matches(
            program,
            control,
            write,
            receiving,
            &source.segments,
            expected,
        )
        .map(|matches| !matches);
    }
    let written = control
        .operand_referent_segments
        .span(write.segments)
        .ok_or("the write has no complete selected path")?;
    if source.segments.len() >= written.len()
        || !source.segments.iter().zip(written).all(|(left, right)| {
            crate::storage_path_relations::canonical_place_segments_equal(program, *left, *right)
        })
    {
        return Err("the overlapping write has no exact endpoint prefix");
    }
    let suffix = &written[source.segments.len()..];
    let at_write = crate::project_type_reference_from_segments(program, expected, suffix)
        .ok_or("the inline observation has no complete typed write path")?;
    let matches = store_endpoint_matches(program, control, write, receiving, written, at_write)?;
    if !matches {
        return Ok(true);
    }
    for length in 0..suffix.len() {
        let prefix =
            crate::project_type_reference_from_segments(program, expected, &suffix[..length])
                .ok_or("the inline observation has an unavailable write prefix")?;
        if crate::reference_shell(program, prefix).is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn write_preserves_observation(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    source: &CanonicalPlace,
    observation: StoredObservation,
    write: checked_trees::FlowOperandReferent,
    endpoint: Option<checked_trees::FlowStorageWriteEndpoint>,
    expected: Option<typed_trees::types::TypeReferenceHandle>,
) -> Result<bool, &'static str> {
    let segments = control
        .operand_referent_segments
        .span(write.segments)
        .ok_or("the observed write path is unavailable")?;
    let overlaps = match observation {
        StoredObservation::Binding => {
            crate::storage_path_relations::binding_overwrite(program, source, segments)
        }
        StoredObservation::InlineContents | StoredObservation::Contents => {
            crate::storage_path_relations::contents_overlap(program, source, segments)
        }
    };
    if !overlaps {
        return Ok(true);
    }
    if matches!(observation, StoredObservation::Contents) {
        return Ok(false);
    }
    let Some(endpoint) = endpoint else {
        return Ok(false);
    };
    let expected = expected.unwrap_or_else(|| {
        observation_endpoint(
            program,
            source,
            observation,
            endpoint.state,
            endpoint.statement_index,
        )
    });
    write_preserves_endpoint(
        program,
        control,
        write,
        endpoint.receiving_type,
        source,
        expected,
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
    matches!(resolve_snapshot(program, control, snapshot, source, observation, SourceBoundary::ReadCuts(&cuts), None),
        Some(Resolution::Sources { incoming: false, stores }) if !stores.is_empty())
}

fn resolve_snapshot(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    snapshot: Snapshot,
    source: &CanonicalPlace,
    observation: StoredObservation,
    boundary: SourceBoundary<'_>,
    endpoint: Option<typed_trees::types::TypeReferenceHandle>,
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
            return Some(Resolution::Unavailable);
        }
        if !control.storage_snapshots.is_valid(current) {
            return Some(Resolution::Unavailable);
        }
        let event = control.storage_snapshots.get(current);
        let key = identity(current);
        if matches!(event.operation, FlowStorageOperation::Entry) && event.previous.is_valid() {
            return Some(Resolution::Unavailable);
        }
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
            FlowStorageOperation::Entry => Some(if event.previous.is_valid() {
                Resolution::Unavailable
            } else {
                Resolution::Incoming
            }),
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
                destination,
                value,
                receiving_type,
                state,
                statement_index,
                ..
            } => match matches_store(program, control, source, destination) {
                None => Some(Resolution::Unavailable),
                Some(false) => None,
                Some(true) => {
                    let expected = endpoint.unwrap_or_else(|| {
                        observation_endpoint(program, source, observation, state, statement_index)
                    });
                    match store_endpoint_matches(
                        program,
                        control,
                        destination,
                        receiving_type,
                        &source.segments,
                        expected,
                    ) {
                        Ok(false) => None,
                        Ok(true) if control.operands.is_valid(value) => Some(Resolution::Sources {
                            incoming: false,
                            stores: vec![current],
                        }),
                        result => {
                            if trace_query_enabled(source) {
                                eprintln!(
                                    "STORAGE_QUERY endpoint-unavailable source={source:?} snapshot={current:?} expected={expected:?} receiving={receiving_type:?} state={state:?} statement={statement_index} reason={result:?}"
                                );
                            }
                            Some(Resolution::Unavailable)
                        }
                    }
                }
            },
            FlowStorageOperation::Writes {
                bindings,
                contents,
                bindings_complete,
                contents_complete,
                endpoint: write_endpoint,
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
                                write.root != source.root || write_preserves_observation(
                                    program, control, source, observation, *write, write_endpoint, endpoint,
                                ).inspect_err(|reason| {
                                    if trace_query_enabled(source) {
                                        eprintln!("STORAGE_QUERY write-endpoint-unavailable source={source:?} snapshot={current:?} endpoint={write_endpoint:?} expected={endpoint:?} reason={reason}");
                                    }
                                }) == Ok(true)
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
        None,
    )?;
    if matches!(resolved, Resolution::Incoming) {
        None
    } else {
        sources_from_resolution(control, snapshot, source, resolved)
    }
}

/// Select this exact stored endpoint, not a payload written through one
/// of its reference shells. The caller retains the selected carrier from
/// the actual capture; receiving annotations supply no storage identity.
pub fn at_typed_snapshot(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    snapshot: Snapshot,
    source: &CanonicalPlace,
    observation: StoredObservation,
    expected: typed_trees::types::TypeReferenceHandle,
) -> Option<StoredOperand> {
    if !program
        .type_reference_table
        .contains_type_reference(expected)
        || !control.storage_snapshots.is_valid(snapshot)
    {
        return Some(StoredOperand::Unavailable);
    }
    let resolved = resolve_snapshot(
        program,
        control,
        snapshot,
        source,
        observation,
        SourceBoundary::Stores,
        Some(expected),
    )?;
    if matches!(resolved, Resolution::Incoming) {
        None
    } else {
        sources_from_resolution(control, snapshot, source, resolved)
    }
}

pub fn sources_at_typed_snapshot(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    snapshot: Snapshot,
    source: &CanonicalPlace,
    observation: StoredObservation,
    expected: typed_trees::types::TypeReferenceHandle,
) -> Option<StoredOperand> {
    if !program
        .type_reference_table
        .contains_type_reference(expected)
        || !control.storage_snapshots.is_valid(snapshot)
    {
        return Some(StoredOperand::Unavailable);
    }
    let resolved = resolve_snapshot(
        program,
        control,
        snapshot,
        source,
        observation,
        SourceBoundary::Stores,
        Some(expected),
    )?;
    sources_from_resolution(control, snapshot, source, resolved)
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
        None,
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
                    ..
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
    if std::env::var_os("OMEGA_STORAGE_QUERY_TRACE").is_none() {
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
            ..
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
            ..
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

#[cfg(test)]
mod tests;
