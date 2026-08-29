use super::{
    FilesystemOutputDirectoryReplayRecord, FilesystemOutputHardLinkReplayRecord,
    FilesystemOutputSymlinkReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES, output_logical_handle_identities,
    source_attempts_use_root, validate_output_directory_records, validate_output_duplicate_replay,
    validate_output_lock_replay,
};
use crate::{
    BuildIncludedSource, FilesystemGrantRootIdentity, FilesystemOperationAttempt,
    FilesystemOutputFileReplayRecord, FilesystemSourceInputReplayRecord,
    MAX_INCLUDED_BUILD_SOURCES, output_file_attempt_count, source_input_record_attempts,
    validate_output_replay_extents, validate_output_time_replay_retention,
};

/// One authored entry in a bounded receipted Output tree.
///
/// Directory creation occupies one filesystem attempt. A file occupies its
/// complete create/operation*/close chain. Entries retain authored order so a
/// nested child can be admitted only after its exact parent directory has
/// actually been created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesystemOutputTreeEntryReplayRecord {
    Directory(FilesystemOutputDirectoryReplayRecord),
    File(FilesystemOutputFileReplayRecord),
    HardLink(FilesystemOutputHardLinkReplayRecord),
    Symlink(FilesystemOutputSymlinkReplayRecord),
}

impl FilesystemOutputTreeEntryReplayRecord {
    pub const fn output_root(&self) -> FilesystemGrantRootIdentity {
        match self {
            Self::Directory(directory) => directory.output_root(),
            Self::File(file) => file.output_root(),
            Self::HardLink(hard_link) => hard_link.output_root(),
            Self::Symlink(symlink) => symlink.output_root(),
        }
    }

    pub fn output_relative_path(&self) -> &[u8] {
        match self {
            Self::Directory(directory) => directory.output_relative_path(),
            Self::File(file) => file.output_relative_path(),
            Self::HardLink(hard_link) => hard_link.output_relative_path(),
            Self::Symlink(symlink) => symlink.output_relative_path(),
        }
    }

    pub const fn as_directory(&self) -> Option<&FilesystemOutputDirectoryReplayRecord> {
        match self {
            Self::Directory(directory) => Some(directory),
            Self::File(_) | Self::HardLink(_) | Self::Symlink(_) => None,
        }
    }

    pub const fn as_file(&self) -> Option<&FilesystemOutputFileReplayRecord> {
        match self {
            Self::Directory(_) | Self::HardLink(_) | Self::Symlink(_) => None,
            Self::File(file) => Some(file),
        }
    }

    pub const fn as_hard_link(&self) -> Option<&FilesystemOutputHardLinkReplayRecord> {
        match self {
            Self::Directory(_) | Self::File(_) | Self::Symlink(_) => None,
            Self::HardLink(hard_link) => Some(hard_link),
        }
    }

    pub const fn as_symlink(&self) -> Option<&FilesystemOutputSymlinkReplayRecord> {
        match self {
            Self::Directory(_) | Self::File(_) | Self::HardLink(_) => None,
            Self::Symlink(symlink) => Some(symlink),
        }
    }

    pub(crate) fn attempt_count(&self) -> Option<usize> {
        match self {
            Self::Directory(_) => Some(1),
            Self::File(file) => output_file_attempt_count(file),
            Self::HardLink(_) => Some(1),
            Self::Symlink(_) => Some(1),
        }
    }
}

/// Typed Source-input plus ordered Output-tree replay grammar.
///
/// This is the common owner for directory-only, file-only, and mixed trees.
/// The older specialized records remain supported as convenience façades.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputOutputTreeReplayRecord {
    source_input: FilesystemSourceInputReplayRecord,
    output_entries: Vec<FilesystemOutputTreeEntryReplayRecord>,
    expected_included_sources: Vec<BuildIncludedSource>,
}

impl FilesystemInputOutputTreeReplayRecord {
    pub fn new(
        source_input: FilesystemSourceInputReplayRecord,
        output_entries: Vec<FilesystemOutputTreeEntryReplayRecord>,
        expected_included_sources: Vec<BuildIncludedSource>,
    ) -> Result<Self, String> {
        let source_attempts = source_input_record_attempts(source_input.clone());
        validate_output_tree_records(
            &source_attempts,
            &output_entries,
            &expected_included_sources,
        )?;
        Ok(Self {
            source_input,
            output_entries,
            expected_included_sources,
        })
    }

