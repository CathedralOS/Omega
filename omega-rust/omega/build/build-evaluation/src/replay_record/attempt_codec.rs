//! The wire encoding of one replay attempt: the shape structs, the byte
//! encoder and decoder, and the tag values every lane carries.

use crate::replay_record::{BuildFilesystemReplayRecordError, BuildFilesystemReplayRecordLimits};
use crate::{
    BuildFilesystemGrantAccess, BuildFilesystemGrantRefusalReason,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutputSource, BuildFilesystemMetadataObservationKind,
    BuildFilesystemObservedByteRegionKind, BuildFilesystemOperationAttempt,
    BuildFilesystemOperationObservationClass, BuildFilesystemOperationResult,
    BuildFilesystemProvider, BuildFilesystemReturnedPathCompleteness,
    BuildFilesystemReturnedPathKind, BuildFilesystemRoot, BuildFilesystemScalarOperandValue,
    BuildReplayActivation,
};

pub(crate) fn encode_replay_activation(
    encoder: &mut Encoder,
    activation: BuildReplayActivation,
) -> Result<(), BuildFilesystemReplayRecordError> {
    match activation.root_package_identity() {
        None => encoder.byte(0),
        Some(identity) => {
            encoder.byte(1);
            encoder.fixed(&identity.digest());
        }
    }
    encoder.byte(match activation.root_role() {
        None => 0,
        Some(package_compilation::BuildDeclarationKind::Package) => 1,
        Some(package_compilation::BuildDeclarationKind::Application) => 2,
        Some(package_compilation::BuildDeclarationKind::Workspace) => 3,
    });
    match activation.selected_target_profile() {
        None => encoder.byte(0),
        Some(profile) => {
            encoder.byte(1);
            encoder.bytes(profile.target_name().as_bytes())?;
        }
    }
    Ok(())
}

pub(crate) fn decode_replay_activation(
    decoder: &mut Decoder<'_>,
) -> Result<BuildReplayActivation, BuildFilesystemReplayRecordError> {
    let root_package_identity = match decoder.byte()? {
        0 => None,
        1 => Some(
            semantic_vocabulary::PackageKeyIdentity::from_digest(decoder.array_32()?).ok_or_else(
                || {
                    BuildFilesystemReplayRecordError::new(
                        "invalid replay activation root package identity",
                    )
                },
            )?,
        ),
        _ => {
            return Err(BuildFilesystemReplayRecordError::new(
                "invalid replay activation root package tag",
            ));
        }
    };
    let root_role = match decoder.byte()? {
        0 => None,
        1 => Some(package_compilation::BuildDeclarationKind::Package),
        2 => Some(package_compilation::BuildDeclarationKind::Application),
        3 => Some(package_compilation::BuildDeclarationKind::Workspace),
        _ => {
            return Err(BuildFilesystemReplayRecordError::new(
                "invalid replay activation root role tag",
            ));
        }
    };
    let selected_target_profile = match decoder.byte()? {
        0 => None,
        1 => {
            let name = std::str::from_utf8(decoder.bytes()?).map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "invalid replay activation target profile encoding",
                )
            })?;
            Some(
                target::TargetProfile::from_canonical_target_name(name).map_err(|_| {
                    BuildFilesystemReplayRecordError::new(
                        "invalid replay activation target profile",
                    )
                })?,
            )
        }
        _ => {
            return Err(BuildFilesystemReplayRecordError::new(
                "invalid replay activation target profile tag",
            ));
        }
    };
    Ok(BuildReplayActivation {
        root_package_identity,
        root_role,
        selected_target_profile,
    })
}

