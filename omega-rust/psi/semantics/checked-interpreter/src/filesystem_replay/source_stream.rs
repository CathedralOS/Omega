//! Associate Source receipts with their original events and descriptor lifetimes.
//!
//! A projection is only an input to the exact per-event validator. It never
//! changes execution order or handoff coordinates. Output operations execute
//! against the virtual tree; Source operations inject their retained inputs.

use std::collections::BTreeMap;

use crate::{
    FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind, FilesystemOperationAttempt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceEventMembership {
    pub events: Vec<Vec<usize>>,
    pub source_attempts: Vec<bool>,
}

struct SourceLifetime {
    event: usize,
    kind: FilesystemLogicalHandleKind,
    open: bool,
}

impl SourceEventMembership {
    pub fn discover(attempts: &[FilesystemOperationAttempt]) -> Result<Self, String> {
        let mut events = Vec::<Vec<usize>>::new();
        let mut source_attempts = vec![false; attempts.len()];
        let mut lifetimes = BTreeMap::<FilesystemLogicalHandleIdentity, SourceLifetime>::new();
        let mut preceding_native_event = None;
        for (ordinal, attempt) in attempts.iter().enumerate() {
            let mut native_event = None;
            let event = match attempt.operation_tag() {
                2 | 28 => {
                    let output = attempt.logical_handle_output.ok_or_else(|| {
                        "filesystem replay Source open has no logical identity".to_owned()
                    })?;
                    let kind = if attempt.operation_tag() == 2 {
                        FilesystemLogicalHandleKind::Descriptor
                    } else {
                        FilesystemLogicalHandleKind::Native
                    };
                    if output.kind != kind || lifetimes.contains_key(&output.identity) {
                        return Err(
                            "filesystem replay Source identity is invalid or reused".to_owned()
                        );
                    }
                    let event = events.len();
                    events.push(Vec::new());
                    lifetimes.insert(
                        output.identity,
                        SourceLifetime {
                            event,
                            kind,
                            open: true,
                        },
                    );
                    if kind == FilesystemLogicalHandleKind::Native {
                        native_event = Some(event);
                    }
                    Some(event)
                }
                21 | 38 | 40 => {
                    let event = events.len();
                    events.push(Vec::new());
                    Some(event)
                }
                35 => {
                    // This operation has no handle operand. Only its immediate
                    // chronological native query context can identify its event.
                    native_event = preceding_native_event;
                    preceding_native_event
                }
                _ => {
                    let mut source_event = None;
                    for input in &attempt.logical_handle_inputs {
                        let FilesystemLogicalHandleInputResolution::Resolved(identity) =
                            input.resolution
                        else {
                            continue;
                        };
                        let Some(lifetime) = lifetimes.get_mut(&identity) else {
                            continue;
                        };
                        if !lifetime.open
                            || input.kind != lifetime.kind
                            || input.operand_ordinal != 0
                            || attempt.logical_handle_inputs.len() != 1
                        {
                            return Err(
                                "filesystem replay Source operation has no exact live handle"
                                    .to_owned(),
                            );
                        }
                        source_event = Some(lifetime.event);
                        if matches!(attempt.operation_tag(), 8 | 29) {
                            lifetime.open = false;
                        }
                        if attempt.operation_tag() == 31
                            && lifetime.kind == FilesystemLogicalHandleKind::Native
                        {
                            native_event = Some(lifetime.event);
                        }
                    }
                    source_event
                }
            };
            if let Some(event) = event {
                events[event].push(ordinal);
                source_attempts[ordinal] = true;
            }
            preceding_native_event = native_event;
        }
        if lifetimes.values().any(|lifetime| lifetime.open) {
            return Err("filesystem replay Source handle was not closed".to_owned());
        }
        Ok(Self {
            events,
            source_attempts,
        })
    }

    pub fn project<'a>(
        &self,
        attempts: &'a [FilesystemOperationAttempt],
    ) -> Vec<Vec<&'a FilesystemOperationAttempt>> {
        self.events
            .iter()
            .map(|event| event.iter().map(|ordinal| &attempts[*ordinal]).collect())
            .collect()
    }

    pub fn output_attempts<'a>(
        &self,
        attempts: &'a [FilesystemOperationAttempt],
    ) -> Vec<(usize, &'a FilesystemOperationAttempt)> {
        attempts
            .iter()
            .enumerate()
            .filter(|(ordinal, _)| !self.source_attempts[*ordinal])
            .collect()
    }
}
