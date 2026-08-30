use crate::{
    BuildCanonicalSourceMetadataIdentity, BuildFilesystemGrantAccess,
    BuildFilesystemGrantRefusalReason, BuildFilesystemLogicalHandleInputResolution,
    BuildFilesystemLogicalHandleKind, BuildFilesystemLogicalHandleOutputSource,
    BuildFilesystemMetadataObservationKind, BuildFilesystemObservedByteRegionKind,
    BuildFilesystemOperationAttempt, BuildFilesystemOperationResult, BuildFilesystemProvider,
    BuildFilesystemReturnedPathCompleteness, BuildFilesystemReturnedPathKind, BuildFilesystemRoot,
    BuildFilesystemScalarOperandValue, BuildObservationSummary,
};
#[cfg(test)]
use crate::{BuildFilesystemReplayDisposition, BuildFilesystemReplayVerdict};
use sha2::{Digest, Sha256};
use std::fmt;

#[cfg(test)]
mod absent_remove_tests;
#[cfg(test)]
mod descriptor_error_state_failure_tests;
mod descriptor_error_state_failures;
mod directories;
#[cfg(test)]
mod directory_tests;
mod duplicates;
mod exact_failure_rehydration;
#[cfg(test)]
mod handle_failure_tests;
mod handle_failures;
mod hard_links;
#[cfg(test)]
mod lock_tests;
mod locks;
#[cfg(test)]
mod native_error_state_failure_tests;
mod native_error_state_failures;
#[cfg(test)]
mod native_mutation_failure_tests;
mod native_mutation_failures;
#[cfg(test)]
mod output_only_tests;
mod output_ownership;
#[cfg(test)]
mod output_ownership_tests;
#[cfg(test)]
mod read_dir_failure_tests;
mod read_dir_failures;
#[cfg(test)]
mod read_link_tests;
mod read_links;
#[cfg(test)]
mod source_directory_tests;
mod symlinks;

use descriptor_error_state_failures::{
    unknown_descriptor_failure_with_errno_operations,
    unknown_descriptor_failure_with_errno_shapes_are_exact,
    validate_unknown_descriptor_failure_with_errno_shapes,
};
use directories::validate_output_directory_shape;
use duplicates::validate_output_duplicate_shapes;
use exact_failure_rehydration::{
    exact_single_failure_shape_is_supported, rehydrate_exact_single_failure_shape,
};
use handle_failures::{
    operand_free_unknown_descriptor_operation, unknown_descriptor_read_operation,
    unknown_descriptor_write_operation, unknown_descriptor_write_payload_operation,
    validate_operand_free_unknown_descriptor_failure_shape,
    validate_unknown_descriptor_get_osfhandle_failure_shape,
    validate_unknown_descriptor_open_at_failure_shape,
    validate_unknown_descriptor_read_failure_shape,
    validate_unknown_descriptor_read_file_metadata_failure_shape,
    validate_unknown_descriptor_seek_failure_shape,
    validate_unknown_descriptor_set_file_times_failure_shape,
    validate_unknown_descriptor_unlink_at_failure_shape,
    validate_unknown_descriptor_write_operation_failure_shape,
    validate_unknown_descriptor_write_payload_failure_shape,
    validate_unknown_native_handle_close_failure_shape,
    validate_unknown_native_handle_final_path_failure_shape,
};
use hard_links::{
    output_hard_link_paths, rehydrate_output_hard_link_shape, validate_output_hard_link_shape,
};
use locks::validate_output_lock_shapes;
use native_error_state_failures::{
    unknown_native_handle_failure_with_last_error_shapes_are_exact,
    validate_unknown_native_handle_failure_with_last_error_shapes,
};
use native_mutation_failures::{
    UnknownNativeHandleMutationShape, unknown_native_handle_mutation_shape,
    validate_unknown_native_handle_mutation_failure_shape,
};
use output_ownership::validate_output_change_file_owner_shape;
use read_dir_failures::validate_unknown_descriptor_read_dir_failure_shape;
use read_links::{rehydrate_source_read_link_shape, validate_source_read_link_shape};
use symlinks::{rehydrate_output_symlink_shape, validate_output_symlink_shape};

const MAGIC: &[u8] = b"OMEGA-BUILD-FILESYSTEM-REPLAY-RECORD\0";
const COMMITMENT_DOMAIN: &[u8] = b"OMEGA-BUILD-FILESYSTEM-REPLAY-RECORD-COMMITMENT\0";
const VERSION: u16 = 52;

/// Resource ceilings for build-evaluation recovery of one partial filesystem
/// replay record. These are decoder sponsorship limits, not Omega language
/// limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildFilesystemReplayRecordLimits {
    maximum_bytes: usize,
    maximum_items_per_lane: usize,
}

impl BuildFilesystemReplayRecordLimits {
    pub const fn new(maximum_bytes: usize, maximum_items_per_lane: usize) -> Self {
        Self {
            maximum_bytes,
            maximum_items_per_lane,
        }
    }

    pub const fn maximum_bytes(self) -> usize {
        self.maximum_bytes
    }
}

impl Default for BuildFilesystemReplayRecordLimits {
    fn default() -> Self {
        Self::new(64 * 1024 * 1024, 4_096)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFilesystemReplayRecordError {
    message: &'static str,
}

impl BuildFilesystemReplayRecordError {
    const fn new(message: &'static str) -> Self {
        Self { message }
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for BuildFilesystemReplayRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for BuildFilesystemReplayRecordError {}

/// Canonical bytes recovered by the compiler for review-only custody.
///
/// Recovery does not reproduce compiler-issued evidence, authorize a build,
/// or establish `Receipted`. The complete bytes can later be handed back to a
/// replay executor once that executor supports restart-stable input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOnlyBuildFilesystemReplayRecord {
    canonical_bytes: Vec<u8>,
    commitment: [u8; 32],
    canonical_source_metadata_identity: Option<BuildCanonicalSourceMetadataIdentity>,
}

impl ReviewOnlyBuildFilesystemReplayRecord {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn commitment(&self) -> [u8; 32] {
        self.commitment
    }

    pub const fn canonical_source_metadata_identity(
        &self,
    ) -> Option<BuildCanonicalSourceMetadataIdentity> {
        self.canonical_source_metadata_identity
    }
}

/// Capture the exact operation record only after the compiler has completed
/// the bounded provider-free replay. A false replay fact produces no record.
pub fn capture_verified_build_filesystem_replay_record(
    summary: &BuildObservationSummary,
    limits: BuildFilesystemReplayRecordLimits,
) -> Result<Option<ReviewOnlyBuildFilesystemReplayRecord>, BuildFilesystemReplayRecordError> {
    let replay_verdict = summary.filesystem_replay_verdict();
    if !replay_verdict.replays_source_inputs() {
        return Ok(None);
    }
    let includes_output = summary
        .filesystem_operation_attempts()
        .iter()
        .any(|attempt| {
            attempt
                .rooted_path_operand_resolutions()
                .iter()
                .any(|path| path.root() == BuildFilesystemRoot::Output)
                || attempt
                    .authorized_paths()
                    .iter()
                    .any(|path| path.root() == BuildFilesystemRoot::Output)
        });
    if includes_output && !replay_verdict.is_complete() {
        return Ok(None);
    }
    let mut encoder = Encoder::new(limits.maximum_bytes);
    encoder.fixed(MAGIC);
    encoder.u16(VERSION);
    encoder.u32(summary.schema_version());
    encoder.u32(summary.filesystem_operation_schema_version());
    match summary.canonical_source_metadata_identity() {
        None => encoder.byte(0),
        Some(identity) => {
            encoder.byte(1);
            encoder.u32(identity.policy_version());
            encoder.fixed(&identity.source_content_commitment());
        }
    }
    encoder.count(summary.included_source_handoffs().len())?;
    for handoff in summary.included_source_handoffs() {
        encoder.bytes(handoff.relative_path())?;
        encoder.u64(handoff.filesystem_attempt_ordinal());
    }
    encoder.count(summary.filesystem_operation_attempts().len())?;
    for attempt in summary.filesystem_operation_attempts() {
        encode_attempt(&mut encoder, attempt)?;
    }
    let bytes = encoder.finish()?;
    let recovered = recover_review_only_build_filesystem_replay_record(&bytes, limits)?;
    Ok(Some(recovered))
}

/// Strictly recover a canonical, non-authoritative replay record.
pub fn recover_review_only_build_filesystem_replay_record(
    bytes: &[u8],
    limits: BuildFilesystemReplayRecordLimits,
) -> Result<ReviewOnlyBuildFilesystemReplayRecord, BuildFilesystemReplayRecordError> {
    let decoded = decode_shapes(bytes, limits)?;
    let canonical_bytes = clone_bytes(bytes)?;
    Ok(ReviewOnlyBuildFilesystemReplayRecord {
        commitment: record_commitment(&canonical_bytes),
        canonical_bytes,
        canonical_source_metadata_identity: decoded.canonical_source_metadata_identity,
    })
}

fn rehydrate_unknown_native_handle_mutation_kind(
    shape: &AttemptShape<'_>,
) -> Result<
    psi_checked_interpreter::FilesystemInputUnknownNativeHandleMutationReplayKind,
    BuildFilesystemReplayRecordError,
> {
    use psi_checked_interpreter::FilesystemInputUnknownNativeHandleMutationReplayKind as Kind;
    match unknown_native_handle_mutation_shape(shape) {
        Some(UnknownNativeHandleMutationShape::SetFileTime {
            creation,
            last_access,
            last_write,
        }) => Ok(Kind::SetFileTime {
            creation,
            last_access: clone_bytes(last_access)?,
            last_write: clone_bytes(last_write)?,
        }),
        Some(UnknownNativeHandleMutationShape::LockFileEx {
            flags,
            reserved,
            length_low,
            length_high,
            overlapped,
        }) => Ok(Kind::LockFileEx {
            flags,
            reserved,
            length_low,
            length_high,
            overlapped: clone_bytes(overlapped)?,
        }),
        Some(UnknownNativeHandleMutationShape::UnlockFile {
            offset_low,
            offset_high,
            length_low,
            length_high,
        }) => Ok(Kind::UnlockFile {
            offset_low,
            offset_high,
            length_low,
            length_high,
        }),
        None => Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay unknown-native-handle mutation inputs are inconsistent",
        )),
    }
}

fn rehydrate_operand_free_unknown_descriptor_kind(
    operation: u16,
) -> Result<
    psi_checked_interpreter::FilesystemInputUnknownDescriptorOperationReplayKind,
    BuildFilesystemReplayRecordError,
> {
    use psi_checked_interpreter::FilesystemInputUnknownDescriptorOperationReplayKind as Kind;
    match operation {
        8 => Ok(Kind::Close),
        43 => Ok(Kind::Sync),
        44 => Ok(Kind::SyncData),
        45 => Ok(Kind::Duplicate),
        _ => Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay operand-free descriptor operation is inconsistent",
        )),
    }
}

