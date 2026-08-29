use crate::{
    BuildCanonicalSourceMetadataIdentity, BuildFilesystemGrantAccess,
    BuildFilesystemGrantRefusalReason, BuildFilesystemLogicalHandleInputResolution,
    BuildFilesystemLogicalHandleKind, BuildFilesystemLogicalHandleOutputSource,
    BuildFilesystemMetadataObservationKind, BuildFilesystemObservedByteRegionKind,
    BuildFilesystemOperationAttempt, BuildFilesystemOperationResult, BuildFilesystemProvider,
    BuildFilesystemReturnedPathCompleteness, BuildFilesystemReturnedPathKind, BuildFilesystemRoot,
    BuildFilesystemScalarOperandValue, BuildObservationSummary,
};
use sha2::{Digest, Sha256};
use std::fmt;

const MAGIC: &[u8] = b"OMEGA-BUILD-FILESYSTEM-REPLAY-RECORD\0";
const COMMITMENT_DOMAIN: &[u8] = b"OMEGA-BUILD-FILESYSTEM-REPLAY-RECORD-COMMITMENT\0";
const VERSION: u16 = 10;

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
    if !summary.source_inputs_replay_verified() {
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
    if includes_output && !summary.operation_replay_verified() {
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
    match summary.included_source_paths() {
        [] => encoder.byte(0),
        [_] => encoder.byte(1),
        _ => {
            return Err(BuildFilesystemReplayRecordError::new(
                "bounded filesystem replay permits at most one included-source handoff",
            ));
        }
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

pub fn rehydrate_review_only_build_filesystem_replay_record(
    record: &ReviewOnlyBuildFilesystemReplayRecord,
    limits: BuildFilesystemReplayRecordLimits,
) -> Result<psi_checked_interpreter::FilesystemReplay, BuildFilesystemReplayRecordError> {
    let decoded = decode_shapes(record.canonical_bytes(), limits)?;
    let output_has_included_source = decoded.output_has_included_source;
    let shapes = decoded.shapes;
    let output_start = shapes
        .iter()
        .position(|shape| shape.operation == 1)
        .unwrap_or(shapes.len());
    let mut events = Vec::new();
    let mut cursor = 0;
    while cursor < output_start {
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
    let typed_record = psi_checked_interpreter::FilesystemSourceInputReplayRecord::new(events)
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay source inputs could not be rehydrated",
            )
        })?;
    if output_start == shapes.len() {
        return psi_checked_interpreter::FilesystemReplay::from_source_input_record(typed_record)
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay source inputs exceed retained replay policy",
                )
            });
    }
    let mut output_records = Vec::new();
    output_records
        .try_reserve_exact((shapes.len() - output_start) / 3)
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay output-chain allocation failed",
            )
        })?;
    for chain in shapes[output_start..].chunks_exact(3) {
        let [create, write, close] = chain else {
            unreachable!("validated receipted output is a create-write-close chain")
        };
        let Some(output) = create.output else {
            unreachable!("validated receipted output create has a descriptor")
        };
        let [rooted] = create.rooted_paths.as_slice() else {
            unreachable!("validated receipted output create has one rooted path")
        };
        let [(_, payload)] = write.byte_operands.as_slice() else {
            unreachable!("validated receipted output write has one payload")
        };
        let ShapeResult::Scalar(write_result) = write.result else {
            unreachable!("validated receipted output write returns a scalar")
        };
        output_records.push(
            psi_checked_interpreter::FilesystemOutputWriteChainReplayRecord::new(
                crate::BUILD_OUTPUT_ROOT_IDENTITY,
                clone_bytes(rooted.bytes)?,
                output.identity,
                create.post_error,
                clone_bytes(payload)?,
                write_result,
                write.post_error,
                close.post_error,
            )
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay output chain could not be rehydrated",
                )
            })?,
        );
    }
    let expected_included_source = output_has_included_source
        .then(|| {
            let output = output_records
                .first()
                .expect("validated generated-source replay has one output");
            psi_checked_interpreter::BuildIncludedSource::from_coordinate(
                crate::BUILD_OUTPUT_ROOT_IDENTITY,
                clone_bytes(output.output_relative_path())?,
                shapes.len(),
            )
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay generated-source handoff could not be rehydrated",
                )
            })
        })
        .transpose()?;
    let typed_record = psi_checked_interpreter::FilesystemInputOutputReplayRecord::new(
        typed_record,
        output_records,
        expected_included_source,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay input/output record could not be rehydrated",
        )
    })?;
    psi_checked_interpreter::FilesystemReplay::from_input_output_record(typed_record).map_err(
        |_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay input/output record exceeds retained replay policy",
            )
        },
    )
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
    output_has_included_source: bool,
    shapes: Vec<AttemptShape<'a>>,
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
    let output_has_included_source = match decoder.byte()? {
        0 => false,
        1 => true,
        _ => {
            return Err(BuildFilesystemReplayRecordError::new(
                "invalid filesystem replay included-source disposition",
            ));
        }
    };
    let attempt_count = decoder.count()?;
    if attempt_count == 0 {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded filesystem replay record must contain source-input events",
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
    let output_chain_count = shapes.iter().filter(|shape| shape.operation == 1).count();
    if output_has_included_source && output_chain_count != 1 {
        return Err(BuildFilesystemReplayRecordError::new(
            "generated-source filesystem replay requires exactly one Output chain",
        ));
    }
    Ok(DecodedReplay {
        canonical_source_metadata_identity,
        output_has_included_source,
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
    resolution: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShapeLogicalOutput {
    kind: u8,
    identity: u64,
    source: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShapeRootedPath<'a> {
    ordinal: u8,
    root: u8,
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
    path_like_operand_count: usize,
    rooted_paths: Vec<ShapeRootedPath<'a>>,
    returned_path_count: usize,
    observed_regions: Vec<ShapeObservedRegion>,
    metadata: Vec<ShapeMetadata>,
    mutable_byte_resolutions: Vec<(u8, &'a [u8])>,
    mutable_i64_resolution_count: usize,
    mutable_bytes: Vec<ShapeMutableBytes<'a>>,
    mutable_i64_count: usize,
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
    let path_like_operand_count = decode_ordinal_bytes_lane(decoder)?;

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
    for _ in 0..returned_path_count {
        let _ = decoder.byte()?;
        let _ = decoder.tag(2, "invalid returned-path kind tag")?;
        let _ = decoder.tag(1, "invalid returned-path completeness tag")?;
        let _ = decoder.bytes()?;
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
    let mutable_i64_resolution_count = decoder.count()?;
    for _ in 0..mutable_i64_resolution_count {
        let _ = decoder.byte()?;
        let _ = decoder.i64()?;
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
    let mutable_i64_count = decoder.count()?;
    for _ in 0..mutable_i64_count {
        let _ = decoder.byte()?;
        let _ = decoder.i64()?;
        let _ = decoder.i64()?;
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
            0 => Some(decoder.nonzero_u64()?),
            1 | 2 => None,
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
            if source != 0 {
                let _ = decoder.nonzero_u64()?;
            }
            Some(ShapeLogicalOutput {
                kind,
                identity,
                source,
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
        path_like_operand_count,
        rooted_paths,
        returned_path_count,
        observed_regions,
        metadata,
        mutable_byte_resolutions,
        mutable_i64_resolution_count,
        mutable_bytes,
        mutable_i64_count,
        authorized_paths,
        inputs,
        output,
        retired,
        refusal_count,
    })
}

fn decode_ordinal_bytes_lane(
    decoder: &mut Decoder<'_>,
) -> Result<usize, BuildFilesystemReplayRecordError> {
    let count = decoder.count()?;
    for _ in 0..count {
        let _ = decoder.byte()?;
        let _ = decoder.bytes()?;
    }
    Ok(count)
}

fn validate_first_rung(
    shapes: &[AttemptShape<'_>],
) -> Result<(), BuildFilesystemReplayRecordError> {
    let mut cursor = 0;
    let mut identities = Vec::new();
    let mut event_count = 0;
    while cursor < shapes.len() {
        if shapes[cursor].operation == 1 {
            break;
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
    if event_count == 0 {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay contains no source-input events",
        ));
    }
    if cursor < shapes.len() {
        let output_shapes = &shapes[cursor..];
        if output_shapes.len() % 3 != 0 {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output must contain complete create-write-close chains",
            ));
        }
        let mut output_paths = Vec::new();
        output_paths
            .try_reserve_exact(output_shapes.len() / 3)
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay output-path allocation failed",
                )
            })?;
        for chain in output_shapes.chunks_exact(3) {
            let [create, write, close] = chain else {
                unreachable!("exact chunks have three output operations")
            };
            validate_output_write_chain(create, write, close)?;
            let output = create
                .output
                .expect("validated output create has a descriptor");
            let rooted = create
                .rooted_paths
                .first()
                .expect("validated output create has a rooted path");
            if identities.contains(&output.identity) {
                return Err(BuildFilesystemReplayRecordError::new(
                    "filesystem replay Output descriptor overlaps another descriptor",
                ));
            }
            identities.push(output.identity);
            if output_paths.contains(&rooted.bytes) {
                return Err(BuildFilesystemReplayRecordError::new(
                    "filesystem replay Output path appears more than once",
                ));
            }
            output_paths.push(rooted.bytes);
        }
    }
    Ok(())
}

fn validate_output_write_chain(
    create: &AttemptShape<'_>,
    write: &AttemptShape<'_>,
    close: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
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
        || rooted.bytes.contains(&b'/')
        || !psi_checked_interpreter::filesystem_root_relative_path_is_canonical(rooted.bytes, false)
        || authorized.ordinal != 0
        || authorized.access != 1
        || authorized.root != 1
        || authorized.bytes != rooted.bytes
        || output.kind != 0
        || output.source != 0
        || !only_output_create_lanes(create)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output create is internally inconsistent",
        ));
    }

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
    if write.operation != 5
        || write.provider != 2
        || write.result != ShapeResult::Scalar(payload_length)
        || write.post_error != 0
        || *payload_ordinal != 1
        || *write_input
            != (ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: Some(output.identity),
            })
        || !only_output_write_lanes(write)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output write is internally inconsistent",
        ));
    }
    validate_close_shape(close, output.identity)?;
    if close.post_error != 0 {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output close changed the post-operation error state",
        ));
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
                resolution: Some(identity),
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
                resolution: Some(identity),
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
                resolution: Some(identity),
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
        && attempt.path_like_operand_count == 0
        && attempt.returned_path_count == 0
        && attempt.metadata.is_empty()
        && attempt.mutable_i64_resolution_count == 0
        && attempt.mutable_i64_count == 0
        && attempt.refusal_count == 0
}

