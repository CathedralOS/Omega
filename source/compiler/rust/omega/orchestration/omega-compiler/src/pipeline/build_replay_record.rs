use super::build_config::{
    BuildFilesystemGrantAccess, BuildFilesystemGrantRefusalReason,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutputSource, BuildFilesystemMetadataObservationKind,
    BuildFilesystemObservedByteRegionKind, BuildFilesystemOperationAttempt,
    BuildFilesystemOperationResult, BuildFilesystemProvider,
    BuildFilesystemReturnedPathCompleteness, BuildFilesystemReturnedPathKind, BuildFilesystemRoot,
    BuildFilesystemScalarOperandValue, BuildObservationSummary,
};
use sha2::{Digest, Sha256};
use std::fmt;

const MAGIC: &[u8] = b"OMEGA-BUILD-FILESYSTEM-REPLAY-RECORD\0";
const COMMITMENT_DOMAIN: &[u8] = b"OMEGA-BUILD-FILESYSTEM-REPLAY-RECORD-COMMITMENT\0";
const VERSION: u16 = 2;

/// Resource ceilings for compiler-owned recovery of one partial filesystem
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
}

impl ReviewOnlyBuildFilesystemReplayRecord {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn commitment(&self) -> [u8; 32] {
        self.commitment
    }
}

/// Capture the exact operation record only after the compiler has completed
/// the bounded provider-free replay. A false replay fact produces no record.
pub fn capture_verified_build_filesystem_replay_record(
    summary: &BuildObservationSummary,
    limits: BuildFilesystemReplayRecordLimits,
) -> Result<Option<ReviewOnlyBuildFilesystemReplayRecord>, BuildFilesystemReplayRecordError> {
    if !summary.open_read_close_replay_verified() {
        return Ok(None);
    }
    let mut encoder = Encoder::new(limits.maximum_bytes);
    encoder.fixed(MAGIC);
    encoder.u16(VERSION);
    encoder.u32(summary.schema_version());
    encoder.u32(summary.filesystem_operation_schema_version());
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
    decode_shapes(bytes, limits)?;
    let canonical_bytes = clone_bytes(bytes)?;
    Ok(ReviewOnlyBuildFilesystemReplayRecord {
        commitment: record_commitment(&canonical_bytes),
        canonical_bytes,
    })
}

pub(super) fn rehydrate_review_only_build_filesystem_replay_record(
    record: &ReviewOnlyBuildFilesystemReplayRecord,
    limits: BuildFilesystemReplayRecordLimits,
) -> Result<psi_checked_interpreter::FilesystemReplay, BuildFilesystemReplayRecordError> {
    let shapes = decode_shapes(record.canonical_bytes(), limits)?;
    let [open, read, close] = shapes.as_slice() else {
        unreachable!("validated bounded replay has exactly three attempts")
    };
    let ShapeResult::Handle(logical_handle_identity) = open.result else {
        unreachable!("validated bounded replay open returns a handle")
    };
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
    let [source_path] = open.rooted_paths.as_slice() else {
        unreachable!("validated bounded replay open has one rooted path")
    };
    let [(1, mutable_resolution)] = read.mutable_byte_resolutions.as_slice() else {
        unreachable!("validated bounded replay read has one mutable resolution")
    };
    let [mutable_carrier] = read.mutable_bytes.as_slice() else {
        unreachable!("validated bounded replay read has one mutable carrier")
    };
    let typed_record = psi_checked_interpreter::FilesystemOpenReadCloseReplayRecord::new(
        super::build_config::BUILD_SOURCE_ROOT_IDENTITY,
        clone_bytes(source_path.bytes)?,
        logical_handle_identity,
        open.post_error,
        read_kind,
        *requested_count,
        read_result,
        read.post_error,
        clone_bytes(mutable_resolution)?,
        clone_bytes(mutable_carrier.pre)?,
        clone_bytes(mutable_carrier.post)?,
        close.post_error,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new("filesystem replay record could not be rehydrated")
    })?;
    Ok(psi_checked_interpreter::FilesystemReplay::from_open_read_close_record(typed_record))
}

fn decode_shapes(
    bytes: &[u8],
    limits: BuildFilesystemReplayRecordLimits,
) -> Result<Vec<AttemptShape<'_>>, BuildFilesystemReplayRecordError> {
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
    if decoder.u32()? != super::build_config::BUILD_OBSERVATION_SCHEMA_VERSION
        || decoder.u32()? != psi_checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "unsupported filesystem replay semantic schema",
        ));
    }
    let attempt_count = decoder.count()?;
    if attempt_count != 3 {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded filesystem replay record must contain three attempts",
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
    Ok(shapes)
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
    byte_operand_count: usize,
    path_like_operand_count: usize,
    rooted_paths: Vec<ShapeRootedPath<'a>>,
    returned_path_count: usize,
    observed_regions: Vec<ShapeObservedRegion>,
    metadata_count: usize,
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

    let byte_operand_count = decode_ordinal_bytes_lane(decoder)?;
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
    let metadata_count = decoder.count()?;
    for _ in 0..metadata_count {
        let _ = decoder.byte()?;
        let _ = decoder.tag(2, "invalid metadata-observation tag")?;
        let _ = decoder.u64()?;
        let _ = decoder.u32()?;
        let _ = decoder.u64()?;
        let _ = decoder.u64()?;
        let _ = decoder.u32()?;
        let _ = decoder.u32()?;
        let _ = decoder.u64()?;
        for _ in 0..5 {
            let _ = decoder.i64()?;
        }
        let _ = decoder.u64()?;
        let _ = decoder.u64()?;
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
        byte_operand_count,
        path_like_operand_count,
        rooted_paths,
        returned_path_count,
        observed_regions,
        metadata_count,
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
    let [open, read, close] = shapes else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay shape is incomplete",
        ));
    };
    let operations = [open.operation, read.operation, close.operation];
    if (operations != [2, 4, 8] && operations != [2, 6, 8])
        || [open.provider, read.provider, close.provider] != [2, 2, 2]
        || open.scalars.as_slice() != [(1, ShapeScalar::I32(0))]
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record is not the bounded source-read chain",
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
        || read.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: Some(identity),
            }]
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
            "filesystem replay record has inconsistent descriptor lineage",
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

fn common_empty_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operand_count == 0
        && attempt.path_like_operand_count == 0
        && attempt.returned_path_count == 0
        && attempt.metadata_count == 0
        && attempt.mutable_i64_resolution_count == 0
        && attempt.mutable_i64_count == 0
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