pub fn rehydrate_review_only_build_filesystem_replay_record(
    record: &ReviewOnlyBuildFilesystemReplayRecord,
    limits: BuildFilesystemReplayRecordLimits,
) -> Result<psi_checked_interpreter::FilesystemReplay, BuildFilesystemReplayRecordError> {
    let decoded = decode_shapes(record.canonical_bytes(), limits)?;
    let included_sources = decoded.included_sources;
    let shapes = decoded.shapes;
    let operation_suffix_start = shapes
        .iter()
        .position(|shape| matches!(shape.operation, 1 | 9 | 11 | 12 | 19 | 20 | 27))
        .or_else(|| {
            shapes.len().checked_sub(2).filter(|suffix_start| {
                unknown_native_handle_failure_with_last_error_shapes_are_exact(
                    &shapes[*suffix_start..],
                )
            })
        })
        .or_else(|| {
            shapes.len().checked_sub(2).filter(|suffix_start| {
                unknown_descriptor_failure_with_errno_shapes_are_exact(&shapes[*suffix_start..])
            })
        })
        .or_else(|| {
            shapes
                .last()
                .filter(|shape| exact_single_failure_shape_is_supported(shape))
                .map(|_| shapes.len() - 1)
        })
        .unwrap_or(shapes.len());
    let mut events = Vec::new();
    let mut cursor = 0;
    while cursor < operation_suffix_start {
        if shapes[cursor].operation == 21 {
            events.push(
                psi_checked_interpreter::FilesystemSourceInputReplayEventRecord::ReadLink(
                    rehydrate_source_read_link_shape(&shapes[cursor])?,
                ),
            );
            cursor += 1;
            continue;
        }
        if matches!(shapes[cursor].operation, 38 | 40) {
            events.push(
                psi_checked_interpreter::FilesystemSourceInputReplayEventRecord::PathMetadata(
                    rehydrate_path_metadata_shape(&shapes[cursor])?,
                ),
            );
            cursor += 1;
            continue;
        }
        let open = &shapes[cursor];
        cursor += 1;
        if shapes[cursor].operation == 39 {
            let metadata = &shapes[cursor];
            let close = &shapes[cursor + 1];
            cursor += 2;
            events.push(
                psi_checked_interpreter::FilesystemSourceInputReplayEventRecord::DescriptorMetadata(
                    rehydrate_descriptor_metadata_shape(open, metadata, close)?,
                ),
            );
            continue;
        }
        if shapes[cursor].operation == 23 {
            let reads_start = cursor;
            while shapes[cursor].operation == 23 {
                cursor += 1;
            }
            let close = &shapes[cursor];
            events.push(
                psi_checked_interpreter::FilesystemSourceInputReplayEventRecord::DirectoryReadChain(
                    rehydrate_source_directory_shape(open, &shapes[reads_start..cursor], close)?,
                ),
            );
            cursor += 1;
            continue;
        }
        let reads_start = cursor;
        while matches!(shapes.get(cursor), Some(read) if matches!(read.operation, 4 | 6)) {
            cursor += 1;
        }
        let close = &shapes[cursor];
        let read_shapes = &shapes[reads_start..cursor];
        cursor += 1;

        let ShapeResult::Handle(logical_handle_identity) = open.result else {
            unreachable!("validated bounded replay open returns a handle")
        };
        let [source_path] = open.rooted_paths.as_slice() else {
            unreachable!("validated bounded replay open has one rooted path")
        };
        let mut reads = Vec::new();
        reads.try_reserve_exact(read_shapes.len()).map_err(|_| {
            BuildFilesystemReplayRecordError::new("filesystem replay read allocation failed")
        })?;
        for read in read_shapes {
            reads.push(rehydrate_read_shape(read)?);
        }
        events.push(
            psi_checked_interpreter::FilesystemSourceInputReplayEventRecord::ReadChain(
                psi_checked_interpreter::FilesystemSourceReadChainReplayRecord::new(
                    crate::BUILD_SOURCE_ROOT_IDENTITY,
                    clone_bytes(source_path.bytes)?,
                    logical_handle_identity,
                    open.post_error,
                    reads,
                    close.post_error,
                )
                .map_err(|_| {
                    BuildFilesystemReplayRecordError::new(
                        "filesystem replay chain could not be rehydrated",
                    )
                })?,
            ),
        );
    }
    let typed_source_record = if events.is_empty() {
        None
    } else {
        Some(
            psi_checked_interpreter::FilesystemSourceInputReplayRecord::new(events).map_err(
                |_| {
                    BuildFilesystemReplayRecordError::new(
                        "filesystem replay source inputs could not be rehydrated",
                    )
                },
            )?,
        )
    };
    if operation_suffix_start == shapes.len() {
        let typed_source_record = typed_source_record.ok_or_else(|| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay source-only record has no Source events",
            )
        })?;
        return psi_checked_interpreter::FilesystemReplay::from_source_input_record(
            typed_source_record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay source inputs exceed retained replay policy",
            )
        });
    }
    if shapes.len() - operation_suffix_start == 2
        && unknown_descriptor_failure_with_errno_shapes_are_exact(&shapes[operation_suffix_start..])
    {
        let replay = rehydrate_exact_single_failure_shape(
            typed_source_record,
            &shapes[operation_suffix_start],
        )?;
        return replay
            .with_immediate_errno_after_unknown_descriptor_failure()
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay descriptor operation and errno sequence could not be rehydrated",
                )
            });
    }
    if shapes.len() - operation_suffix_start == 1
        && exact_single_failure_shape_is_supported(&shapes[operation_suffix_start])
    {
        return rehydrate_exact_single_failure_shape(
            typed_source_record,
            &shapes[operation_suffix_start],
        );
    }
    if shapes.len() - operation_suffix_start == 2
        && unknown_native_handle_failure_with_last_error_shapes_are_exact(
            &shapes[operation_suffix_start..],
        )
    {
        let replay = rehydrate_exact_single_failure_shape(
            typed_source_record,
            &shapes[operation_suffix_start],
        )?;
        return replay
            .with_immediate_last_error_after_unknown_native_handle_failure()
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay native-handle failure and last-error sequence could not be rehydrated",
                )
            });
    }
    if shapes[operation_suffix_start..]
        .iter()
        .all(|shape| matches!(shape.operation, 9 | 12))
    {
        let mut absent_removes = Vec::new();
        absent_removes
            .try_reserve_exact(shapes.len() - operation_suffix_start)
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay absent-remove allocation failed",
                )
            })?;
        for shape in &shapes[operation_suffix_start..] {
            let [rooted] = shape.rooted_paths.as_slice() else {
                unreachable!("validated absent Output remove has one rooted path")
            };
            let kind = match shape.operation {
                9 => psi_checked_interpreter::FilesystemOutputAbsentRemoveKind::File,
                12 => psi_checked_interpreter::FilesystemOutputAbsentRemoveKind::Directory,
                _ => unreachable!("validated absent Output remove has an exact operation"),
            };
            absent_removes.push(
                psi_checked_interpreter::FilesystemOutputAbsentRemoveReplayRecord::new(
                    kind,
                    crate::BUILD_OUTPUT_ROOT_IDENTITY,
                    clone_bytes(rooted.bytes)?,
                )
                .map_err(|_| {
                    BuildFilesystemReplayRecordError::new(
                        "filesystem replay absent Output remove could not be rehydrated",
                    )
                })?,
            );
        }
        let typed_record =
            psi_checked_interpreter::FilesystemInputOutputAbsentRemovesReplayRecord::new(
                typed_source_record,
                absent_removes,
            )
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay absent Output removes could not be rehydrated",
                )
            })?;
        return psi_checked_interpreter::FilesystemReplay::from_input_output_absent_removes_record(
            typed_record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay absent Output removes exceed retained replay policy",
            )
        });
    }
    let output_ranges = output_tree_ranges(&shapes, operation_suffix_start)?;
    let mut output_entries = Vec::new();
    output_entries
        .try_reserve_exact(output_ranges.len())
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new("filesystem replay Output-tree allocation failed")
        })?;
    for range in output_ranges {
        output_entries.push(match range {
            OutputShapeRange::Directory(index) => {
                let [rooted] = shapes[index].rooted_paths.as_slice() else {
                    unreachable!("validated Output directory has one rooted path")
                };
                psi_checked_interpreter::FilesystemOutputTreeEntryReplayRecord::Directory(
                    psi_checked_interpreter::FilesystemOutputDirectoryReplayRecord::new(
                        crate::BUILD_OUTPUT_ROOT_IDENTITY,
                        clone_bytes(rooted.bytes)?,
                    )
                    .map_err(|_| {
                        BuildFilesystemReplayRecordError::new(
                            "filesystem replay Output directory could not be rehydrated",
                        )
                    })?,
                )
            }
            OutputShapeRange::File { start, end } => {
                psi_checked_interpreter::FilesystemOutputTreeEntryReplayRecord::File(
                    rehydrate_output_file_shape(&shapes[start..end])?,
                )
            }
            OutputShapeRange::HardLink(index) => {
                psi_checked_interpreter::FilesystemOutputTreeEntryReplayRecord::HardLink(
                    rehydrate_output_hard_link_shape(&shapes[index])?,
                )
            }
            OutputShapeRange::Symlink(index) => {
                psi_checked_interpreter::FilesystemOutputTreeEntryReplayRecord::Symlink(
                    rehydrate_output_symlink_shape(&shapes[index])?,
                )
            }
        });
    }
    let mut expected_included_sources = Vec::new();
    expected_included_sources
        .try_reserve_exact(included_sources.len())
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay included-source allocation failed",
            )
        })?;
    for included in included_sources {
        expected_included_sources.push(
            psi_checked_interpreter::BuildIncludedSource::from_coordinate(
                crate::BUILD_OUTPUT_ROOT_IDENTITY,
                clone_bytes(included.relative_path)?,
                usize::try_from(included.filesystem_attempt_ordinal).map_err(|_| {
                    BuildFilesystemReplayRecordError::new(
                        "filesystem replay included-source ordinal exceeds this compiler host",
                    )
                })?,
            )
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay generated-source handoff could not be rehydrated",
                )
            })?,
        );
    }
    let typed_record = match typed_source_record {
        Some(typed_source_record) => {
            psi_checked_interpreter::FilesystemInputOutputTreeReplayRecord::new(
                typed_source_record,
                output_entries,
                expected_included_sources,
            )
        }
        None if operation_suffix_start == 0 => {
            psi_checked_interpreter::FilesystemInputOutputTreeReplayRecord::output_only(
                output_entries,
                expected_included_sources,
            )
        }
        None => {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay Output record has a malformed non-Source prefix",
            ));
        }
    }
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay input/Output-tree record could not be rehydrated",
        )
    })?;
    psi_checked_interpreter::FilesystemReplay::from_input_output_tree_record(typed_record).map_err(
        |_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay input/Output-tree record exceeds retained replay policy",
            )
        },
    )
}

fn rehydrate_source_directory_shape(
    open: &AttemptShape<'_>,
    reads: &[AttemptShape<'_>],
    close: &AttemptShape<'_>,
) -> Result<
    psi_checked_interpreter::FilesystemSourceDirectoryReadChainReplayRecord,
    BuildFilesystemReplayRecordError,
> {
    let ShapeResult::Handle(identity) = open.result else {
        unreachable!("validated directory replay open returns one handle")
    };
    let [source_path] = open.rooted_paths.as_slice() else {
        unreachable!("validated directory replay open has one rooted path")
    };
    let mut records = Vec::new();
    records.try_reserve_exact(reads.len()).map_err(|_| {
        BuildFilesystemReplayRecordError::new("filesystem directory replay allocation failed")
    })?;
    for read in reads {
        let ShapeResult::Scalar(result) = read.result else {
            unreachable!("validated directory replay read returns one scalar")
        };
        let [(2, ShapeScalar::U64(requested))] = read.scalars.as_slice() else {
            unreachable!("validated directory replay read has one count")
        };
        let [(1, resolution)] = read.mutable_byte_resolutions.as_slice() else {
            unreachable!("validated directory replay read has one byte resolution")
        };
        let [carrier] = read.mutable_bytes.as_slice() else {
            unreachable!("validated directory replay read has one byte carrier")
        };
        let [(3, position_resolution)] = read.mutable_i64_resolutions.as_slice() else {
            unreachable!("validated directory replay read has one cursor resolution")
        };
        let [position] = read.mutable_i64s.as_slice() else {
            unreachable!("validated directory replay read has one cursor carrier")
        };
        records.push(
            psi_checked_interpreter::FilesystemSourceDirectoryReadReplayRecord::new(
                *requested,
                result,
                read.post_error,
                clone_bytes(resolution)?,
                clone_bytes(carrier.pre)?,
                clone_bytes(carrier.post)?,
                *position_resolution,
                position.pre,
                position.post,
            )
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem directory replay read could not be rehydrated",
                )
            })?,
        );
    }
    psi_checked_interpreter::FilesystemSourceDirectoryReadChainReplayRecord::new(
        crate::BUILD_SOURCE_ROOT_IDENTITY,
        clone_bytes(source_path.bytes)?,
        identity,
        open.post_error,
        records,
        close.post_error,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem directory replay chain could not be rehydrated",
        )
    })
}

fn rehydrate_output_file_shape(
    chain: &[AttemptShape<'_>],
) -> Result<
    psi_checked_interpreter::FilesystemOutputFileReplayRecord,
    BuildFilesystemReplayRecordError,
> {
    let create = &chain[0];
    let close = chain.last().expect("validated Output file has a close");
    let operations = &chain[1..chain.len() - 1];
    let Some(output) = create.output else {
        unreachable!("validated receipted output create has a descriptor")
    };
    let [rooted] = create.rooted_paths.as_slice() else {
        unreachable!("validated receipted output create has one rooted path")
    };
    let mut operation_records = Vec::new();
    operation_records
        .try_reserve_exact(operations.len())
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay output-operation allocation failed",
            )
        })?;
    let mut operation_cursor = 0;
    while operation_cursor < operations.len() {
        let operation = &operations[operation_cursor];
        let record = match operation.operation {
            45 => {
                let Some(duplicate) = operation.output else {
                    unreachable!("validated Output duplicate has one fresh identity")
                };
                operation_cursor += 2;
                psi_checked_interpreter::FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(
                    psi_checked_interpreter::FilesystemOutputDuplicateReplayRecord::new(
                        duplicate.identity,
                    )
                    .map_err(|_| {
                        BuildFilesystemReplayRecordError::new(
                            "filesystem replay Output duplicate could not be rehydrated",
                        )
                    })?,
                )
            }
            46 => {
                let release = &operations[operation_cursor + 1];
                let [(1, ShapeScalar::I32(acquire_operation))] = operation.scalars.as_slice()
                else {
                    unreachable!("validated Output lock acquire has one i32 scalar")
                };
                let [(1, ShapeScalar::I32(release_operation))] = release.scalars.as_slice() else {
                    unreachable!("validated Output lock release has one i32 scalar")
                };
                let ShapeResult::Scalar(acquire_result) = operation.result else {
                    unreachable!("validated Output lock acquire has one scalar result")
                };
                let ShapeResult::Scalar(release_result) = release.result else {
                    unreachable!("validated Output lock release has one scalar result")
                };
                operation_cursor += 2;
                psi_checked_interpreter::FilesystemOutputFileOperationReplayRecord::LockAndUnlock(
                    psi_checked_interpreter::FilesystemOutputLockReplayRecord::new(
                        *acquire_operation,
                        acquire_result,
                        operation.post_error,
                        *release_operation,
                        release_result,
                        release.post_error,
                    )
                    .map_err(|_| {
                        BuildFilesystemReplayRecordError::new(
                            "filesystem replay Output lock could not be rehydrated",
                        )
                    })?,
                )
            }
            10 => {
                let [(1, ShapeScalar::I64(offset)), (2, ShapeScalar::I32(whence))] =
                    operation.scalars.as_slice()
                else {
                    unreachable!("validated Output seek has exact offset and whence")
                };
                let ShapeResult::Scalar(result) = operation.result else {
                    unreachable!("validated Output seek returns a scalar")
                };
                operation_cursor += 1;
                psi_checked_interpreter::FilesystemOutputFileOperationReplayRecord::Seek {
                    offset: *offset,
                    whence: *whence,
                    result,
                }
            }
            41 => {
                let [(1, ShapeScalar::I64(length))] = operation.scalars.as_slice() else {
                    unreachable!("validated Output set_len has one i64 length")
                };
                operation_cursor += 1;
                psi_checked_interpreter::FilesystemOutputFileOperationReplayRecord::SetLength {
                    length: *length,
                }
            }
            17 => {
                let [(1, ShapeScalar::U32(mode))] = operation.scalars.as_slice() else {
                    unreachable!("validated Output set_file_permissions has one u32 mode")
                };
                operation_cursor += 1;
                psi_checked_interpreter::FilesystemOutputFileOperationReplayRecord::SetFilePermissions {
                    mode: *mode,
                }
            }
            42 => {
                let [(1, times)] = operation.mutable_byte_resolutions.as_slice() else {
                    unreachable!("validated Output set_file_times has one exact carrier")
                };
                operation_cursor += 1;
                psi_checked_interpreter::FilesystemOutputFileOperationReplayRecord::SetFileTimes {
                    times: clone_bytes(times)?,
                }
            }
            43 => {
                operation_cursor += 1;
                psi_checked_interpreter::FilesystemOutputFileOperationReplayRecord::Sync
            }
            44 => {
                operation_cursor += 1;
                psi_checked_interpreter::FilesystemOutputFileOperationReplayRecord::SyncData
            }
            49 => {
                let [(1, ShapeScalar::I32(uid)), (2, ShapeScalar::I32(gid))] =
                    operation.scalars.as_slice()
                else {
                    unreachable!("validated Output change_file_owner has exact uid and gid")
                };
                let ShapeResult::Scalar(result) = operation.result else {
                    unreachable!("validated Output change_file_owner returns a scalar")
                };
                operation_cursor += 1;
                psi_checked_interpreter::FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(
                    psi_checked_interpreter::FilesystemOutputChangeFileOwnerReplayRecord::new(
                        *uid,
                        *gid,
                        result,
                        operation.post_error,
                    )
                    .map_err(|_| {
                        BuildFilesystemReplayRecordError::new(
                            "filesystem replay Output change_file_owner could not be rehydrated",
                        )
                    })?,
                )
            }
            5 | 7 => {
                let [(_, payload)] = operation.byte_operands.as_slice() else {
                    unreachable!("validated receipted output write has one payload")
                };
                let ShapeResult::Scalar(write_result) = operation.result else {
                    unreachable!("validated receipted output write returns a scalar")
                };
                let write_record = if operation.operation == 5 {
                    psi_checked_interpreter::FilesystemOutputWriteReplayRecord::new(
                        clone_bytes(payload)?,
                        write_result,
                        operation.post_error,
                    )
                } else {
                    let [(2, ShapeScalar::I64(offset))] = operation.scalars.as_slice() else {
                        unreachable!("validated positioned write has one i64 offset")
                    };
                    psi_checked_interpreter::FilesystemOutputWriteReplayRecord::positioned(
                        *offset,
                        clone_bytes(payload)?,
                        write_result,
                        operation.post_error,
                    )
                };
                operation_cursor += 1;
                psi_checked_interpreter::FilesystemOutputFileOperationReplayRecord::Write(
                    write_record.map_err(|_| {
                        BuildFilesystemReplayRecordError::new(
                            "filesystem replay output write could not be rehydrated",
                        )
                    })?,
                )
            }
            _ => unreachable!("validated Output file has an admitted operation"),
        };
        operation_records.push(record);
    }
    psi_checked_interpreter::FilesystemOutputFileReplayRecord::with_operations(
        crate::BUILD_OUTPUT_ROOT_IDENTITY,
        clone_bytes(rooted.bytes)?,
        output.identity,
        create.post_error,
        operation_records,
        close.post_error,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay Output file could not be rehydrated",
        )
    })
}