    pub const fn source_input(&self) -> &FilesystemSourceInputReplayRecord {
        &self.source_input
    }

    pub fn output_entries(&self) -> &[FilesystemOutputTreeEntryReplayRecord] {
        &self.output_entries
    }

    pub fn expected_included_sources(&self) -> &[BuildIncludedSource] {
        &self.expected_included_sources
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        FilesystemSourceInputReplayRecord,
        Vec<FilesystemOutputTreeEntryReplayRecord>,
        Vec<BuildIncludedSource>,
    ) {
        (
            self.source_input,
            self.output_entries,
            self.expected_included_sources,
        )
    }
}

pub(crate) fn validate_output_tree_records(
    source_attempts: &[FilesystemOperationAttempt],
    entries: &[FilesystemOutputTreeEntryReplayRecord],
    included_sources: &[BuildIncludedSource],
) -> Result<(), String> {
    validate_output_tree_shape(source_attempts, entries, included_sources)
}

pub(crate) fn validate_observed_output_tree_records(
    source_attempts: &[FilesystemOperationAttempt],
    entries: &[FilesystemOutputTreeEntryReplayRecord],
    included_sources: &[BuildIncludedSource],
) -> Result<(), String> {
    validate_output_tree_shape(source_attempts, entries, included_sources)
}

fn validate_output_tree_shape(
    source_attempts: &[FilesystemOperationAttempt],
    entries: &[FilesystemOutputTreeEntryReplayRecord],
    included_sources: &[BuildIncludedSource],
) -> Result<(), String> {
    // Observed records do not need to reconstruct the already validated typed
    // Source prefix. Validate the same tree invariants directly.
    if entries.is_empty() {
        return Err("filesystem replay Output tree must not be empty".to_owned());
    }
    if entries.len() > MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES {
        return Err(format!(
            "filesystem replay Output tree exceeds its {MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES}-entry ceiling"
        ));
    }
    let output_root = entries[0].output_root();
    if source_attempts_use_root(source_attempts, output_root) {
        return Err("filesystem replay Source and Output roots must be distinct".to_owned());
    }
    let mut retained_path_bytes = 0usize;
    for (index, entry) in entries.iter().enumerate() {
        if entry.output_root() != output_root {
            return Err("filesystem replay Output tree must use one exact root".to_owned());
        }
        if entry.output_relative_path().len() > MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES {
            return Err(format!(
                "filesystem replay Output tree path exceeds its {MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES}-byte ceiling"
            ));
        }
        retained_path_bytes = retained_path_bytes
            .checked_add(entry.output_relative_path().len())
            .filter(|bytes| {
                *bytes <= MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES
            })
            .ok_or_else(|| {
                format!(
                    "filesystem replay Output tree paths exceed their {MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES}-byte aggregate ceiling"
                )
            })?;
        if let Some(symlink) = entry.as_symlink() {
            retained_path_bytes = retained_path_bytes
                .checked_add(symlink.target_spelling().len())
                .filter(|bytes| {
                    *bytes <= MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES
                })
                .ok_or_else(|| {
                    format!(
                        "filesystem replay Output paths and symlink targets exceed their {MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES}-byte aggregate ceiling"
                    )
                })?;
        }
        if let Some(hard_link) = entry.as_hard_link() {
            retained_path_bytes = retained_path_bytes
                .checked_add(hard_link.existing_relative_path().len())
                .filter(|bytes| {
                    *bytes <= MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES
                })
                .ok_or_else(|| {
                    format!(
                        "filesystem replay Output and hard-link paths exceed their {MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES}-byte aggregate ceiling"
                    )
                })?;
            let existing = entries[..index]
                .iter()
                .find(|prior| prior.output_relative_path() == hard_link.existing_relative_path());
            if !existing
                .is_some_and(|prior| prior.as_file().is_some() || prior.as_hard_link().is_some())
            {
                return Err(
                    "filesystem replay Output hard link must follow an existing regular-file name"
                        .to_owned(),
                );
            }
        }
        if entries[..index]
            .iter()
            .any(|prior| prior.output_relative_path() == entry.output_relative_path())
        {
            return Err("filesystem replay Output paths must be distinct".to_owned());
        }
        if let Some(separator) = entry
            .output_relative_path()
            .iter()
            .rposition(|byte| *byte == b'/')
        {
            let parent = &entry.output_relative_path()[..separator];
            if !entries[..index].iter().any(|prior| {
                prior.output_relative_path() == parent && prior.as_directory().is_some()
            }) {
                return Err(
                    "filesystem replay nested Output entry must follow its exact parent directory"
                        .to_owned(),
                );
            }
        }
    }
    let directories = entries
        .iter()
        .filter_map(|entry| entry.as_directory().cloned())
        .collect::<Vec<_>>();
    if !directories.is_empty() {
        validate_output_directory_records(&directories)?;
    }
    let files = entries
        .iter()
        .filter_map(|entry| entry.as_file().cloned())
        .collect::<Vec<_>>();
    validate_output_duplicate_replay(&files)?;
    validate_output_lock_replay(&files)?;
    validate_output_time_replay_retention(&files)?;
    validate_output_replay_extents(&files)?;
    let mut output_identities = Vec::new();
    for output in &files {
        for identity in output_logical_handle_identities(output) {
            if output_identities.contains(&identity) {
                return Err(
                    "filesystem replay Output descriptors must be globally distinct".to_owned(),
                );
            }
            if crate::source_attempts_overlap_output(
                source_attempts,
                output.output_root(),
                identity,
            ) {
                return Err(
                    "filesystem replay Source and Output roots and descriptors must be distinct"
                        .to_owned(),
                );
            }
            output_identities.push(identity);
        }
    }
    validate_tree_included_sources(entries, included_sources, source_attempts.len())
}

