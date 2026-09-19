//! Derive output objects from chronological descriptor lifetimes.
//!
//! Per-file operations are borrowed projections for content validation, not an
//! execution schedule. Replay retains the original stream, including error-state
//! changes and generated-source handoff ordinals. Independent files may overlap;
//! the bounded duplicate/lock contracts still apply within each file.

use std::collections::BTreeMap;

use super::directories::output_directory_record_from_attempt;
use super::hard_links::output_hard_link_record_from_attempt;
use super::output_attempts::output_file_record_from_attempts;
use super::symlinks::output_symlink_record_from_attempt;
use super::{FilesystemOutputTreeEntryReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES};
use crate::{
    FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemOperationAttempt,
};

pub(crate) struct ObservedOutputTree {
    pub entries: Vec<FilesystemOutputTreeEntryReplayRecord>,
    /// Completed attempt counts relative to the start of the Output stream.
    pub completion_ordinals: Vec<usize>,
}

enum EntryAttempts<'a> {
    File(Vec<&'a FilesystemOperationAttempt>),
    Namespace(&'a FilesystemOperationAttempt),
}

struct DescriptorLifetime {
    entry: usize,
    is_root: bool,
    open: bool,
}

pub(crate) fn output_tree_from_attempts(
    attempts: &[FilesystemOperationAttempt],
) -> Result<ObservedOutputTree, String> {
    if attempts.is_empty() {
        return Err("bounded filesystem replay requires Output entries".to_owned());
    }
    let mut entries = Vec::new();
    let mut creation_ordinals = Vec::new();
    let mut completion_ordinals = Vec::new();
    // Serialized identities are sparse u64 values, not trusted dense slots.
    // Retired entries remain present so a later create cannot revive an identity.
    let mut descriptors = BTreeMap::<FilesystemLogicalHandleIdentity, DescriptorLifetime>::new();
    for (attempt_index, attempt) in attempts.iter().enumerate() {
        if matches!(attempt.operation_tag(), 1 | 11 | 19 | 20 | 27) {
            if entries.len() == MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES {
                return Err("filesystem replay Output tree exceeds its entry ceiling".to_owned());
            }
            if attempt.operation_tag() == 1 {
                let output = attempt.logical_handle_output.ok_or_else(|| {
                    "filesystem replay Output create has no descriptor identity".to_owned()
                })?;
                if descriptors.contains_key(&output.identity) {
                    return Err("filesystem replay Output descriptor identity is reused".to_owned());
                }
                descriptors.insert(
                    output.identity,
                    DescriptorLifetime {
                        entry: entries.len(),
                        is_root: true,
                        open: true,
                    },
                );
                entries.push(EntryAttempts::File(vec![attempt]));
                completion_ordinals.push(0);
            } else {
                entries.push(EntryAttempts::Namespace(attempt));
                completion_ordinals.push(attempt_index + 1);
            }
            creation_ordinals.push(attempt_index);
            continue;
        }
        if !matches!(attempt.operation_tag(), 5 | 7 | 8 | 10 | 17 | 41..=46 | 49) {
            return Err("filesystem replay Output operation is unsupported".to_owned());
        }
        let [
            FilesystemLogicalHandleInput {
                kind: FilesystemLogicalHandleKind::Descriptor,
                resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
                operand_ordinal: 0,
            },
        ] = attempt.logical_handle_inputs.as_slice()
        else {
            return Err("filesystem replay Output operation has no exact descriptor".to_owned());
        };
        let descriptor = descriptors
            .get_mut(identity)
            .filter(|descriptor| descriptor.open)
            .ok_or_else(|| "filesystem replay Output descriptor is not live".to_owned())?;
        let entry_index = descriptor.entry;
        if completion_ordinals[entry_index] != 0 {
            return Err("filesystem replay Output file is already closed".to_owned());
        }
        let EntryAttempts::File(file_attempts) = &mut entries[entry_index] else {
            return Err("filesystem replay descriptor does not name a file".to_owned());
        };
        file_attempts.push(attempt);
        if attempt.operation_tag() == 8 {
            descriptor.open = false;
            if descriptor.is_root {
                completion_ordinals[entry_index] = attempt_index + 1;
            }
        } else if attempt.operation_tag() == 45 {
            let output = attempt.logical_handle_output.ok_or_else(|| {
                "filesystem replay Output duplicate has no descriptor identity".to_owned()
            })?;
            if descriptors.contains_key(&output.identity) {
                return Err("filesystem replay Output descriptor identity is reused".to_owned());
            }
            descriptors.insert(
                output.identity,
                DescriptorLifetime {
                    entry: entry_index,
                    is_root: false,
                    open: true,
                },
            );
        }
    }
    if descriptors.values().any(|descriptor| descriptor.open) {
        return Err("filesystem replay Output descriptor was not closed".to_owned());
    }
    let entries = entries
        .into_iter()
        .map(|entry| match entry {
            EntryAttempts::File(attempts) => output_file_record_from_attempts(&attempts)
                .map(FilesystemOutputTreeEntryReplayRecord::File),
            EntryAttempts::Namespace(attempt) => match attempt.operation_tag() {
                11 => output_directory_record_from_attempt(attempt)
                    .map(FilesystemOutputTreeEntryReplayRecord::Directory),
                20 => output_symlink_record_from_attempt(attempt)
                    .map(FilesystemOutputTreeEntryReplayRecord::Symlink),
                19 | 27 => output_hard_link_record_from_attempt(attempt)
                    .map(FilesystemOutputTreeEntryReplayRecord::HardLink),
                _ => Err("filesystem replay Output namespace operation is unsupported".to_owned()),
            },
        })
        .collect::<Result<Vec<_>, _>>()?;

    // The current final-content projection supports links to settled objects.
    // A link to an open file would require shared-object mutation tracking.
    for (entry_index, entry) in entries.iter().enumerate() {
        if let Some(link) = entry.as_hard_link() {
            let settled_source =
                entries[..entry_index]
                    .iter()
                    .enumerate()
                    .any(|(prior_index, prior)| {
                        prior.output_relative_path() == link.existing_relative_path()
                            && (prior.as_file().is_some() || prior.as_hard_link().is_some())
                            && completion_ordinals[prior_index] <= creation_ordinals[entry_index]
                    });
            if !settled_source {
                return Err(
                    "filesystem replay Output hard link must follow its source close".to_owned(),
                );
            }
        }
    }
    Ok(ObservedOutputTree {
        entries,
        completion_ordinals,
    })
}