fn rehydrate_path_metadata_shape(
    shape: &AttemptShape<'_>,
) -> Result<
    psi_checked_interpreter::FilesystemSourcePathMetadataReplayRecord,
    BuildFilesystemReplayRecordError,
> {
    let [rooted] = shape.rooted_paths.as_slice() else {
        unreachable!("validated source metadata has one rooted input")
    };
    let [authorized] = shape.authorized_paths.as_slice() else {
        unreachable!("validated source metadata has one authorized target")
    };
    let [metadata] = shape.metadata.as_slice() else {
        unreachable!("validated source metadata has one semantic row")
    };
    let [(1, mutable_resolution)] = shape.mutable_byte_resolutions.as_slice() else {
        unreachable!("validated source metadata has one mutable resolution")
    };
    let [mutable] = shape.mutable_bytes.as_slice() else {
        unreachable!("validated source metadata has one mutable carrier")
    };
    let kind = match shape.operation {
        38 => psi_checked_interpreter::FilesystemMetadataObservationKind::FollowedPath,
        40 => psi_checked_interpreter::FilesystemMetadataObservationKind::UnfollowedFinalPath,
        _ => unreachable!("validated source metadata operation"),
    };
    let metadata = psi_checked_interpreter::FilesystemMetadataObservation::from_replay(
        kind,
        metadata.device,
        metadata.mode,
        metadata.link_count,
        metadata.inode,
        metadata.user,
        metadata.group,
        metadata.referenced_device,
        metadata.access_time,
        metadata.modification_time,
        metadata.change_time,
        metadata.birth_time,
        metadata.size,
        metadata.blocks_512,
        metadata.preferred_block_size,
    );
    psi_checked_interpreter::FilesystemSourcePathMetadataReplayRecord::new(
        kind,
        crate::BUILD_SOURCE_ROOT_IDENTITY,
        clone_bytes(rooted.bytes)?,
        crate::BUILD_SOURCE_ROOT_IDENTITY,
        clone_bytes(authorized.bytes)?,
        shape.post_error,
        clone_bytes(mutable_resolution)?,
        clone_bytes(mutable.pre)?,
        clone_bytes(mutable.post)?,
        metadata,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay path metadata could not be rehydrated",
        )
    })
}

fn rehydrate_descriptor_metadata_shape(
    open: &AttemptShape<'_>,
    metadata_shape: &AttemptShape<'_>,
    close: &AttemptShape<'_>,
) -> Result<
    psi_checked_interpreter::FilesystemSourceDescriptorMetadataReplayRecord,
    BuildFilesystemReplayRecordError,
> {
    let ShapeResult::Handle(logical_handle_identity) = open.result else {
        unreachable!("validated descriptor metadata open returns a handle")
    };
    let [source_path] = open.rooted_paths.as_slice() else {
        unreachable!("validated descriptor metadata open has one source path")
    };
    let [metadata] = metadata_shape.metadata.as_slice() else {
        unreachable!("validated descriptor metadata has one semantic row")
    };
    let [(1, mutable_resolution)] = metadata_shape.mutable_byte_resolutions.as_slice() else {
        unreachable!("validated descriptor metadata has one mutable resolution")
    };
    let [mutable] = metadata_shape.mutable_bytes.as_slice() else {
        unreachable!("validated descriptor metadata has one mutable carrier")
    };
    let metadata = psi_checked_interpreter::FilesystemMetadataObservation::from_replay(
        psi_checked_interpreter::FilesystemMetadataObservationKind::OpenDescriptor,
        metadata.device,
        metadata.mode,
        metadata.link_count,
        metadata.inode,
        metadata.user,
        metadata.group,
        metadata.referenced_device,
        metadata.access_time,
        metadata.modification_time,
        metadata.change_time,
        metadata.birth_time,
        metadata.size,
        metadata.blocks_512,
        metadata.preferred_block_size,
    );
    psi_checked_interpreter::FilesystemSourceDescriptorMetadataReplayRecord::new(
        crate::BUILD_SOURCE_ROOT_IDENTITY,
        clone_bytes(source_path.bytes)?,
        logical_handle_identity,
        open.post_error,
        metadata_shape.post_error,
        clone_bytes(mutable_resolution)?,
        clone_bytes(mutable.pre)?,
        clone_bytes(mutable.post)?,
        metadata,
        close.post_error,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay descriptor metadata could not be rehydrated",
        )
    })
}

fn rehydrate_read_shape(
    read: &AttemptShape<'_>,
) -> Result<psi_checked_interpreter::FilesystemReplayReadRecord, BuildFilesystemReplayRecordError> {
    let ShapeResult::Scalar(read_result) = read.result else {
        unreachable!("validated bounded replay read returns a scalar")
    };
    let (read_kind, requested_count) = match read.scalars.as_slice() {
        [(2, ShapeScalar::U64(requested_count))] => (
            psi_checked_interpreter::FilesystemReplayReadKind::Sequential,
            requested_count,
        ),
        [
            (2, ShapeScalar::U64(requested_count)),
            (3, ShapeScalar::I64(offset)),
        ] => (
            psi_checked_interpreter::FilesystemReplayReadKind::Positioned { offset: *offset },
            requested_count,
        ),
        _ => unreachable!("validated bounded replay read has exact count and optional offset"),
    };
    let [(1, mutable_resolution)] = read.mutable_byte_resolutions.as_slice() else {
        unreachable!("validated bounded replay read has one mutable resolution")
    };
    let [mutable_carrier] = read.mutable_bytes.as_slice() else {
        unreachable!("validated bounded replay read has one mutable carrier")
    };
    psi_checked_interpreter::FilesystemReplayReadRecord::new(
        read_kind,
        *requested_count,
        read_result,
        read.post_error,
        clone_bytes(mutable_resolution)?,
        clone_bytes(mutable_carrier.pre)?,
        clone_bytes(mutable_carrier.post)?,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new("filesystem replay read could not be rehydrated")
    })
}

struct DecodedReplay<'a> {
    canonical_source_metadata_identity: Option<BuildCanonicalSourceMetadataIdentity>,
    included_sources: Vec<ShapeIncludedSource<'a>>,
    shapes: Vec<AttemptShape<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShapeIncludedSource<'a> {
    relative_path: &'a [u8],
    filesystem_attempt_ordinal: u64,
}

fn decode_shapes(
    bytes: &[u8],
    limits: BuildFilesystemReplayRecordLimits,
) -> Result<DecodedReplay<'_>, BuildFilesystemReplayRecordError> {
    if bytes.len() > limits.maximum_bytes {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record exceeds its byte ceiling",
        ));
    }
    let mut decoder = Decoder::new(bytes, limits);
    decoder.fixed(MAGIC)?;
    if decoder.u16()? != VERSION {
        return Err(BuildFilesystemReplayRecordError::new(
            "unsupported filesystem replay record version",
        ));
    }
    if decoder.u32()? != crate::BUILD_OBSERVATION_SCHEMA_VERSION
        || decoder.u32()? != psi_checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "unsupported filesystem replay semantic schema",
        ));
    }
    let canonical_source_metadata_identity =
        decode_canonical_source_metadata_identity(&mut decoder)?;
    let included_source_count = decoder.count()?;
    if included_source_count > psi_checked_interpreter::MAX_INCLUDED_BUILD_SOURCES {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay exceeds its 256-source handoff ceiling",
        ));
    }
    let mut included_sources = Vec::new();
    included_sources
        .try_reserve_exact(included_source_count)
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay included-source allocation failed",
            )
        })?;
    for _ in 0..included_source_count {
        included_sources.push(ShapeIncludedSource {
            relative_path: decoder.bytes()?,
            filesystem_attempt_ordinal: decoder.u64()?,
        });
    }
    let attempt_count = decoder.count()?;
    if attempt_count == 0 {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded filesystem replay record must contain at least one filesystem attempt",
        ));
    }
    let mut shapes = Vec::new();
    shapes
        .try_reserve_exact(attempt_count)
        .map_err(|_| BuildFilesystemReplayRecordError::new("replay shape allocation failed"))?;
    for _ in 0..attempt_count {
        shapes.push(decode_attempt(&mut decoder)?);
    }
    decoder.finish()?;
    validate_first_rung(&shapes)?;
    validate_included_source_shapes(&shapes, &included_sources)?;
    Ok(DecodedReplay {
        canonical_source_metadata_identity,
        included_sources,
        shapes,
    })
}

fn decode_canonical_source_metadata_identity(
    decoder: &mut Decoder<'_>,
) -> Result<Option<BuildCanonicalSourceMetadataIdentity>, BuildFilesystemReplayRecordError> {
    match decoder.byte()? {
        0 => Ok(None),
        1 => Ok(Some(BuildCanonicalSourceMetadataIdentity::new(
            decoder.u32()?,
            decoder.array_32()?,
        ))),
        _ => Err(BuildFilesystemReplayRecordError::new(
            "invalid canonical source metadata identity tag",
        )),
    }
}