fn only_path_metadata_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operand_count == 0
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.mutable_i64_resolution_count == 0
        && attempt.mutable_i64_count == 0
        && attempt.scalars.is_empty()
        && attempt.inputs.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

fn only_descriptor_metadata_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operand_count == 0
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.mutable_i64_resolution_count == 0
        && attempt.mutable_i64_count == 0
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

fn only_output_write_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.path_like_operand_count == 0
        && attempt.returned_path_count == 0
        && attempt.metadata.is_empty()
        && attempt.mutable_i64_resolution_count == 0
        && attempt.mutable_i64_count == 0
        && attempt.refusal_count == 0
        && attempt.scalars.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.observed_regions.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
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
            path_like_operand_count: 0,
            rooted_paths: Vec::new(),
            returned_path_count: 0,
            observed_regions: Vec::new(),
            metadata: Vec::new(),
            mutable_byte_resolutions: Vec::new(),
            mutable_i64_resolution_count: 0,
            mutable_bytes: Vec::new(),
            mutable_i64_count: 0,
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
            resolution: Some(1),
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
        });

        let mut write = empty_shape(5, ShapeResult::Scalar(7));
        write.byte_operands = vec![(1, b"payload")];
        write.inputs = vec![ShapeLogicalInput {
            ordinal: 0,
            kind: 0,
            resolution: Some(2),
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
            resolution: Some(1),
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
        shapes[4].inputs[0].resolution = Some(1);
        shapes[5].inputs[0].resolution = Some(1);
        shapes[5].retired[0] = 1;
        assert!(validate_first_rung(&shapes).is_err());
    }

    #[test]
    fn descriptor_metadata_chain_validates_exact_kind_lineage_and_retirement() {
        let mut shapes = exact_descriptor_metadata_shapes();
        assert!(validate_first_rung(&shapes).is_ok());

        shapes[1].metadata[0].kind = 0;
        assert!(validate_first_rung(&shapes).is_err());

        let mut shapes = exact_descriptor_metadata_shapes();
        shapes[1].inputs[0].resolution = Some(9);
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