fn validate_tree_included_sources(
    entries: &[FilesystemOutputTreeEntryReplayRecord],
    included_sources: &[BuildIncludedSource],
    source_attempt_count: usize,
) -> Result<(), String> {
    if included_sources.len() > MAX_INCLUDED_BUILD_SOURCES {
        return Err(format!(
            "filesystem replay exceeds its {MAX_INCLUDED_BUILD_SOURCES}-source handoff ceiling"
        ));
    }
    let mut close_ordinals = Vec::new();
    let mut total_attempt_count = source_attempt_count;
    for entry in entries {
        total_attempt_count = total_attempt_count
            .checked_add(
                entry
                    .attempt_count()
                    .ok_or_else(|| "filesystem replay event count overflowed".to_owned())?,
            )
            .ok_or_else(|| "filesystem replay event count overflowed".to_owned())?;
        if let Some(file) = entry.as_file() {
            close_ordinals.push((
                file.output_root(),
                file.output_relative_path(),
                total_attempt_count,
            ));
        }
    }
    let mut previous_ordinal = source_attempt_count;
    for (handoff_index, included) in included_sources.iter().enumerate() {
        if included.filesystem_attempt_ordinal() < previous_ordinal {
            return Err(
                "filesystem replay included-source handoff ordinals must be nondecreasing"
                    .to_owned(),
            );
        }
        previous_ordinal = included.filesystem_attempt_ordinal();
        if included_sources[..handoff_index].iter().any(|prior| {
            prior.root() == included.root() && prior.relative_path() == included.relative_path()
        }) {
            return Err(
                "filesystem replay included-source handoff names one output more than once"
                    .to_owned(),
            );
        }
        let Some((_, _, close_ordinal)) = close_ordinals
            .iter()
            .find(|(root, path, _)| *root == included.root() && *path == included.relative_path())
        else {
            return Err(
                "filesystem replay included-source handoff has no matching output file".to_owned(),
            );
        };
        if included.filesystem_attempt_ordinal() < *close_ordinal
            || included.filesystem_attempt_ordinal() > total_attempt_count
        {
            return Err(
                "filesystem replay included-source handoff must follow its exact Output close"
                    .to_owned(),
            );
        }
    }
    Ok(())
}