fn encode_attempt(
    encoder: &mut Encoder,
    attempt: &BuildFilesystemOperationAttempt,
) -> Result<(), BuildFilesystemReplayRecordError> {
    encoder.u16(attempt.operation_tag());
    encoder.byte(provider_tag(attempt.provider()));
    match attempt.result() {
        BuildFilesystemOperationResult::Scalar(value) => {
            encoder.byte(0);
            encoder.i64(value);
        }
        BuildFilesystemOperationResult::LogicalHandle(identity) => {
            encoder.byte(1);
            encoder.u64(identity.get());
        }
    }
    encoder.i32(attempt.post_error());

    encoder.count(attempt.scalar_operands().len())?;
    for operand in attempt.scalar_operands() {
        encoder.byte(operand.operand_ordinal());
        match operand.value() {
            BuildFilesystemScalarOperandValue::I32(value) => {
                encoder.byte(0);
                encoder.i32(value);
            }
            BuildFilesystemScalarOperandValue::U32(value) => {
                encoder.byte(1);
                encoder.u32(value);
            }
            BuildFilesystemScalarOperandValue::I64(value) => {
                encoder.byte(2);
                encoder.i64(value);
            }
            BuildFilesystemScalarOperandValue::U64(value) => {
                encoder.byte(3);
                encoder.u64(value);
            }
        }
    }

    encoder.count(attempt.byte_operands().len())?;
    for operand in attempt.byte_operands() {
        encoder.byte(operand.operand_ordinal());
        encoder.bytes(operand.bytes())?;
    }
    encoder.count(attempt.path_like_operands().len())?;
    for operand in attempt.path_like_operands() {
        encoder.byte(operand.operand_ordinal());
        encoder.bytes(operand.bytes())?;
    }
    encoder.count(attempt.rooted_path_operand_resolutions().len())?;
    for operand in attempt.rooted_path_operand_resolutions() {
        encoder.byte(operand.operand_ordinal());
        encoder.byte(root_tag(operand.root()));
        encoder.bytes(operand.relative_path())?;
    }
    encoder.count(attempt.returned_paths().len())?;
    for returned in attempt.returned_paths() {
        encoder.byte(returned.operand_ordinal());
        encoder.byte(returned_path_kind_tag(returned.kind()));
        encoder.byte(returned_path_completeness_tag(returned.completeness()));
        encoder.bytes(returned.bytes())?;
    }
    encoder.count(attempt.observed_byte_regions().len())?;
    for region in attempt.observed_byte_regions() {
        encoder.byte(region.output_operand_ordinal());
        encoder.byte(observed_region_kind_tag(region.kind()));
        encoder.u64(region.offset());
        encoder.u64(region.length());
    }
    encoder.count(attempt.metadata_observations().len())?;
    for metadata in attempt.metadata_observations() {
        encoder.byte(metadata.output_operand_ordinal());
        encoder.byte(metadata_kind_tag(metadata.kind()));
        encoder.u64(metadata.device());
        encoder.u32(metadata.mode());
        encoder.u64(metadata.link_count());
        encoder.u64(metadata.inode());
        encoder.u32(metadata.user());
        encoder.u32(metadata.group());
        encoder.u64(metadata.referenced_device());
        encoder.i64(metadata.access_time());
        encoder.i64(metadata.modification_time());
        encoder.i64(metadata.change_time());
        encoder.i64(metadata.birth_time());
        encoder.i64(metadata.size());
        encoder.u64(metadata.blocks_512());
        encoder.u64(metadata.preferred_block_size());
    }
    encoder.count(attempt.mutable_byte_operand_resolutions().len())?;
    for operand in attempt.mutable_byte_operand_resolutions() {
        encoder.byte(operand.operand_ordinal());
        encoder.bytes(operand.bytes())?;
    }
    encoder.count(attempt.mutable_i64_operand_resolutions().len())?;
    for operand in attempt.mutable_i64_operand_resolutions() {
        encoder.byte(operand.operand_ordinal());
        encoder.i64(operand.value());
    }
    encoder.count(attempt.mutable_byte_operands().len())?;
    for operand in attempt.mutable_byte_operands() {
        encoder.byte(operand.operand_ordinal());
        encoder.bytes(operand.pre_bytes())?;
        encoder.bytes(operand.post_bytes())?;
    }
    encoder.count(attempt.mutable_i64_operands().len())?;
    for operand in attempt.mutable_i64_operands() {
        encoder.byte(operand.operand_ordinal());
        encoder.i64(operand.pre_value());
        encoder.i64(operand.post_value());
    }
    encoder.count(attempt.authorized_paths().len())?;
    for path in attempt.authorized_paths() {
        encoder.byte(path.operand_ordinal());
        encoder.byte(access_tag(path.access()));
        encoder.byte(root_tag(path.root()));
        encoder.bytes(path.relative_path())?;
    }
    encoder.count(attempt.logical_handle_inputs().len())?;
    for input in attempt.logical_handle_inputs() {
        encoder.byte(input.operand_ordinal());
        encoder.byte(handle_kind_tag(input.kind()));
        match input.resolution() {
            BuildFilesystemLogicalHandleInputResolution::Resolved(identity) => {
                encoder.byte(0);
                encoder.u64(identity.get());
            }
            BuildFilesystemLogicalHandleInputResolution::Null => encoder.byte(1),
            BuildFilesystemLogicalHandleInputResolution::Unknown => encoder.byte(2),
        }
    }
    match attempt.logical_handle_output() {
        None => encoder.byte(0),
        Some(output) => {
            encoder.byte(1);
            encoder.byte(handle_kind_tag(output.kind()));
            encoder.u64(output.identity().get());
            match output.source() {
                BuildFilesystemLogicalHandleOutputSource::Created => encoder.byte(0),
                BuildFilesystemLogicalHandleOutputSource::Duplicated(identity) => {
                    encoder.byte(1);
                    encoder.u64(identity.get());
                }
                BuildFilesystemLogicalHandleOutputSource::Borrowed(identity) => {
                    encoder.byte(2);
                    encoder.u64(identity.get());
                }
            }
        }
    }
    encoder.count(attempt.retired_logical_handles().len())?;
    for identity in attempt.retired_logical_handles() {
        encoder.u64(identity.get());
    }
    encoder.count(attempt.grant_refusals().len())?;
    for refusal in attempt.grant_refusals() {
        encoder.byte(refusal.operand_ordinal());
        encoder.byte(access_tag(refusal.access()));
        encoder.byte(refusal_reason_tag(refusal.reason()));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapeResult {
    Scalar(i64),
    Handle(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapeScalar {
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShapeLogicalInput {
    ordinal: u8,
    kind: u8,
    resolution: ShapeLogicalInputResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapeLogicalInputResolution {
    Resolved(u64),
    Null,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShapeLogicalOutput {
    kind: u8,
    identity: u64,
    source: u8,
    source_identity: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShapeRootedPath<'a> {
    ordinal: u8,
    root: u8,
    bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShapeReturnedPath<'a> {
    ordinal: u8,
    kind: u8,
    completeness: u8,
    bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShapeObservedRegion {
    ordinal: u8,
    kind: u8,
    offset: u64,
    length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShapeMetadata {
    ordinal: u8,
    kind: u8,
    device: u64,
    mode: u32,
    link_count: u64,
    inode: u64,
    user: u32,
    group: u32,
    referenced_device: u64,
    access_time: i64,
    modification_time: i64,
    change_time: i64,
    birth_time: i64,
    size: i64,
    blocks_512: u64,
    preferred_block_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShapeMutableBytes<'a> {
    ordinal: u8,
    pre: &'a [u8],
    post: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShapeMutableI64 {
    ordinal: u8,
    pre: i64,
    post: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShapeAuthorizedPath<'a> {
    ordinal: u8,
    access: u8,
    root: u8,
    bytes: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AttemptShape<'a> {
    operation: u16,
    provider: u8,
    result: ShapeResult,
    post_error: i32,
    scalars: Vec<(u8, ShapeScalar)>,
    byte_operands: Vec<(u8, &'a [u8])>,
    path_like_operands: Vec<(u8, &'a [u8])>,
    rooted_paths: Vec<ShapeRootedPath<'a>>,
    returned_paths: Vec<ShapeReturnedPath<'a>>,
    returned_path_count: usize,
    observed_regions: Vec<ShapeObservedRegion>,
    metadata: Vec<ShapeMetadata>,
    mutable_byte_resolutions: Vec<(u8, &'a [u8])>,
    mutable_i64_resolutions: Vec<(u8, i64)>,
    mutable_bytes: Vec<ShapeMutableBytes<'a>>,
    mutable_i64s: Vec<ShapeMutableI64>,
    authorized_paths: Vec<ShapeAuthorizedPath<'a>>,
    inputs: Vec<ShapeLogicalInput>,
    output: Option<ShapeLogicalOutput>,
    retired: Vec<u64>,
    refusal_count: usize,
}

fn decode_attempt<'a>(
    decoder: &mut Decoder<'a>,
) -> Result<AttemptShape<'a>, BuildFilesystemReplayRecordError> {
    let operation = decoder.u16()?;
    let provider = decoder.tag(2, "invalid filesystem provider tag")?;
    let result = match decoder.tag(1, "invalid filesystem result tag")? {
        0 => ShapeResult::Scalar(decoder.i64()?),
        1 => ShapeResult::Handle(decoder.nonzero_u64()?),
        _ => unreachable!(),
    };
    let post_error = decoder.i32()?;

    let mut scalars = Vec::new();
    let count = decoder.count()?;
    scalars
        .try_reserve_exact(count)
        .map_err(|_| BuildFilesystemReplayRecordError::new("replay scalar allocation failed"))?;
    for _ in 0..count {
        let ordinal = decoder.byte()?;
        let value = match decoder.tag(3, "invalid filesystem scalar tag")? {
            0 => ShapeScalar::I32(decoder.i32()?),
            1 => ShapeScalar::U32(decoder.u32()?),
            2 => ShapeScalar::I64(decoder.i64()?),
            3 => ShapeScalar::U64(decoder.u64()?),
            _ => unreachable!(),
        };
        scalars.push((ordinal, value));
    }

    let mut byte_operands = Vec::new();
    let count = decoder.count()?;
    byte_operands.try_reserve_exact(count).map_err(|_| {
        BuildFilesystemReplayRecordError::new("replay byte-operand allocation failed")
    })?;
    for _ in 0..count {
        byte_operands.push((decoder.byte()?, decoder.bytes()?));
    }
    let path_like_operands = decode_ordinal_bytes_lane(decoder)?;

    let mut rooted_paths = Vec::new();
    let count = decoder.count()?;
    rooted_paths.try_reserve_exact(count).map_err(|_| {
        BuildFilesystemReplayRecordError::new("replay rooted-path allocation failed")
    })?;
    for _ in 0..count {
        let ordinal = decoder.byte()?;
        let root = decoder.tag(1, "invalid filesystem root tag")?;
        let bytes = decoder.bytes()?;
        rooted_paths.push(ShapeRootedPath {
            ordinal,
            root,
            bytes,
        });
    }

    let returned_path_count = decoder.count()?;
    let mut returned_paths = Vec::new();
    returned_paths
        .try_reserve_exact(returned_path_count)
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new("replay returned-path allocation failed")
        })?;
    for _ in 0..returned_path_count {
        returned_paths.push(ShapeReturnedPath {
            ordinal: decoder.byte()?,
            kind: decoder.tag(2, "invalid returned-path kind tag")?,
            completeness: decoder.tag(1, "invalid returned-path completeness tag")?,
            bytes: decoder.bytes()?,
        });
    }
    let mut observed_regions = Vec::new();
    let count = decoder.count()?;
    observed_regions.try_reserve_exact(count).map_err(|_| {
        BuildFilesystemReplayRecordError::new("replay observed-region allocation failed")
    })?;
    for _ in 0..count {
        observed_regions.push(ShapeObservedRegion {
            ordinal: decoder.byte()?,
            kind: decoder.tag(3, "invalid observed-byte-region tag")?,
            offset: decoder.u64()?,
            length: decoder.u64()?,
        });
    }
    let mut metadata = Vec::new();
    let count = decoder.count()?;
    metadata
        .try_reserve_exact(count)
        .map_err(|_| BuildFilesystemReplayRecordError::new("replay metadata allocation failed"))?;
    for _ in 0..count {
        metadata.push(ShapeMetadata {
            ordinal: decoder.byte()?,
            kind: decoder.tag(2, "invalid metadata-observation tag")?,
            device: decoder.u64()?,
            mode: decoder.u32()?,
            link_count: decoder.u64()?,
            inode: decoder.u64()?,
            user: decoder.u32()?,
            group: decoder.u32()?,
            referenced_device: decoder.u64()?,
            access_time: decoder.i64()?,
            modification_time: decoder.i64()?,
            change_time: decoder.i64()?,
            birth_time: decoder.i64()?,
            size: decoder.i64()?,
            blocks_512: decoder.u64()?,
            preferred_block_size: decoder.u64()?,
        });
    }
    let mut mutable_byte_resolutions = Vec::new();
    let count = decoder.count()?;
    mutable_byte_resolutions
        .try_reserve_exact(count)
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new("replay mutable-resolution allocation failed")
        })?;
    for _ in 0..count {
        mutable_byte_resolutions.push((decoder.byte()?, decoder.bytes()?));
    }
    let count = decoder.count()?;
    let mut mutable_i64_resolutions = Vec::new();
    mutable_i64_resolutions
        .try_reserve_exact(count)
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new("replay mutable-i64-resolution allocation failed")
        })?;
    for _ in 0..count {
        mutable_i64_resolutions.push((decoder.byte()?, decoder.i64()?));
    }
    let mut mutable_bytes = Vec::new();
    let count = decoder.count()?;
    mutable_bytes.try_reserve_exact(count).map_err(|_| {
        BuildFilesystemReplayRecordError::new("replay mutable-byte allocation failed")
    })?;
    for _ in 0..count {
        mutable_bytes.push(ShapeMutableBytes {
            ordinal: decoder.byte()?,
            pre: decoder.bytes()?,
            post: decoder.bytes()?,
        });
    }
    let count = decoder.count()?;
    let mut mutable_i64s = Vec::new();
    mutable_i64s.try_reserve_exact(count).map_err(|_| {
        BuildFilesystemReplayRecordError::new("replay mutable-i64 allocation failed")
    })?;
    for _ in 0..count {
        mutable_i64s.push(ShapeMutableI64 {
            ordinal: decoder.byte()?,
            pre: decoder.i64()?,
            post: decoder.i64()?,
        });
    }
    let mut authorized_paths = Vec::new();
    let count = decoder.count()?;
    authorized_paths.try_reserve_exact(count).map_err(|_| {
        BuildFilesystemReplayRecordError::new("replay authorized-path allocation failed")
    })?;
    for _ in 0..count {
        authorized_paths.push(ShapeAuthorizedPath {
            ordinal: decoder.byte()?,
            access: decoder.tag(1, "invalid filesystem grant-access tag")?,
            root: decoder.tag(1, "invalid filesystem root tag")?,
            bytes: decoder.bytes()?,
        });
    }

    let mut inputs = Vec::new();
    let count = decoder.count()?;
    inputs.try_reserve_exact(count).map_err(|_| {
        BuildFilesystemReplayRecordError::new("replay handle-input allocation failed")
    })?;
    for _ in 0..count {
        let ordinal = decoder.byte()?;
        let kind = decoder.tag(2, "invalid logical-handle kind tag")?;
        let resolution = match decoder.tag(2, "invalid logical-handle resolution tag")? {
            0 => ShapeLogicalInputResolution::Resolved(decoder.nonzero_u64()?),
            1 => ShapeLogicalInputResolution::Null,
            2 => ShapeLogicalInputResolution::Unknown,
            _ => unreachable!(),
        };
        inputs.push(ShapeLogicalInput {
            ordinal,
            kind,
            resolution,
        });
    }
    let output = match decoder.tag(1, "invalid logical-handle output option tag")? {
        0 => None,
        1 => {
            let kind = decoder.tag(2, "invalid logical-handle kind tag")?;
            let identity = decoder.nonzero_u64()?;
            let source = decoder.tag(2, "invalid logical-handle source tag")?;
            let source_identity = (source != 0).then(|| decoder.nonzero_u64()).transpose()?;
            Some(ShapeLogicalOutput {
                kind,
                identity,
                source,
                source_identity,
            })
        }
        _ => unreachable!(),
    };
    let mut retired = Vec::new();
    let count = decoder.count()?;
    retired.try_reserve_exact(count).map_err(|_| {
        BuildFilesystemReplayRecordError::new("replay retired-handle allocation failed")
    })?;
    for _ in 0..count {
        retired.push(decoder.nonzero_u64()?);
    }
    let refusal_count = decoder.count()?;
    for _ in 0..refusal_count {
        let _ = decoder.byte()?;
        let _ = decoder.tag(1, "invalid filesystem grant-access tag")?;
        let _ = decoder.tag(3, "invalid filesystem grant-refusal tag")?;
    }
    Ok(AttemptShape {
        operation,
        provider,
        result,
        post_error,
        scalars,
        byte_operands,
        path_like_operands,
        rooted_paths,
        returned_paths,
        returned_path_count,
        observed_regions,
        metadata,
        mutable_byte_resolutions,
        mutable_i64_resolutions,
        mutable_bytes,
        mutable_i64s,
        authorized_paths,
        inputs,
        output,
        retired,
        refusal_count,
    })
}

fn decode_ordinal_bytes_lane<'a>(
    decoder: &mut Decoder<'a>,
) -> Result<Vec<(u8, &'a [u8])>, BuildFilesystemReplayRecordError> {
    let count = decoder.count()?;
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| {
        BuildFilesystemReplayRecordError::new("replay path-like operand allocation failed")
    })?;
    for _ in 0..count {
        values.push((decoder.byte()?, decoder.bytes()?));
    }
    Ok(values)
}

fn validate_first_rung(
    shapes: &[AttemptShape<'_>],
) -> Result<(), BuildFilesystemReplayRecordError> {
    let mut cursor = 0;
    let mut identities = Vec::new();
    let mut event_count = 0;
    while cursor < shapes.len() {
        if matches!(
            shapes[cursor].operation,
            1 | 4
                | 5
                | 6
                | 7
                | 8
                | 9
                | 10
                | 11
                | 12
                | 14
                | 15
                | 17
                | 19
                | 20
                | 23
                | 27
                | 29
                | 30
                | 31
                | 32
                | 33
                | 34
                | 39
                | 41
                | 42
                | 43
                | 44
                | 45
                | 46
                | 49
        ) {
            break;
        }
        if shapes[cursor].operation == 21 {
            validate_source_read_link_shape(&shapes[cursor])?;
            cursor += 1;
            event_count += 1;
            continue;
        }
        if matches!(shapes[cursor].operation, 38 | 40) {
            validate_path_metadata_shape(&shapes[cursor])?;
            cursor += 1;
            event_count += 1;
            continue;
        }
        let identity = validate_open_shape(&shapes[cursor])?;
        if identities.contains(&identity) {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay source-read chains reuse a descriptor identity",
            ));
        }
        identities.push(identity);
        cursor += 1;

        if cursor < shapes.len() && shapes[cursor].operation == 39 {
            validate_descriptor_metadata_shape(&shapes[cursor], identity)?;
            cursor += 1;
            if cursor == shapes.len() {
                return Err(BuildFilesystemReplayRecordError::new(
                    "filesystem replay descriptor metadata chain is incomplete",
                ));
            }
            validate_close_shape(&shapes[cursor], identity)?;
            cursor += 1;
            event_count += 1;
            continue;
        }

        if cursor < shapes.len() && shapes[cursor].operation == 23 {
            let reads_start = cursor;
            while cursor < shapes.len() && shapes[cursor].operation == 23 {
                validate_directory_read_shape(&shapes[cursor], identity)?;
                cursor += 1;
            }
            if cursor == reads_start || cursor == shapes.len() {
                return Err(BuildFilesystemReplayRecordError::new(
                    "filesystem replay Source directory chain is incomplete",
                ));
            }
            validate_close_shape(&shapes[cursor], identity)?;
            cursor += 1;
            event_count += 1;
            continue;
        }

        let reads_start = cursor;
        while cursor < shapes.len() && matches!(shapes[cursor].operation, 4 | 6) {
            validate_read_shape(&shapes[cursor], identity)?;
            cursor += 1;
        }
        if cursor == reads_start || cursor == shapes.len() {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay source-read chain is incomplete",
            ));
        }
        validate_close_shape(&shapes[cursor], identity)?;
        cursor += 1;
        event_count += 1;
    }
    let begins_with_replay_suffix = cursor == 0
        && shapes.first().is_some_and(|shape| {
            matches!(
                shape.operation,
                1 | 4
                    | 5
                    | 6
                    | 7
                    | 8
                    | 9
                    | 10
                    | 11
                    | 12
                    | 14
                    | 15
                    | 17
                    | 19
                    | 20
                    | 23
                    | 27
                    | 29
                    | 30
                    | 31
                    | 32
                    | 33
                    | 34
                    | 39
                    | 41
                    | 42
                    | 43
                    | 44
                    | 45
                    | 46
                    | 49
            )
        });
    if event_count == 0 && !begins_with_replay_suffix {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay contains neither Source events nor a supported replay suffix",
        ));
    }
    if cursor < shapes.len() {
        if shapes.len() - cursor == 2
            && unknown_descriptor_failure_with_errno_operations(&shapes[cursor..])
        {
            validate_unknown_descriptor_failure_with_errno_shapes(&shapes[cursor..])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1
            && operand_free_unknown_descriptor_operation(shapes[cursor].operation)
        {
            validate_operand_free_unknown_descriptor_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 10 {
            validate_unknown_descriptor_seek_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 14 {
            validate_unknown_descriptor_open_at_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 15 {
            validate_unknown_descriptor_unlink_at_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 23 {
            validate_unknown_descriptor_read_dir_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1
            && unknown_descriptor_write_operation(shapes[cursor].operation)
        {
            validate_unknown_descriptor_write_operation_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 42 {
            validate_unknown_descriptor_set_file_times_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && unknown_descriptor_read_operation(shapes[cursor].operation)
        {
            validate_unknown_descriptor_read_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1
            && unknown_descriptor_write_payload_operation(shapes[cursor].operation)
        {
            validate_unknown_descriptor_write_payload_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 39 {
            validate_unknown_descriptor_read_file_metadata_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 30 {
            validate_unknown_descriptor_get_osfhandle_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 29 {
            validate_unknown_native_handle_close_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 31 {
            validate_unknown_native_handle_final_path_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 2
            && matches!(shapes[cursor].operation, 29 | 31 | 32 | 33 | 34)
            && shapes[cursor + 1].operation == 35
        {
            validate_unknown_native_handle_failure_with_last_error_shapes(&shapes[cursor..])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && matches!(shapes[cursor].operation, 32 | 33 | 34) {
            validate_unknown_native_handle_mutation_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes[cursor..]
            .iter()
            .all(|shape| matches!(shape.operation, 9 | 12))
        {
            validate_output_absent_remove_shapes(&shapes[cursor..])?;
            return Ok(());
        }
        let output_ranges = output_tree_ranges(shapes, cursor)?;
        if output_ranges.len() > psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output exceeds the Output-tree entry ceiling",
            ));
        }
        let mut output_paths = Vec::new();
        output_paths
            .try_reserve_exact(output_ranges.len())
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay output-path allocation failed",
                )
            })?;
        let mut aggregate_output_extent = 0usize;
        let mut aggregate_output_duplicates = 0usize;
        let mut aggregate_output_lock_pairs = 0usize;
        let mut aggregate_path_bytes = 0usize;
        for (entry_index, range) in output_ranges.iter().copied().enumerate() {
            let path = range.path(shapes);
            if path.len()
                > psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES
            {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output path exceeds its explicit ceiling",
                ));
            }
            aggregate_path_bytes = aggregate_path_bytes
                .checked_add(path.len())
                .filter(|bytes| {
                    *bytes
                        <= psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES
                })
                .ok_or_else(|| {
                    BuildFilesystemReplayRecordError::new(
                        "receipted build output paths exceed their aggregate ceiling",
                    )
                })?;
            if output_paths.contains(&path) {
                return Err(BuildFilesystemReplayRecordError::new(
                    "filesystem replay Output path appears more than once",
                ));
            }
            if let Some(separator) = path.iter().rposition(|byte| *byte == b'/') {
                let parent = &path[..separator];
                if !output_ranges[..entry_index]
                    .iter()
                    .any(|prior| prior.is_directory() && prior.path(shapes) == parent)
                {
                    return Err(BuildFilesystemReplayRecordError::new(
                        "receipted nested Output entry does not follow its exact parent directory",
                    ));
                }
            }
            output_paths.push(path);

            let (start, end) = match range {
                OutputShapeRange::Directory(index) => {
                    validate_output_directory_shape(&shapes[index])?;
                    continue;
                }
                OutputShapeRange::HardLink(index) => {
                    validate_output_hard_link_shape(&shapes[index])?;
                    let (existing, _) = output_hard_link_paths(&shapes[index])?;
                    aggregate_path_bytes = aggregate_path_bytes
                        .checked_add(existing.len())
                        .filter(|bytes| {
                            *bytes
                                <= psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES
                        })
                        .ok_or_else(|| {
                            BuildFilesystemReplayRecordError::new(
                                "receipted build output and hard-link paths exceed their aggregate ceiling",
                            )
                        })?;
                    if !output_ranges[..entry_index].iter().any(|prior| {
                        matches!(
                            prior,
                            OutputShapeRange::File { .. } | OutputShapeRange::HardLink(_)
                        ) && prior.path(shapes) == existing
                    }) {
                        return Err(BuildFilesystemReplayRecordError::new(
                            "filesystem replay Output hard link does not follow an existing regular-file name",
                        ));
                    }
                    continue;
                }
                OutputShapeRange::Symlink(index) => {
                    validate_output_symlink_shape(&shapes[index])?;
                    let [(_, target)] = shapes[index].path_like_operands.as_slice() else {
                        unreachable!("validated Output symlink has one target spelling")
                    };
                    aggregate_path_bytes = aggregate_path_bytes
                        .checked_add(target.len())
                        .filter(|bytes| {
                            *bytes
                                <= psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES
                        })
                        .ok_or_else(|| {
                            BuildFilesystemReplayRecordError::new(
                                "receipted build output paths and symlink targets exceed their aggregate ceiling",
                            )
                        })?;
                    continue;
                }
                OutputShapeRange::File { start, end } => (start, end),
            };
            let chain = &shapes[start..end];
            let create = &chain[0];
            let close = chain.last().expect("validated Output file has a close");
            let extent = validate_output_file(create, &chain[1..chain.len() - 1], close)?;
            aggregate_output_extent = aggregate_output_extent
                .checked_add(extent)
                .filter(|total| {
                    *total <= psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_RETAINED_BYTES
                })
                .ok_or_else(|| {
                    BuildFilesystemReplayRecordError::new(
                        "receipted build outputs exceed the aggregate replay extent ceiling",
                    )
                })?;
            let output = create
                .output
                .expect("validated output create has a descriptor");
            for identity in std::iter::once(output.identity).chain(
                chain[1..chain.len() - 1]
                    .iter()
                    .filter(|operation| operation.operation == 45)
                    .filter_map(|operation| operation.output.map(|output| output.identity)),
            ) {
                if identities.contains(&identity) {
                    return Err(BuildFilesystemReplayRecordError::new(
                        "filesystem replay Output descriptor overlaps another descriptor",
                    ));
                }
                identities.push(identity);
            }
            aggregate_output_duplicates = aggregate_output_duplicates
                .checked_add(
                    chain[1..chain.len() - 1]
                        .iter()
                        .filter(|operation| operation.operation == 45)
                        .count(),
                )
                .filter(|count| {
                    *count <= psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES
                })
                .ok_or_else(|| {
                    BuildFilesystemReplayRecordError::new(
                        "receipted build outputs exceed the duplicate-descriptor ceiling",
                    )
                })?;
            aggregate_output_lock_pairs = aggregate_output_lock_pairs
                .checked_add(
                    chain[1..chain.len() - 1]
                        .iter()
                        .filter(|operation| {
                            operation.operation == 46
                                && operation.scalars.as_slice() == [(1, ShapeScalar::I32(6))]
                        })
                        .count(),
                )
                .filter(|count| {
                    *count <= psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_LOCK_PAIRS
                })
                .ok_or_else(|| {
                    BuildFilesystemReplayRecordError::new(
                        "receipted build outputs exceed the descriptor-lock-pair ceiling",
                    )
                })?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputShapeRange {
    Directory(usize),
    File { start: usize, end: usize },
    HardLink(usize),
    Symlink(usize),
}

impl OutputShapeRange {
    fn path<'a>(self, shapes: &'a [AttemptShape<'a>]) -> &'a [u8] {
        if let Self::HardLink(index) = self {
            return output_hard_link_paths(&shapes[index])
                .expect("validated Output hard link has exact paths")
                .1;
        }
        let index = match self {
            Self::Directory(index) | Self::File { start: index, .. } | Self::Symlink(index) => {
                index
            }
            Self::HardLink(_) => unreachable!(),
        };
        shapes[index]
            .rooted_paths
            .first()
            .expect("validated Output entry has one rooted path")
            .bytes
    }

    const fn is_directory(self) -> bool {
        matches!(self, Self::Directory(_))
    }
}

fn output_file_end(
    shapes: &[AttemptShape<'_>],
    start: usize,
) -> Result<usize, BuildFilesystemReplayRecordError> {
    if shapes.get(start).is_none_or(|shape| shape.operation != 1) {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay Output file must begin with create",
        ));
    }
    let Some(root_identity) = shapes[start].output.map(|output| output.identity) else {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay Output create has no descriptor identity",
        ));
    };
    let mut cursor = start + 1;
    loop {
        if cursor == shapes.len() {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output must contain complete create-operation*-close files",
            ));
        }
        if matches!(
            shapes[cursor].operation,
            5 | 7 | 10 | 17 | 41 | 42 | 43 | 44 | 49
        ) {
            cursor += 1;
            continue;
        }
        if shapes[cursor].operation == 45 {
            if cursor + 1 >= shapes.len() || shapes[cursor + 1].operation != 8 {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output duplicate must be immediately retired",
                ));
            }
            cursor += 2;
            continue;
        }
        if shapes[cursor].operation == 46 {
            if cursor + 1 >= shapes.len() || shapes[cursor + 1].operation != 46 {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output lock must be immediately released",
                ));
            }
            cursor += 2;
            continue;
        }
        let closes_root = shapes[cursor].operation == 8
            && matches!(
                shapes[cursor].inputs.as_slice(),
                [ShapeLogicalInput {
                    resolution: ShapeLogicalInputResolution::Resolved(identity),
                    ..
                }] if *identity == root_identity
            );
        if closes_root {
            return Ok(cursor + 1);
        }
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output must contain complete create-operation*-close files",
        ));
    }
}

