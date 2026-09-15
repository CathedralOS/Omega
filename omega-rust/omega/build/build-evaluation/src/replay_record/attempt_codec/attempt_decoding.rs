//! Decoding one attempt and its ordinal byte lanes.

use crate::replay_record::BuildFilesystemReplayRecordError;
use crate::replay_record::attempt_codec::attempt_shapes::{
    AttemptShape, ShapeAuthorizedPath, ShapeLogicalInput, ShapeLogicalInputResolution,
    ShapeLogicalOutput, ShapeMetadata, ShapeMutableBytes, ShapeMutableI64, ShapeObservedRegion,
    ShapeRefusal, ShapeResult, ShapeReturnedPath, ShapeRootedPath, ShapeScalar,
};
use crate::replay_record::attempt_codec::wire::Decoder;

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
