//! Source lifetime projections retain indices into the authoritative chronology.

use std::collections::BTreeMap;

use super::native_shapes::{
    validate_native_error_observation_shape, validate_native_final_path_query_shape,
    validate_native_handle_close_shape, validate_native_query_open_shape,
};
use super::path_and_descriptor_shapes::{
    validate_close_shape, validate_descriptor_metadata_shape, validate_directory_read_shape,
    validate_open_shape, validate_path_metadata_shape, validate_read_shape,
};
use crate::evidence::replay_record::BuildFilesystemReplayRecordError;
use crate::evidence::replay_record::attempt_codec::{AttemptShape, ShapeLogicalInputResolution};
use crate::evidence::replay_record::read_links::validate_source_read_link_shape;

pub(crate) struct SourceOutputMembership {
    pub(crate) source_events: Vec<Vec<usize>>,
    pub(crate) output_attempts: Vec<usize>,
}

struct Lifetime {
    source_event: Option<usize>,
    closed: bool,
}

/// Exact failure-only grammars remain distinct from successful tree construction.
/// A failed ownership mutation belongs to its Output lifetime, not this route.
pub(crate) fn has_failure_sequence(shapes: &[AttemptShape<'_>]) -> bool {
    shapes.iter().any(|shape| {
        matches!(shape.operation, 9 | 12)
            || !shape.refusals.is_empty()
            || shape
                .inputs
                .iter()
                .any(|input| input.resolution == ShapeLogicalInputResolution::Unknown)
    })
}

pub(crate) fn source_output_membership(
    shapes: &[AttemptShape<'_>],
) -> Result<SourceOutputMembership, BuildFilesystemReplayRecordError> {
    let failure = BuildFilesystemReplayRecordError::new;
    if shapes.is_empty() {
        return Err(failure("filesystem replay contains no operation"));
    }
    let mut source_events: Vec<Vec<usize>> = Vec::new();
    let mut output_attempts = Vec::new();
    // All domains share logical identity, including retired sparse wire keys.
    let mut lifetimes = BTreeMap::<u64, Lifetime>::new();
    let mut previous_native_query_event = None;
    for (position, shape) in shapes.iter().enumerate() {
        let previous_native = previous_native_query_event.take();
        match shape.operation {
            2 | 28 => {
                let identity = if shape.operation == 2 {
                    validate_open_shape(shape)?
                } else {
                    validate_native_query_open_shape(shape)?
                };
                if identity == 0 || lifetimes.contains_key(&identity) {
                    return Err(failure(
                        "filesystem replay logical handle identity is reused",
                    ));
                }
                lifetimes.insert(
                    identity,
                    Lifetime {
                        source_event: Some(source_events.len()),
                        closed: false,
                    },
                );
                if shape.operation == 28 {
                    previous_native_query_event = Some(source_events.len());
                }
                source_events.push(vec![position]);
                continue;
            }
            21 | 38 | 40 => {
                if shape.operation == 21 {
                    validate_source_read_link_shape(shape)?;
                } else {
                    validate_path_metadata_shape(shape)?;
                }
                source_events.push(vec![position]);
                continue;
            }
            35 => {
                let event: usize = previous_native.ok_or_else(|| failure(
                    "Source last-error observation does not immediately follow its native query",
                ))?;
                validate_native_error_observation_shape(shape)?;
                source_events[event].push(position);
                previous_native_query_event = Some(event);
                continue;
            }
            _ => {}
        }
        if let [input] = shape.inputs.as_slice()
            && let ShapeLogicalInputResolution::Resolved(identity) = input.resolution
            && let Some(lifetime) = lifetimes.get_mut(&identity)
            && let Some(event) = lifetime.source_event
        {
            if lifetime.closed {
                return Err(failure("Source operation uses a retired logical handle"));
            }
            source_events[event].push(position);
            if matches!(shape.operation, 8 | 29) {
                lifetime.closed = true;
            }
            if shape.operation == 31 && shapes[source_events[event][0]].operation == 28 {
                previous_native_query_event = Some(event);
            }
            continue;
        }
        if let Some(output) = shape.output {
            if output.identity == 0 || lifetimes.contains_key(&output.identity) {
                return Err(failure(
                    "filesystem replay logical handle identity is reused",
                ));
            }
            lifetimes.insert(
                output.identity,
                Lifetime {
                    source_event: None,
                    closed: false,
                },
            );
        }
        output_attempts.push(position);
    }
    for attempts in &source_events {
        validate_source_event(shapes, attempts)?;
    }
    Ok(SourceOutputMembership {
        source_events,
        output_attempts,
    })
}

fn validate_source_event(
    shapes: &[AttemptShape<'_>],
    attempts: &[usize],
) -> Result<(), BuildFilesystemReplayRecordError> {
    let failure = BuildFilesystemReplayRecordError::new;
    let open = &shapes[attempts[0]];
    if matches!(open.operation, 21 | 38 | 40) {
        return Ok(());
    }
    if attempts.len() < 3 {
        return Err(failure("filesystem replay Source lifetime is incomplete"));
    }
    let identity = open
        .output
        .ok_or_else(|| failure("Source lifetime has no identity"))?
        .identity;
    let close = &shapes[*attempts
        .last()
        .ok_or_else(|| failure("Source lifetime has no close"))?];
    let operations = &attempts[1..attempts.len() - 1];
    if open.operation == 28 {
        validate_native_handle_close_shape(close, identity)?;
        let mut has_query = false;
        for position in operations {
            let operation = &shapes[*position];
            if operation.operation == 31 {
                validate_native_final_path_query_shape(operation, identity)?;
                has_query = true;
            } else if operation.operation == 35 {
                validate_native_error_observation_shape(operation)?;
            } else {
                return Err(failure(
                    "Source native lifetime contains an unsupported operation",
                ));
            }
        }
        if !has_query {
            return Err(failure("Source native lifetime has no final-path query"));
        }
    } else {
        validate_close_shape(close, identity)?;
        match shapes[operations[0]].operation {
            39 if operations.len() == 1 => {
                validate_descriptor_metadata_shape(&shapes[operations[0]], identity)?;
            }
            23 => {
                for position in operations {
                    validate_directory_read_shape(&shapes[*position], identity)?;
                }
            }
            _ => {
                for position in operations {
                    validate_read_shape(&shapes[*position], identity)?;
                }
            }
        }
    }
    Ok(())
}