fn output_tree_ranges(
    shapes: &[AttemptShape<'_>],
    output_start: usize,
) -> Result<Vec<OutputShapeRange>, BuildFilesystemReplayRecordError> {
    let mut ranges = Vec::new();
    let mut cursor = output_start;
    while cursor < shapes.len() {
        match shapes[cursor].operation {
            11 => {
                ranges.push(OutputShapeRange::Directory(cursor));
                cursor += 1;
            }
            1 => {
                let end = output_file_end(shapes, cursor)?;
                ranges.push(OutputShapeRange::File { start: cursor, end });
                cursor = end;
            }
            19 | 27 => {
                ranges.push(OutputShapeRange::HardLink(cursor));
                cursor += 1;
            }
            20 => {
                ranges.push(OutputShapeRange::Symlink(cursor));
                cursor += 1;
            }
            _ => {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output must contain ordered directory, file, hard-link, or symlink entries",
                ));
            }
        }
    }
    Ok(ranges)
}

fn validate_included_source_shapes(
    shapes: &[AttemptShape<'_>],
    included_sources: &[ShapeIncludedSource<'_>],
) -> Result<(), BuildFilesystemReplayRecordError> {
    if included_sources.len() > psi_checked_interpreter::MAX_INCLUDED_BUILD_SOURCES {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay exceeds its 256-source handoff ceiling",
        ));
    }
    if included_sources.is_empty() {
        return Ok(());
    }
    let output_start = shapes
        .iter()
        .position(|shape| matches!(shape.operation, 1 | 9 | 11 | 12 | 19 | 20 | 27))
        .ok_or_else(|| {
            BuildFilesystemReplayRecordError::new(
                "source-only filesystem replay cannot retain included-source handoffs",
            )
        })?;
    let output_ranges = output_tree_ranges(shapes, output_start)?;
    let total_attempt_count = u64::try_from(shapes.len()).map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay attempt count exceeds canonical u64",
        )
    })?;
    let mut previous_ordinal = u64::try_from(output_start).map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay Source-prefix count exceeds canonical u64",
        )
    })?;
    for (handoff_index, included) in included_sources.iter().enumerate() {
        if included.filesystem_attempt_ordinal < previous_ordinal {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay included-source ordinals are not nondecreasing",
            ));
        }
        previous_ordinal = included.filesystem_attempt_ordinal;
        if included_sources[..handoff_index]
            .iter()
            .any(|prior| prior.relative_path == included.relative_path)
        {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay included-source path appears more than once",
            ));
        }
        let output_index = output_ranges
            .iter()
            .position(|range| {
                matches!(range, OutputShapeRange::File { .. })
                    && range.path(shapes) == included.relative_path
            })
            .ok_or_else(|| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay included-source path has no matching Output file",
                )
            })?;
        let OutputShapeRange::File { end, .. } = output_ranges[output_index] else {
            unreachable!("included source matched an Output file range")
        };
        let earliest_ordinal = u64::try_from(end).map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay included-source ordinal exceeds canonical u64",
            )
        })?;
        if included.filesystem_attempt_ordinal < earliest_ordinal
            || included.filesystem_attempt_ordinal > total_attempt_count
        {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay included-source handoff does not follow its Output close",
            ));
        }
    }
    Ok(())
}