pub(crate) fn encode_attempt(
    encoder: &mut Encoder,
    attempt: &BuildFilesystemOperationAttempt,
) -> Result<(), BuildFilesystemReplayRecordError> {
    encoder.u16(attempt.operation_tag());
    encoder.byte(provider_tag(attempt.provider()));
    encoder.byte(match attempt.observation_class() {
        BuildFilesystemOperationObservationClass::Receipted => 0,
        BuildFilesystemOperationObservationClass::Volatile => 1,
    });
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
pub(crate) enum ShapeResult {
    Scalar(i64),
    Handle(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShapeScalar {
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeLogicalInput {
    pub(crate) ordinal: u8,
    pub(crate) kind: u8,
    pub(crate) resolution: ShapeLogicalInputResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShapeLogicalInputResolution {
    Resolved(u64),
    Null,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeLogicalOutput {
    pub(crate) kind: u8,
    pub(crate) identity: u64,
    pub(crate) source: u8,
    pub(crate) source_identity: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeRootedPath<'a> {
    pub(crate) ordinal: u8,
    pub(crate) root: u8,
    pub(crate) bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeReturnedPath<'a> {
    pub(crate) ordinal: u8,
    pub(crate) kind: u8,
    pub(crate) completeness: u8,
    pub(crate) bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeObservedRegion {
    pub(crate) ordinal: u8,
    pub(crate) kind: u8,
    pub(crate) offset: u64,
    pub(crate) length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeMetadata {
    pub(crate) ordinal: u8,
    pub(crate) kind: u8,
    pub(crate) device: u64,
    pub(crate) mode: u32,
    pub(crate) link_count: u64,
    pub(crate) inode: u64,
    pub(crate) user: u32,
    pub(crate) group: u32,
    pub(crate) referenced_device: u64,
    pub(crate) access_time: i64,
    pub(crate) modification_time: i64,
    pub(crate) change_time: i64,
    pub(crate) birth_time: i64,
    pub(crate) size: i64,
    pub(crate) blocks_512: u64,
    pub(crate) preferred_block_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeMutableBytes<'a> {
    pub(crate) ordinal: u8,
    pub(crate) pre: &'a [u8],
    pub(crate) post: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeMutableI64 {
    pub(crate) ordinal: u8,
    pub(crate) pre: i64,
    pub(crate) post: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeAuthorizedPath<'a> {
    pub(crate) ordinal: u8,
    pub(crate) access: u8,
    pub(crate) root: u8,
    pub(crate) bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeRefusal {
    pub(crate) ordinal: u8,
    pub(crate) access: u8,
    pub(crate) reason: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttemptShape<'a> {
    pub(crate) operation: u16,
    pub(crate) provider: u8,
    pub(crate) result: ShapeResult,
    pub(crate) post_error: i32,
    pub(crate) scalars: Vec<(u8, ShapeScalar)>,
    pub(crate) byte_operands: Vec<(u8, &'a [u8])>,
    pub(crate) path_like_operands: Vec<(u8, &'a [u8])>,
    pub(crate) rooted_paths: Vec<ShapeRootedPath<'a>>,
    pub(crate) returned_paths: Vec<ShapeReturnedPath<'a>>,
    pub(crate) returned_path_count: usize,
    pub(crate) observed_regions: Vec<ShapeObservedRegion>,
    pub(crate) metadata: Vec<ShapeMetadata>,
    pub(crate) mutable_byte_resolutions: Vec<(u8, &'a [u8])>,
    pub(crate) mutable_i64_resolutions: Vec<(u8, i64)>,
    pub(crate) mutable_bytes: Vec<ShapeMutableBytes<'a>>,
    pub(crate) mutable_i64s: Vec<ShapeMutableI64>,
    pub(crate) authorized_paths: Vec<ShapeAuthorizedPath<'a>>,
    pub(crate) inputs: Vec<ShapeLogicalInput>,
    pub(crate) output: Option<ShapeLogicalOutput>,
    pub(crate) retired: Vec<u64>,
    pub(crate) refusal_count: usize,
    pub(crate) refusals: Vec<ShapeRefusal>,
}

pub(crate) fn decode_attempt<'a>(
    decoder: &mut Decoder<'a>,
) -> Result<AttemptShape<'a>, BuildFilesystemReplayRecordError> {
    let operation = decoder.u16()?;
    let provider = decoder.tag(2, "invalid filesystem provider tag")?;
    if decoder.tag(1, "invalid filesystem operation observation class tag")? != 0 {
        return Err(BuildFilesystemReplayRecordError::new(
            "volatile filesystem operation cannot appear in a replay record",
        ));
    }
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
    let mut refusals = Vec::new();
    refusals.try_reserve_exact(refusal_count).map_err(|_| {
        BuildFilesystemReplayRecordError::new("replay grant-refusal allocation failed")
    })?;
    for _ in 0..refusal_count {
        refusals.push(ShapeRefusal {
            ordinal: decoder.byte()?,
            access: decoder.tag(1, "invalid filesystem grant-access tag")?,
            reason: decoder.tag(3, "invalid filesystem grant-refusal tag")?,
        });
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
        refusals,
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

pub(crate) struct Encoder {
    bytes: Vec<u8>,
    maximum: usize,
    exceeded: bool,
}

impl Encoder {
    pub(crate) fn new(maximum: usize) -> Self {
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
    pub(crate) fn fixed(&mut self, value: &[u8]) {
        self.append(value);
    }
    pub(crate) fn byte(&mut self, value: u8) {
        self.append(&[value]);
    }
    pub(crate) fn u16(&mut self, value: u16) {
        self.append(&value.to_le_bytes());
    }
    pub(crate) fn u32(&mut self, value: u32) {
        self.append(&value.to_le_bytes());
    }
    fn i32(&mut self, value: i32) {
        self.append(&value.to_le_bytes());
    }
    pub(crate) fn u64(&mut self, value: u64) {
        self.append(&value.to_le_bytes());
    }
    fn i64(&mut self, value: i64) {
        self.append(&value.to_le_bytes());
    }
    pub(crate) fn count(&mut self, value: usize) -> Result<(), BuildFilesystemReplayRecordError> {
        self.u64(
            u64::try_from(value)
                .map_err(|_| BuildFilesystemReplayRecordError::new("replay count exceeds u64"))?,
        );
        Ok(())
    }
    pub(crate) fn bytes(&mut self, value: &[u8]) -> Result<(), BuildFilesystemReplayRecordError> {
        self.count(value.len())?;
        self.append(value);
        Ok(())
    }
    pub(crate) fn finish(self) -> Result<Vec<u8>, BuildFilesystemReplayRecordError> {
        if self.exceeded {
            Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay record exceeds its byte ceiling",
            ))
        } else {
            Ok(self.bytes)
        }
    }
}

pub(crate) struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
    limits: BuildFilesystemReplayRecordLimits,
}

impl<'a> Decoder<'a> {
    pub(crate) const fn new(bytes: &'a [u8], limits: BuildFilesystemReplayRecordLimits) -> Self {
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
    pub(crate) fn fixed(
        &mut self,
        expected: &[u8],
    ) -> Result<(), BuildFilesystemReplayRecordError> {
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(BuildFilesystemReplayRecordError::new(
                "invalid filesystem replay record magic",
            ))
        }
    }
    pub(crate) fn byte(&mut self) -> Result<u8, BuildFilesystemReplayRecordError> {
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
    pub(crate) fn u16(&mut self) -> Result<u16, BuildFilesystemReplayRecordError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub(crate) fn u32(&mut self) -> Result<u32, BuildFilesystemReplayRecordError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub(crate) fn array_32(&mut self) -> Result<[u8; 32], BuildFilesystemReplayRecordError> {
        Ok(self.take(32)?.try_into().unwrap())
    }
    fn i32(&mut self) -> Result<i32, BuildFilesystemReplayRecordError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub(crate) fn u64(&mut self) -> Result<u64, BuildFilesystemReplayRecordError> {
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
    pub(crate) fn count(&mut self) -> Result<usize, BuildFilesystemReplayRecordError> {
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
    pub(crate) fn bytes(&mut self) -> Result<&'a [u8], BuildFilesystemReplayRecordError> {
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
    pub(crate) fn finish(self) -> Result<(), BuildFilesystemReplayRecordError> {
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

#[cfg(test)]
mod first_rung_validation_tests {
    use super::{
        AttemptShape, BuildFilesystemReplayRecordLimits, Decoder, Encoder, ShapeAuthorizedPath,
        ShapeLogicalInput, ShapeLogicalInputResolution, ShapeLogicalOutput, ShapeMetadata,
        ShapeMutableBytes, ShapeObservedRegion, ShapeResult, ShapeReturnedPath, ShapeRootedPath,
        ShapeScalar,
    };
    use crate::BuildCanonicalSourceMetadataIdentity;
    use crate::replay_record::rehydration::decode_canonical_source_metadata_identity;
    use crate::replay_record::shape_validation::validate_first_rung;

    static METADATA_CARRIER: [u8; checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES] =
        [0; checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES];

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
            refusals: Vec::new(),
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
            ShapeScalar::I32(checked_interpreter::FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE),
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
                i64::try_from(checked_interpreter::MAX_FILESYSTEM_REPLAY_RETAINED_BYTES).unwrap(),
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

    static NATIVE_QUERY_FINAL_PATH: &[u8] = b"C:\\pkg\\main.omg";
    static NATIVE_QUERY_CARRIER: [u8; 16] = [0; 16];
    static NATIVE_QUERY_POST: &[u8] = b"C:\\pkg\\main.omg\0";

    fn native_query_chain_shapes() -> Vec<AttemptShape<'static>> {
        let mut open = empty_shape(28, ShapeResult::Handle(7));
        open.scalars = vec![
            (1, ShapeScalar::U32(0)),
            (2, ShapeScalar::U32(0x7)),
            (3, ShapeScalar::I64(0)),
            (4, ShapeScalar::U32(3)),
            (5, ShapeScalar::U32(0x0200_0000)),
        ];
        open.rooted_paths = vec![ShapeRootedPath {
            ordinal: 0,
            root: 0,
            bytes: b"pkg/main.omg",
        }];
        open.authorized_paths = vec![ShapeAuthorizedPath {
            ordinal: 0,
            access: 0,
            root: 0,
            bytes: b"pkg/main.omg",
        }];
        open.inputs = vec![ShapeLogicalInput {
            ordinal: 6,
            kind: 1,
            resolution: ShapeLogicalInputResolution::Null,
        }];
        open.output = Some(ShapeLogicalOutput {
            kind: 1,
            identity: 7,
            source: 0,
            source_identity: None,
        });

        let mut query = empty_shape(31, ShapeResult::Scalar(15));
        query.scalars = vec![(2, ShapeScalar::U64(16)), (3, ShapeScalar::U32(0))];
        query.returned_paths = vec![ShapeReturnedPath {
            ordinal: 1,
            kind: 2,
            completeness: 0,
            bytes: NATIVE_QUERY_FINAL_PATH,
        }];
        query.mutable_byte_resolutions = vec![(1, &NATIVE_QUERY_CARRIER)];
        query.mutable_bytes = vec![ShapeMutableBytes {
            ordinal: 1,
            pre: &NATIVE_QUERY_CARRIER,
            post: NATIVE_QUERY_POST,
        }];
        query.inputs = vec![ShapeLogicalInput {
            ordinal: 0,
            kind: 1,
            resolution: ShapeLogicalInputResolution::Resolved(7),
        }];

        let mut close = empty_shape(29, ShapeResult::Scalar(1));
        close.inputs = query.inputs.clone();
        close.retired = vec![7];

        vec![open, query, close]
    }

    #[test]
    fn native_query_chain_validates_exact_acquisition_queries_and_release() {
        let shapes = native_query_chain_shapes();
        assert!(validate_first_rung(&shapes).is_ok());

        // A native-handle failure suffix composes after the closed chain.
        let mut with_failure = native_query_chain_shapes();
        let mut failure = empty_shape(29, ShapeResult::Scalar(0));
        failure.post_error = 6;
        failure.inputs = vec![ShapeLogicalInput {
            ordinal: 0,
            kind: 1,
            resolution: ShapeLogicalInputResolution::Unknown,
        }];
        with_failure.push(failure);
        assert!(validate_first_rung(&with_failure).is_ok());

        // The constrained acquisition contract admits no Output root, no
        // write authorization, and no deferred-deletion flag.
        let mut wrong_root = shapes.clone();
        wrong_root[0].rooted_paths[0].root = 1;
        assert!(validate_first_rung(&wrong_root).is_err());

        let mut wrong_access = shapes.clone();
        wrong_access[0].authorized_paths[0].access = 1;
        assert!(validate_first_rung(&wrong_access).is_err());

        let mut delete_on_close = shapes.clone();
        delete_on_close[0].scalars[4] = (5, ShapeScalar::U32(0x0600_0000));
        assert!(validate_first_rung(&delete_on_close).is_err());

        let mut descriptor_output = shapes.clone();
        descriptor_output[0].output.as_mut().unwrap().kind = 0;
        assert!(validate_first_rung(&descriptor_output).is_err());

        let mut borrowed_template = shapes.clone();
        borrowed_template[0].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
        assert!(validate_first_rung(&borrowed_template).is_err());

        // A chain needs at least one final-path observation on the identity.
        let mut error_only = vec![
            shapes[0].clone(),
            empty_shape(35, ShapeResult::Scalar(0)),
            shapes[2].clone(),
        ];
        assert!(validate_first_rung(&error_only).is_err());
        error_only[1].post_error = 5;
        error_only[1].result = ShapeResult::Scalar(5);
        assert!(validate_first_rung(&error_only).is_err());

        let mut substituted = shapes.clone();
        substituted[1].inputs[0].resolution = ShapeLogicalInputResolution::Resolved(9);
        assert!(validate_first_rung(&substituted).is_err());

        let mut wrong_kind = shapes.clone();
        wrong_kind[1].inputs[0].kind = 0;
        assert!(validate_first_rung(&wrong_kind).is_err());

        let mut wrong_result = shapes.clone();
        wrong_result[1].result = ShapeResult::Scalar(16);
        assert!(validate_first_rung(&wrong_result).is_err());

        static TAMPERED_POST: &[u8] = b"C:\\pkg\\main.omg\x01";
        let mut tampered_post = shapes.clone();
        tampered_post[1].mutable_bytes[0].post = TAMPERED_POST;
        assert!(validate_first_rung(&tampered_post).is_err());

        let mut failed_close = shapes.clone();
        failed_close[2].result = ShapeResult::Scalar(0);
        failed_close[2].post_error = 6;
        assert!(validate_first_rung(&failed_close).is_err());

        let mut unretired = shapes.clone();
        unretired[2].retired.clear();
        assert!(validate_first_rung(&unretired).is_err());

        let mut missing_close = shapes.clone();
        missing_close.pop();
        assert!(validate_first_rung(&missing_close).is_err());

        let mut late_use = shapes.clone();
        late_use.push(empty_shape(35, ShapeResult::Scalar(0)));
        assert!(validate_first_rung(&late_use).is_err());

        let mut reused_identity = native_query_chain_shapes();
        reused_identity.extend(native_query_chain_shapes());
        assert!(validate_first_rung(&reused_identity).is_err());
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