fn validate_output_file(
    create: &AttemptShape<'_>,
    operations: &[AttemptShape<'_>],
    close: &AttemptShape<'_>,
) -> Result<usize, BuildFilesystemReplayRecordError> {
    let Some(output) = create.output else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output create has no descriptor identity",
        ));
    };
    let [rooted] = create.rooted_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output create has no unique rooted path",
        ));
    };
    let [authorized] = create.authorized_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output create has no unique authorization",
        ));
    };
    if create.operation != 1
        || create.provider != 2
        || create.result != ShapeResult::Handle(output.identity)
        || create.post_error != 0
        || create.scalars.as_slice()
            != [(
                1,
                ShapeScalar::I32(psi_checked_interpreter::FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE),
            )]
        || rooted.ordinal != 0
        || rooted.root != 1
        || !psi_checked_interpreter::filesystem_root_relative_path_is_canonical(rooted.bytes, false)
        || authorized.ordinal != 0
        || authorized.access != 1
        || authorized.root != 1
        || authorized.bytes != rooted.bytes
        || output.kind != 0
        || output.source != 0
        || output.source_identity.is_some()
        || !only_output_create_lanes(create)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output create is internally inconsistent",
        ));
    }

    let mut cursor = 0usize;
    let mut extent = 0usize;
    let mut peak_extent = 0usize;
    let mut duplicate_identities = Vec::new();
    let mut operation_cursor = 0;
    while operation_cursor < operations.len() {
        let operation = &operations[operation_cursor];
        if operation.operation == 45 {
            let close_duplicate = operations.get(operation_cursor + 1).ok_or_else(|| {
                BuildFilesystemReplayRecordError::new(
                    "receipted build output duplicate is not immediately retired",
                )
            })?;
            let duplicate_identity =
                validate_output_duplicate_shapes(operation, close_duplicate, output.identity)?;
            if duplicate_identity == output.identity
                || duplicate_identities.contains(&duplicate_identity)
            {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output duplicate identity is reused",
                ));
            }
            duplicate_identities.push(duplicate_identity);
            if duplicate_identities.len()
                > psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES
            {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output exceeds its duplicate-descriptor ceiling",
                ));
            }
            operation_cursor += 2;
            continue;
        }
        if operation.operation == 46 {
            let release = operations.get(operation_cursor + 1).ok_or_else(|| {
                BuildFilesystemReplayRecordError::new(
                    "receipted build output lock is not immediately released",
                )
            })?;
            validate_output_lock_shapes(operation, release, output.identity)?;
            operation_cursor += 2;
            continue;
        }
        if operation.operation == 10 {
            cursor = validate_output_seek_shape(operation, output.identity, cursor, extent)?;
            operation_cursor += 1;
            continue;
        }
        if operation.operation == 41 {
            extent = validate_output_set_length_shape(operation, output.identity)?;
            peak_extent = peak_extent.max(extent);
            operation_cursor += 1;
            continue;
        }
        if operation.operation == 17 {
            validate_output_set_file_permissions_shape(operation, output.identity)?;
            operation_cursor += 1;
            continue;
        }
        if operation.operation == 42 {
            validate_output_set_file_times_shape(operation, output.identity)?;
            operation_cursor += 1;
            continue;
        }
        if matches!(operation.operation, 43 | 44) {
            validate_output_sync_shape(operation, output.identity)?;
            operation_cursor += 1;
            continue;
        }
        if operation.operation == 49 {
            validate_output_change_file_owner_shape(operation, output.identity)?;
            operation_cursor += 1;
            continue;
        }
        let write = operation;
        let [(payload_ordinal, payload)] = write.byte_operands.as_slice() else {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output write has no unique immutable payload",
            ));
        };
        let [write_input] = write.inputs.as_slice() else {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output write has no unique descriptor input",
            ));
        };
        let payload_length = i64::try_from(payload.len()).map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "receipted build output payload exceeds this compiler host",
            )
        })?;
        let start = match write.operation {
            5 if write.scalars.is_empty() => cursor,
            7 => {
                let [(2, ShapeScalar::I64(offset))] = write.scalars.as_slice() else {
                    return Err(BuildFilesystemReplayRecordError::new(
                        "receipted positioned output write has no unique offset",
                    ));
                };
                usize::try_from(*offset).map_err(|_| {
                    BuildFilesystemReplayRecordError::new(
                        "receipted positioned output offset exceeds this compiler host",
                    )
                })?
            }
            _ => {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output write operation is unsupported",
                ));
            }
        };
        let end = start.checked_add(payload.len()).ok_or_else(|| {
            BuildFilesystemReplayRecordError::new("receipted build output extent overflowed")
        })?;
        if !payload.is_empty() {
            extent = extent.max(end);
            peak_extent = peak_extent.max(extent);
        }
        if write.operation == 5 {
            cursor = end;
        }
        if write.provider != 2
            || write.result != ShapeResult::Scalar(payload_length)
            || write.post_error != 0
            || *payload_ordinal != 1
            || *write_input
                != (ShapeLogicalInput {
                    ordinal: 0,
                    kind: 0,
                    resolution: ShapeLogicalInputResolution::Resolved(output.identity),
                })
            || !only_output_write_lanes(write)
        {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output write is internally inconsistent",
            ));
        }
        operation_cursor += 1;
    }
    if peak_extent > psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_RETAINED_BYTES {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output exceeds the replay-retention ceiling",
        ));
    }
    validate_close_shape(close, output.identity)?;
    Ok(peak_extent)
}

fn validate_output_seek_shape(
    operation: &AttemptShape<'_>,
    identity: u64,
    cursor: usize,
    extent: usize,
) -> Result<usize, BuildFilesystemReplayRecordError> {
    let [(1, ShapeScalar::I64(offset)), (2, ShapeScalar::I32(whence))] =
        operation.scalars.as_slice()
    else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output seek has no exact offset and whence",
        ));
    };
    let [input] = operation.inputs.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output seek has no unique descriptor input",
        ));
    };
    let base = match whence {
        0 => 0i64,
        1 => i64::try_from(cursor).map_err(|_| {
            BuildFilesystemReplayRecordError::new("receipted build output cursor exceeds i64")
        })?,
        2 => i64::try_from(extent).map_err(|_| {
            BuildFilesystemReplayRecordError::new("receipted build output extent exceeds i64")
        })?,
        _ => {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output seek whence is unsupported",
            ));
        }
    };
    let expected = base.checked_add(*offset).ok_or_else(|| {
        BuildFilesystemReplayRecordError::new("receipted build output seek result overflowed")
    })?;
    let result = usize::try_from(expected).map_err(|_| {
        BuildFilesystemReplayRecordError::new("receipted build output seek result is negative")
    })?;
    if operation.provider != 2
        || operation.result != ShapeResult::Scalar(expected)
        || operation.post_error != 0
        || *input
            != (ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            })
        || !only_output_seek_lanes(operation)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output seek is internally inconsistent",
        ));
    }
    Ok(result)
}

fn validate_output_set_length_shape(
    operation: &AttemptShape<'_>,
    identity: u64,
) -> Result<usize, BuildFilesystemReplayRecordError> {
    let [(1, ShapeScalar::I64(length))] = operation.scalars.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_len has no exact length",
        ));
    };
    let [input] = operation.inputs.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_len has no unique descriptor input",
        ));
    };
    let length = usize::try_from(*length).map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "receipted build output set_len length exceeds this compiler host",
        )
    })?;
    if operation.provider != 2
        || operation.result != ShapeResult::Scalar(0)
        || operation.post_error != 0
        || *input
            != (ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            })
        || !only_output_set_length_lanes(operation)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_len is internally inconsistent",
        ));
    }
    Ok(length)
}

fn validate_output_set_file_permissions_shape(
    operation: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let [(1, ShapeScalar::U32(_mode))] = operation.scalars.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_file_permissions has no exact mode",
        ));
    };
    let [input] = operation.inputs.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_file_permissions has no unique descriptor input",
        ));
    };
    if operation.provider != 2
        || operation.result != ShapeResult::Scalar(0)
        || operation.post_error != 0
        || *input
            != (ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            })
        || !only_output_set_file_permissions_lanes(operation)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_file_permissions is internally inconsistent",
        ));
    }
    Ok(())
}

fn validate_output_set_file_times_shape(
    operation: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let [(resolution_ordinal, resolution)] = operation.mutable_byte_resolutions.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_file_times has no exact input carrier",
        ));
    };
    let [carrier] = operation.mutable_bytes.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_file_times has no exact provider carrier",
        ));
    };
    let [input] = operation.inputs.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_file_times has no unique descriptor input",
        ));
    };
    if operation.provider != 2
        || operation.result != ShapeResult::Scalar(0)
        || operation.post_error != 0
        || *resolution_ordinal != 1
        || carrier.ordinal != 1
        || resolution.len() < 32
        || *resolution != carrier.pre
        || carrier.pre != carrier.post
        || *input
            != (ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            })
        || !only_output_set_file_times_lanes(operation)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_file_times is internally inconsistent",
        ));
    }
    Ok(())
}

fn validate_output_sync_shape(
    operation: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let [input] = operation.inputs.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output sync has no unique descriptor input",
        ));
    };
    if !matches!(operation.operation, 43 | 44)
        || operation.provider != 2
        || operation.result != ShapeResult::Scalar(0)
        || operation.post_error != 0
        || *input
            != (ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            })
        || !only_output_sync_lanes(operation)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output sync is internally inconsistent",
        ));
    }
    Ok(())
}

fn validate_output_absent_remove_shapes(
    shapes: &[AttemptShape<'_>],
) -> Result<(), BuildFilesystemReplayRecordError> {
    if shapes.is_empty()
        || shapes.len() > psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_ABSENT_REMOVES
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay absent Output removes exceed their attempt ceiling",
        ));
    }
    let mut retained_path_bytes = 0usize;
    for shape in shapes {
        let [rooted] = shape.rooted_paths.as_slice() else {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay absent Output remove has no unique rooted path",
            ));
        };
        let [authorized] = shape.authorized_paths.as_slice() else {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay absent Output remove has no unique authorization",
            ));
        };
        retained_path_bytes = retained_path_bytes
            .checked_add(rooted.bytes.len())
            .filter(|bytes| {
                *bytes
                    <= psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES
            })
            .ok_or_else(|| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay absent Output remove paths exceed their aggregate ceiling",
                )
            })?;
        if !matches!(shape.operation, 9 | 12)
            || shape.provider != 2
            || shape.result != ShapeResult::Scalar(-1)
            || shape.post_error != 2
            || rooted.ordinal != 0
            || rooted.root != 1
            || !psi_checked_interpreter::filesystem_root_relative_path_is_canonical(
                rooted.bytes,
                false,
            )
            || authorized.ordinal != 0
            || authorized.access != 1
            || authorized.root != 1
            || authorized.bytes != rooted.bytes
            || !only_output_absent_remove_lanes(shape)
        {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay absent Output remove is internally inconsistent",
            ));
        }
    }
    Ok(())
}

fn validate_path_metadata_shape(
    metadata_attempt: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let expected_kind = match metadata_attempt.operation {
        38 => 0,
        40 => 2,
        _ => {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay record has an unsupported source metadata operation",
            ));
        }
    };
    let [rooted] = metadata_attempt.rooted_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay source metadata has no unique rooted input",
        ));
    };
    let [authorized] = metadata_attempt.authorized_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay source metadata has no unique authorized target",
        ));
    };
    let [metadata] = metadata_attempt.metadata.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay source metadata has no unique semantic row",
        ));
    };
    let [(resolution_ordinal, resolution)] = metadata_attempt.mutable_byte_resolutions.as_slice()
    else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay source metadata has no unique mutable resolution",
        ));
    };
    let [carrier] = metadata_attempt.mutable_bytes.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay source metadata has no unique mutable carrier",
        ));
    };
    if metadata_attempt.provider != 2
        || metadata_attempt.result != ShapeResult::Scalar(0)
        || rooted.ordinal != 0
        || rooted.root != 0
        || !psi_checked_interpreter::filesystem_root_relative_path_is_canonical(rooted.bytes, false)
        || authorized.ordinal != 0
        || authorized.access != 0
        || authorized.root != 0
        || !psi_checked_interpreter::filesystem_root_relative_path_is_canonical(
            authorized.bytes,
            true,
        )
        || metadata.ordinal != 1
        || metadata.kind != expected_kind
        || *resolution_ordinal != 1
        || carrier.ordinal != 1
        || *resolution != carrier.pre
        || carrier.pre.len() != carrier.post.len()
        || carrier.post.len() < psi_checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES
        || !only_path_metadata_lanes(metadata_attempt)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay source metadata is internally inconsistent",
        ));
    }
    Ok(())
}

fn validate_open_shape(open: &AttemptShape<'_>) -> Result<u64, BuildFilesystemReplayRecordError> {
    if open.operation != 2
        || open.provider != 2
        || open.scalars.as_slice() != [(1, ShapeScalar::I32(0))]
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record is not a bounded source-read chain",
        ));
    }
    let Some(output) = open.output else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay open has no handle output",
        ));
    };
    let identity = output.identity;
    let [open_rooted] = open.rooted_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay open has no unique rooted source path",
        ));
    };
    let [open_authorized] = open.authorized_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay open has no unique authorized source path",
        ));
    };
    if output.kind != 0
        || output.source != 0
        || output.source_identity.is_some()
        || open.result != ShapeResult::Handle(identity)
        || open_rooted.ordinal != 0
        || open_rooted.root != 0
        || open_authorized.ordinal != 0
        || open_authorized.access != 0
        || open_authorized.root != 0
        || open_authorized.bytes != open_rooted.bytes
        || !only_open_lanes(open)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record has inconsistent descriptor creation",
        ));
    }
    Ok(identity)
}

fn validate_close_shape(
    close: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if close.operation != 8
        || close.provider != 2
        || close.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            }]
        || close.result != ShapeResult::Scalar(0)
        || close.retired.as_slice() != [identity]
        || !only_close_lanes(close)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record has inconsistent descriptor retirement",
        ));
    }
    Ok(())
}

fn validate_read_shape(
    read: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if read.provider != 2
        || read.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            }]
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay read has inconsistent descriptor lineage",
        ));
    }
    let ShapeResult::Scalar(read_result) = read.result else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay read has a non-scalar result",
        ));
    };
    let Ok(read_length) = u64::try_from(read_result) else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay read did not succeed",
        ));
    };
    let (requested, expected_region_kind) = match (read.operation, read.scalars.as_slice()) {
        (4, [(2, ShapeScalar::U64(requested))]) => (requested, 0),
        (
            6,
            [
                (2, ShapeScalar::U64(requested)),
                (3, ShapeScalar::I64(offset)),
            ],
        ) if *offset >= 0 => (requested, 1),
        _ => {
            return Err(BuildFilesystemReplayRecordError::new(
                "bounded replay read has no exact transfer count and positioned offset",
            ));
        }
    };
    let [region] = read.observed_regions.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay read has no unique observed region",
        ));
    };
    let [(resolution_ordinal, resolution)] = read.mutable_byte_resolutions.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay read has no unique mutable resolution",
        ));
    };
    let [carrier] = read.mutable_bytes.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay read has no unique mutable carrier",
        ));
    };
    let Ok(read_end) = usize::try_from(read_length) else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay read length exceeds this host",
        ));
    };
    if region.ordinal != 1
        || region.kind != expected_region_kind
        || region.offset != 0
        || region.length != read_length
        || *resolution_ordinal != 1
        || carrier.ordinal != 1
        || *resolution != carrier.pre
        || carrier.pre.len() != carrier.post.len()
        || read_end > carrier.post.len()
        || read_length > *requested
        || u64::try_from(carrier.post.len()).is_ok_and(|capacity| *requested > capacity)
        || carrier.pre[read_end..] != carrier.post[read_end..]
        || !only_read_lanes(read)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay read carrier is internally inconsistent",
        ));
    }
    Ok(())
}

fn validate_directory_read_shape(
    read: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let ShapeResult::Scalar(result) = read.result else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay has a non-scalar result",
        ));
    };
    let Ok(result_length) = u64::try_from(result) else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay did not succeed",
        ));
    };
    let [(2, ShapeScalar::U64(requested))] = read.scalars.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay has no exact transfer count",
        ));
    };
    let [region] = read.observed_regions.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay has no unique observed region",
        ));
    };
    let [(resolution_ordinal, resolution)] = read.mutable_byte_resolutions.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay has no unique byte resolution",
        ));
    };
    let [carrier] = read.mutable_bytes.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay has no unique byte carrier",
        ));
    };
    let [(position_resolution_ordinal, _)] = read.mutable_i64_resolutions.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay has no unique cursor resolution",
        ));
    };
    let [position] = read.mutable_i64s.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay has no unique cursor carrier",
        ));
    };
    let Ok(result_end) = usize::try_from(result_length) else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay length exceeds this host",
        ));
    };
    if read.operation != 23
        || read.provider != 2
        || read.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            }]
        || region.ordinal != 1
        || region.kind != 2
        || region.offset != 0
        || region.length != result_length
        || *resolution_ordinal != 1
        || carrier.ordinal != 1
        || resolution.len() != carrier.pre.len()
        || carrier.pre.len() != carrier.post.len()
        || result_end > carrier.post.len()
        || result_length > *requested
        || u64::try_from(carrier.post.len()).is_ok_and(|capacity| *requested > capacity)
        || carrier.pre[result_end..] != carrier.post[result_end..]
        || *position_resolution_ordinal != 3
        || position.ordinal != 3
        || !only_directory_read_lanes(read)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay directory carrier is internally inconsistent",
        ));
    }
    Ok(())
}

fn validate_descriptor_metadata_shape(
    metadata_attempt: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let [metadata] = metadata_attempt.metadata.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay descriptor metadata has no unique semantic row",
        ));
    };
    let [(resolution_ordinal, resolution)] = metadata_attempt.mutable_byte_resolutions.as_slice()
    else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay descriptor metadata has no unique mutable resolution",
        ));
    };
    let [carrier] = metadata_attempt.mutable_bytes.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay descriptor metadata has no unique mutable carrier",
        ));
    };
    if metadata_attempt.operation != 39
        || metadata_attempt.provider != 2
        || metadata_attempt.result != ShapeResult::Scalar(0)
        || metadata_attempt.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            }]
        || metadata.ordinal != 1
        || metadata.kind != 1
        || *resolution_ordinal != 1
        || carrier.ordinal != 1
        || *resolution != carrier.pre
        || carrier.pre.len() != carrier.post.len()
        || carrier.post.len() < psi_checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES
        || !only_descriptor_metadata_lanes(metadata_attempt)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay descriptor metadata is internally inconsistent",
        ));
    }
    Ok(())
}

fn common_empty_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.returned_path_count == 0
        && attempt.metadata.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.refusal_count == 0
}

fn only_path_metadata_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.scalars.is_empty()
        && attempt.inputs.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

fn only_descriptor_metadata_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.scalars.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

fn only_open_lanes(attempt: &AttemptShape<'_>) -> bool {
    common_empty_lanes(attempt)
        && attempt.observed_regions.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.inputs.is_empty()
        && attempt.retired.is_empty()
}

fn only_read_lanes(attempt: &AttemptShape<'_>) -> bool {
    common_empty_lanes(attempt)
        && attempt.rooted_paths.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
}

fn only_directory_read_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.returned_path_count == 0
        && attempt.metadata.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

fn only_close_lanes(attempt: &AttemptShape<'_>) -> bool {
    common_empty_lanes(attempt)
        && attempt.scalars.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.observed_regions.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.output.is_none()
}

fn only_output_create_lanes(attempt: &AttemptShape<'_>) -> bool {
    common_empty_lanes(attempt)
        && attempt.observed_regions.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.inputs.is_empty()
        && attempt.retired.is_empty()
}

fn only_output_absent_remove_lanes(attempt: &AttemptShape<'_>) -> bool {
    common_empty_lanes(attempt)
        && attempt.scalars.is_empty()
        && attempt.observed_regions.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.inputs.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
}

fn only_output_write_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.path_like_operands.is_empty()
        && attempt.returned_path_count == 0
        && attempt.metadata.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.refusal_count == 0
        && attempt.rooted_paths.is_empty()
        && attempt.observed_regions.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
}

fn only_output_sync_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.scalars.is_empty()
        && attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

fn only_output_set_length_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

fn only_output_set_file_permissions_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

fn only_output_set_file_times_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.scalars.is_empty()
        && attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

fn only_output_seek_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

fn record_commitment(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(COMMITMENT_DOMAIN);
    digest.update(
        u64::try_from(bytes.len())
            .expect("bounded replay bytes fit u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
    digest.finalize().into()
}

fn clone_bytes(bytes: &[u8]) -> Result<Vec<u8>, BuildFilesystemReplayRecordError> {
    let mut cloned = Vec::new();
    cloned
        .try_reserve_exact(bytes.len())
        .map_err(|_| BuildFilesystemReplayRecordError::new("replay record allocation failed"))?;
    cloned.extend_from_slice(bytes);
    Ok(cloned)
}

#[cfg(test)]
mod first_rung_validation_tests {
    use super::*;

    static METADATA_CARRIER: [u8; psi_checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES] =
        [0; psi_checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES];

    fn empty_shape(operation: u16, result: ShapeResult) -> AttemptShape<'static> {
        AttemptShape {
            operation,
            provider: 2,
            result,
            post_error: 0,
            scalars: Vec::new(),
            byte_operands: Vec::new(),
            path_like_operands: Vec::new(),
            rooted_paths: Vec::new(),
            returned_paths: Vec::new(),
            returned_path_count: 0,
            observed_regions: Vec::new(),
            metadata: Vec::new(),
            mutable_byte_resolutions: Vec::new(),
            mutable_i64_resolutions: Vec::new(),
            mutable_bytes: Vec::new(),
            mutable_i64s: Vec::new(),
            authorized_paths: Vec::new(),
            inputs: Vec::new(),
            output: None,
            retired: Vec::new(),
            refusal_count: 0,
        }
    }

    fn exact_input_output_shapes() -> Vec<AttemptShape<'static>> {
        let mut open = empty_shape(2, ShapeResult::Handle(1));
        open.scalars = vec![(1, ShapeScalar::I32(0))];
        open.rooted_paths = vec![ShapeRootedPath {
            ordinal: 0,
            root: 0,
            bytes: b"main.omg",
        }];
        open.authorized_paths = vec![ShapeAuthorizedPath {
            ordinal: 0,
            access: 0,
            root: 0,
            bytes: b"main.omg",
        }];
        open.output = Some(ShapeLogicalOutput {
            kind: 0,
            identity: 1,
            source: 0,
            source_identity: None,
        });

        let mut read = empty_shape(4, ShapeResult::Scalar(0));
        read.scalars = vec![(2, ShapeScalar::U64(0))];
        read.observed_regions = vec![ShapeObservedRegion {
            ordinal: 1,
            kind: 0,
            offset: 0,
            length: 0,
        }];
        read.mutable_byte_resolutions = vec![(1, b"")];
        read.mutable_bytes = vec![ShapeMutableBytes {
            ordinal: 1,
            pre: b"",
            post: b"",
        }];
        read.inputs = vec![ShapeLogicalInput {
            ordinal: 0,
            kind: 0,
            resolution: ShapeLogicalInputResolution::Resolved(1),
        }];

        let mut source_close = empty_shape(8, ShapeResult::Scalar(0));
        source_close.inputs = read.inputs.clone();
        source_close.retired = vec![1];

        let mut create = empty_shape(1, ShapeResult::Handle(2));
        create.scalars = vec![(
            1,
            ShapeScalar::I32(psi_checked_interpreter::FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE),
        )];
        create.rooted_paths = vec![ShapeRootedPath {
            ordinal: 0,
            root: 1,
            bytes: b"generated.omg",
        }];
        create.authorized_paths = vec![ShapeAuthorizedPath {
            ordinal: 0,
            access: 1,
            root: 1,
            bytes: b"generated.omg",
        }];
        create.output = Some(ShapeLogicalOutput {
            kind: 0,
            identity: 2,
            source: 0,
            source_identity: None,
        });

        let mut write = empty_shape(5, ShapeResult::Scalar(7));
        write.byte_operands = vec![(1, b"payload")];
        write.inputs = vec![ShapeLogicalInput {
            ordinal: 0,
            kind: 0,
            resolution: ShapeLogicalInputResolution::Resolved(2),
        }];

        let mut output_close = empty_shape(8, ShapeResult::Scalar(0));
        output_close.inputs = write.inputs.clone();
        output_close.retired = vec![2];

        vec![open, read, source_close, create, write, output_close]
    }

    fn exact_descriptor_metadata_shapes() -> Vec<AttemptShape<'static>> {
        let mut shapes = exact_input_output_shapes();
        let mut metadata = empty_shape(39, ShapeResult::Scalar(0));
        metadata.metadata = vec![ShapeMetadata {
            ordinal: 1,
            kind: 1,
            device: 1,
            mode: 0o100444,
            link_count: 1,
            inode: 2,
            user: 3,
            group: 4,
            referenced_device: 0,
            access_time: 5,
            modification_time: 6,
            change_time: 7,
            birth_time: 8,
            size: 23,
            blocks_512: 8,
            preferred_block_size: 4096,
        }];
        metadata.mutable_byte_resolutions = vec![(1, &METADATA_CARRIER)];
        metadata.mutable_bytes = vec![ShapeMutableBytes {
            ordinal: 1,
            pre: &METADATA_CARRIER,
            post: &METADATA_CARRIER,
        }];
        metadata.inputs = vec![ShapeLogicalInput {
            ordinal: 0,
            kind: 0,
            resolution: ShapeLogicalInputResolution::Resolved(1),
        }];
        shapes[1] = metadata;
        shapes
    }

    #[test]
    fn output_write_authorization_lane_rejects_during_recovery_validation() {
        let mut shapes = exact_input_output_shapes();
        assert!(validate_first_rung(&shapes).is_ok());
        shapes[4].authorized_paths.push(ShapeAuthorizedPath {
            ordinal: 0,
            access: 1,
            root: 1,
            bytes: b"generated.omg",
        });
        assert!(validate_first_rung(&shapes).is_err());
    }

    #[test]
    fn output_descriptor_overlap_rejects_during_recovery_validation() {
        let mut shapes = exact_input_output_shapes();
        shapes[3].result = ShapeResult::Handle(1);
        shapes[3].output.as_mut().unwrap().identity = 1;
        shapes[4].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(1);
        shapes[5].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(1);
        shapes[5].retired[0] = 1;
        assert!(validate_first_rung(&shapes).is_err());
    }

    #[test]
    fn positioned_output_write_requires_one_exact_nonnegative_offset() {
        let mut shapes = exact_input_output_shapes();
        shapes[4].operation = 7;
        shapes[4].scalars = vec![(2, ShapeScalar::I64(3))];
        assert!(validate_first_rung(&shapes).is_ok());

        for scalars in [
            Vec::new(),
            vec![(1, ShapeScalar::I64(3))],
            vec![(2, ShapeScalar::I64(-1))],
            vec![(2, ShapeScalar::I64(3)), (3, ShapeScalar::I64(4))],
        ] {
            let mut malformed = shapes.clone();
            malformed[4].scalars = scalars;
            assert!(validate_first_rung(&malformed).is_err());
        }

        let mut sequential_with_offset = exact_input_output_shapes();
        sequential_with_offset[4].scalars = vec![(2, ShapeScalar::I64(3))];
        assert!(validate_first_rung(&sequential_with_offset).is_err());

        let mut sparse_over_ceiling = shapes;
        sparse_over_ceiling[4].scalars = vec![(
            2,
            ShapeScalar::I64(
                i64::try_from(psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_RETAINED_BYTES)
                    .unwrap(),
            ),
        )];
        assert!(validate_first_rung(&sparse_over_ceiling).is_err());
    }

    #[test]
    fn empty_output_file_requires_exact_create_close_pair() {
        let mut shapes = exact_input_output_shapes();
        shapes.remove(4);
        assert!(validate_first_rung(&shapes).is_ok());

        let mut missing_close = shapes.clone();
        missing_close.pop();
        assert!(validate_first_rung(&missing_close).is_err());

        let mut extra_operation = shapes;
        extra_operation.insert(4, empty_shape(12, ShapeResult::Scalar(0)));
        assert!(validate_first_rung(&extra_operation).is_err());
    }

    #[test]
    fn output_sync_requires_exact_success_and_descriptor_lineage() {
        let mut shapes = exact_input_output_shapes();
        let mut sync = empty_shape(43, ShapeResult::Scalar(0));
        sync.inputs = shapes[4].inputs.clone();
        shapes.insert(4, sync);
        assert!(validate_first_rung(&shapes).is_ok());

        let mut failed = shapes.clone();
        failed[4].result = ShapeResult::Scalar(-1);
        assert!(validate_first_rung(&failed).is_err());

        let mut wrong_descriptor = shapes.clone();
        wrong_descriptor[4].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
        assert!(validate_first_rung(&wrong_descriptor).is_err());

        let mut spoofed_lane = shapes;
        spoofed_lane[4].scalars = vec![(1, ShapeScalar::I32(0))];
        assert!(validate_first_rung(&spoofed_lane).is_err());
    }

    #[test]
    fn output_duplicate_requires_exact_lineage_and_immediate_retirement() {
        let mut shapes = exact_input_output_shapes();
        let mut duplicate = empty_shape(45, ShapeResult::Handle(3));
        duplicate.inputs = shapes[4].inputs.clone();
        duplicate.output = Some(ShapeLogicalOutput {
            kind: 0,
            identity: 3,
            source: 1,
            source_identity: Some(2),
        });
        let mut duplicate_close = empty_shape(8, ShapeResult::Scalar(0));
        duplicate_close.inputs = vec![ShapeLogicalInput {
            ordinal: 0,
            kind: 0,
            resolution: ShapeLogicalInputResolution::Resolved(3),
        }];
        duplicate_close.retired = vec![3];
        shapes.insert(4, duplicate);
        shapes.insert(5, duplicate_close);
        assert!(validate_first_rung(&shapes).is_ok());

        let mut wrong_source = shapes.clone();
        wrong_source[4].output.as_mut().unwrap().source = 0;
        assert!(validate_first_rung(&wrong_source).is_err());

        let mut wrong_source_identity = shapes.clone();
        wrong_source_identity[4]
            .output
            .as_mut()
            .unwrap()
            .source_identity = Some(9);
        assert!(validate_first_rung(&wrong_source_identity).is_err());

        let mut wrong_result = shapes.clone();
        wrong_result[4].result = ShapeResult::Handle(4);
        assert!(validate_first_rung(&wrong_result).is_err());

        let mut failed = shapes.clone();
        failed[4].result = ShapeResult::Scalar(-1);
        failed[4].post_error = 9;
        assert!(validate_first_rung(&failed).is_err());

        let mut wrong_close_lineage = shapes.clone();
        wrong_close_lineage[5].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(2);
        assert!(validate_first_rung(&wrong_close_lineage).is_err());

        let mut missing_close = shapes;
        missing_close.remove(5);
        assert!(validate_first_rung(&missing_close).is_err());
    }

    #[test]
    fn output_set_length_requires_exact_nonnegative_length_and_lineage() {
        let mut shapes = exact_input_output_shapes();
        let mut set_length = empty_shape(41, ShapeResult::Scalar(0));
        set_length.scalars = vec![(1, ShapeScalar::I64(3))];
        set_length.inputs = shapes[4].inputs.clone();
        shapes.insert(5, set_length);
        assert!(validate_first_rung(&shapes).is_ok());

        let mut negative = shapes.clone();
        negative[5].scalars = vec![(1, ShapeScalar::I64(-1))];
        assert!(validate_first_rung(&negative).is_err());

        let mut wrong_ordinal = shapes.clone();
        wrong_ordinal[5].scalars = vec![(0, ShapeScalar::I64(3))];
        assert!(validate_first_rung(&wrong_ordinal).is_err());

        let mut wrong_descriptor = shapes;
        wrong_descriptor[5].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
        assert!(validate_first_rung(&wrong_descriptor).is_err());
    }

    #[test]
    fn output_set_file_permissions_requires_exact_success_mode_and_lineage() {
        let mut shapes = exact_input_output_shapes();
        let mut permissions = empty_shape(17, ShapeResult::Scalar(0));
        permissions.scalars = vec![(1, ShapeScalar::U32(0o755))];
        permissions.inputs = shapes[4].inputs.clone();
        shapes.insert(5, permissions);
        assert!(validate_first_rung(&shapes).is_ok());

        let mut failed = shapes.clone();
        failed[5].result = ShapeResult::Scalar(-1);
        assert!(validate_first_rung(&failed).is_err());

        let mut wrong_type = shapes.clone();
        wrong_type[5].scalars = vec![(1, ShapeScalar::I32(0o755))];
        assert!(validate_first_rung(&wrong_type).is_err());

        let mut wrong_ordinal = shapes.clone();
        wrong_ordinal[5].scalars = vec![(0, ShapeScalar::U32(0o755))];
        assert!(validate_first_rung(&wrong_ordinal).is_err());

        let mut wrong_descriptor = shapes;
        wrong_descriptor[5].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
        assert!(validate_first_rung(&wrong_descriptor).is_err());
    }

    #[test]
    fn output_set_file_times_requires_exact_unchanged_carrier_and_lineage() {
        const TIMES: [u8; 32] = [7; 32];
        const CHANGED_TIMES: [u8; 32] = [8; 32];
        const SHORT_TIMES: [u8; 31] = [7; 31];

        let mut shapes = exact_input_output_shapes();
        let mut times = empty_shape(42, ShapeResult::Scalar(0));
        times.mutable_byte_resolutions = vec![(1, &TIMES)];
        times.mutable_bytes = vec![ShapeMutableBytes {
            ordinal: 1,
            pre: &TIMES,
            post: &TIMES,
        }];
        times.inputs = shapes[4].inputs.clone();
        shapes.insert(5, times);
        assert!(validate_first_rung(&shapes).is_ok());

        let mut failed = shapes.clone();
        failed[5].result = ShapeResult::Scalar(-1);
        assert!(validate_first_rung(&failed).is_err());

        let mut changed_post = shapes.clone();
        changed_post[5].mutable_bytes[0].post = &CHANGED_TIMES;
        assert!(validate_first_rung(&changed_post).is_err());

        let mut wrong_ordinal = shapes.clone();
        wrong_ordinal[5].mutable_byte_resolutions[0].0 = 0;
        assert!(validate_first_rung(&wrong_ordinal).is_err());

        let mut short = shapes.clone();
        short[5].mutable_byte_resolutions[0].1 = &SHORT_TIMES;
        short[5].mutable_bytes[0].pre = &SHORT_TIMES;
        short[5].mutable_bytes[0].post = &SHORT_TIMES;
        assert!(validate_first_rung(&short).is_err());

        let mut wrong_descriptor = shapes;
        wrong_descriptor[5].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
        assert!(validate_first_rung(&wrong_descriptor).is_err());
    }

    #[test]
    fn output_seek_requires_exact_recomputed_result_and_lineage() {
        let mut shapes = exact_input_output_shapes();
        let mut seek = empty_shape(10, ShapeResult::Scalar(5));
        seek.scalars = vec![(1, ShapeScalar::I64(-2)), (2, ShapeScalar::I32(2))];
        seek.inputs = shapes[4].inputs.clone();
        shapes.insert(5, seek);
        assert!(validate_first_rung(&shapes).is_ok());

        let mut wrong_result = shapes.clone();
        wrong_result[5].result = ShapeResult::Scalar(4);
        assert!(validate_first_rung(&wrong_result).is_err());

        let mut bad_whence = shapes.clone();
        bad_whence[5].scalars[1] = (2, ShapeScalar::I32(9));
        assert!(validate_first_rung(&bad_whence).is_err());

        let mut wrong_descriptor = shapes;
        wrong_descriptor[5].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
        assert!(validate_first_rung(&wrong_descriptor).is_err());
    }

    #[test]
    fn descriptor_metadata_chain_validates_exact_kind_lineage_and_retirement() {
        let mut shapes = exact_descriptor_metadata_shapes();
        assert!(validate_first_rung(&shapes).is_ok());

        shapes[1].metadata[0].kind = 0;
        assert!(validate_first_rung(&shapes).is_err());

        let mut shapes = exact_descriptor_metadata_shapes();
        shapes[1].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
        assert!(validate_first_rung(&shapes).is_err());

        let mut shapes = exact_descriptor_metadata_shapes();
        shapes.remove(2);
        assert!(validate_first_rung(&shapes).is_err());
    }

    #[test]
    fn canonical_source_metadata_identity_round_trips_and_rejects_unknown_tags() {
        let expected = BuildCanonicalSourceMetadataIdentity::new(7, [0xa5; 32]);
        let mut encoder = Encoder::new(64);
        encoder.byte(1);
        encoder.u32(expected.policy_version());
        encoder.fixed(&expected.source_content_commitment());
        let bytes = encoder.finish().expect("encode metadata identity");
        let mut decoder = Decoder::new(&bytes, BuildFilesystemReplayRecordLimits::new(64, 1));
        assert_eq!(
            decode_canonical_source_metadata_identity(&mut decoder)
                .expect("decode metadata identity"),
            Some(expected)
        );
        decoder.finish().expect("consume exact identity bytes");

        let mut decoder = Decoder::new(&[2], BuildFilesystemReplayRecordLimits::new(1, 1));
        assert!(decode_canonical_source_metadata_identity(&mut decoder).is_err());
    }
}

struct Encoder {
    bytes: Vec<u8>,
    maximum: usize,
    exceeded: bool,
}

impl Encoder {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
            exceeded: false,
        }
    }
    fn append(&mut self, bytes: &[u8]) {
        if self.exceeded
            || self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|length| length > self.maximum)
        {
            self.exceeded = true;
            return;
        }
        if self.bytes.try_reserve(bytes.len()).is_err() {
            self.exceeded = true;
            return;
        }
        self.bytes.extend_from_slice(bytes);
    }
    fn fixed(&mut self, value: &[u8]) {
        self.append(value);
    }
    fn byte(&mut self, value: u8) {
        self.append(&[value]);
    }
    fn u16(&mut self, value: u16) {
        self.append(&value.to_le_bytes());
    }
    fn u32(&mut self, value: u32) {
        self.append(&value.to_le_bytes());
    }
    fn i32(&mut self, value: i32) {
        self.append(&value.to_le_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.append(&value.to_le_bytes());
    }
    fn i64(&mut self, value: i64) {
        self.append(&value.to_le_bytes());
    }
    fn count(&mut self, value: usize) -> Result<(), BuildFilesystemReplayRecordError> {
        self.u64(
            u64::try_from(value)
                .map_err(|_| BuildFilesystemReplayRecordError::new("replay count exceeds u64"))?,
        );
        Ok(())
    }
    fn bytes(&mut self, value: &[u8]) -> Result<(), BuildFilesystemReplayRecordError> {
        self.count(value.len())?;
        self.append(value);
        Ok(())
    }
    fn finish(self) -> Result<Vec<u8>, BuildFilesystemReplayRecordError> {
        if self.exceeded {
            Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay record exceeds its byte ceiling",
            ))
        } else {
            Ok(self.bytes)
        }
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
    limits: BuildFilesystemReplayRecordLimits,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8], limits: BuildFilesystemReplayRecordLimits) -> Self {
        Self {
            bytes,
            offset: 0,
            limits,
        }
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], BuildFilesystemReplayRecordError> {
        let end = self.offset.checked_add(length).ok_or_else(|| {
            BuildFilesystemReplayRecordError::new("filesystem replay record length overflow")
        })?;
        let value = self.bytes.get(self.offset..end).ok_or_else(|| {
            BuildFilesystemReplayRecordError::new("truncated filesystem replay record")
        })?;
        self.offset = end;
        Ok(value)
    }
    fn fixed(&mut self, expected: &[u8]) -> Result<(), BuildFilesystemReplayRecordError> {
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(BuildFilesystemReplayRecordError::new(
                "invalid filesystem replay record magic",
            ))
        }
    }
    fn byte(&mut self) -> Result<u8, BuildFilesystemReplayRecordError> {
        Ok(self.take(1)?[0])
    }
    fn tag(
        &mut self,
        maximum: u8,
        message: &'static str,
    ) -> Result<u8, BuildFilesystemReplayRecordError> {
        let value = self.byte()?;
        if value <= maximum {
            Ok(value)
        } else {
            Err(BuildFilesystemReplayRecordError::new(message))
        }
    }
    fn u16(&mut self) -> Result<u16, BuildFilesystemReplayRecordError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, BuildFilesystemReplayRecordError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn array_32(&mut self) -> Result<[u8; 32], BuildFilesystemReplayRecordError> {
        Ok(self.take(32)?.try_into().unwrap())
    }
    fn i32(&mut self) -> Result<i32, BuildFilesystemReplayRecordError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, BuildFilesystemReplayRecordError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64, BuildFilesystemReplayRecordError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn nonzero_u64(&mut self) -> Result<u64, BuildFilesystemReplayRecordError> {
        let value = self.u64()?;
        if value == 0 {
            Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay record contains a zero handle identity",
            ))
        } else {
            Ok(value)
        }
    }
    fn count(&mut self) -> Result<usize, BuildFilesystemReplayRecordError> {
        let value = usize::try_from(self.u64()?).map_err(|_| {
            BuildFilesystemReplayRecordError::new("filesystem replay count exceeds this host")
        })?;
        if value > self.limits.maximum_items_per_lane {
            Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay lane exceeds its item ceiling",
            ))
        } else {
            Ok(value)
        }
    }
    fn bytes(&mut self) -> Result<&'a [u8], BuildFilesystemReplayRecordError> {
        let length = usize::try_from(self.u64()?).map_err(|_| {
            BuildFilesystemReplayRecordError::new("filesystem replay byte length exceeds this host")
        })?;
        if length > self.limits.maximum_bytes {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay field exceeds its byte ceiling",
            ));
        }
        self.take(length)
    }
    fn finish(self) -> Result<(), BuildFilesystemReplayRecordError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay record has trailing bytes",
            ))
        }
    }
}

const fn provider_tag(value: BuildFilesystemProvider) -> u8 {
    match value {
        BuildFilesystemProvider::Virtual => 0,
        BuildFilesystemProvider::RealUnscoped => 1,
        BuildFilesystemProvider::RealScoped => 2,
    }
}
const fn access_tag(value: BuildFilesystemGrantAccess) -> u8 {
    match value {
        BuildFilesystemGrantAccess::Read => 0,
        BuildFilesystemGrantAccess::Write => 1,
    }
}
const fn root_tag(value: BuildFilesystemRoot) -> u8 {
    match value {
        BuildFilesystemRoot::Source => 0,
        BuildFilesystemRoot::Output => 1,
    }
}
const fn handle_kind_tag(value: BuildFilesystemLogicalHandleKind) -> u8 {
    match value {
        BuildFilesystemLogicalHandleKind::Descriptor => 0,
        BuildFilesystemLogicalHandleKind::Native => 1,
        BuildFilesystemLogicalHandleKind::Find => 2,
    }
}
const fn refusal_reason_tag(value: BuildFilesystemGrantRefusalReason) -> u8 {
    match value {
        BuildFilesystemGrantRefusalReason::Unresolvable => 0,
        BuildFilesystemGrantRefusalReason::OutsideGrantedRoots => 1,
        BuildFilesystemGrantRefusalReason::UnrepresentableRootedPath => 2,
        BuildFilesystemGrantRefusalReason::ObservationEvidenceLimitExceeded => 3,
    }
}
const fn returned_path_kind_tag(value: BuildFilesystemReturnedPathKind) -> u8 {
    match value {
        BuildFilesystemReturnedPathKind::ReadLinkPayload => 0,
        BuildFilesystemReturnedPathKind::CanonicalPath => 1,
        BuildFilesystemReturnedPathKind::FinalPath => 2,
    }
}
const fn returned_path_completeness_tag(value: BuildFilesystemReturnedPathCompleteness) -> u8 {
    match value {
        BuildFilesystemReturnedPathCompleteness::Complete => 0,
        BuildFilesystemReturnedPathCompleteness::LimitReached => 1,
    }
}
const fn observed_region_kind_tag(value: BuildFilesystemObservedByteRegionKind) -> u8 {
    match value {
        BuildFilesystemObservedByteRegionKind::SequentialFileRead => 0,
        BuildFilesystemObservedByteRegionKind::PositionedFileRead => 1,
        BuildFilesystemObservedByteRegionKind::DirectoryRecords => 2,
        BuildFilesystemObservedByteRegionKind::FindEntry => 3,
    }
}
const fn metadata_kind_tag(value: BuildFilesystemMetadataObservationKind) -> u8 {
    match value {
        BuildFilesystemMetadataObservationKind::FollowedPath => 0,
        BuildFilesystemMetadataObservationKind::OpenDescriptor => 1,
        BuildFilesystemMetadataObservationKind::UnfollowedFinalPath => 2,
    }
}
