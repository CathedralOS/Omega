//! Path metadata, open, close, read, directory read and descriptor
//! metadata shapes.

use crate::evidence::replay_record::BuildFilesystemReplayRecordError;
use crate::evidence::replay_record::attempt_codec::{
    AttemptShape, ShapeLogicalInput, ShapeLogicalInputResolution, ShapeResult, ShapeScalar,
};
use crate::evidence::replay_record::lane_selection::{
    only_close_lanes, only_descriptor_metadata_lanes, only_directory_read_lanes, only_open_lanes,
    only_path_metadata_lanes, only_read_lanes,
};

pub(crate) fn validate_path_metadata_shape(
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
        || !checked_interpreter::filesystem_root_relative_path_is_canonical(rooted.bytes, false)
        || authorized.ordinal != 0
        || authorized.access != 0
        || authorized.root != 0
        || !checked_interpreter::filesystem_root_relative_path_is_canonical(authorized.bytes, true)
        || metadata.ordinal != 1
        || metadata.kind != expected_kind
        || *resolution_ordinal != 1
        || carrier.ordinal != 1
        || *resolution != carrier.pre
        || carrier.pre.len() != carrier.post.len()
        || carrier.post.len() < checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES
        || !only_path_metadata_lanes(metadata_attempt)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay source metadata is internally inconsistent",
        ));
    }
    Ok(())
}

pub(crate) fn validate_open_shape(
    open: &AttemptShape<'_>,
) -> Result<u64, BuildFilesystemReplayRecordError> {
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

pub(crate) fn validate_close_shape(
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

pub(crate) fn validate_read_shape(
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

pub(crate) fn validate_directory_read_shape(
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

pub(crate) fn validate_descriptor_metadata_shape(
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
        || carrier.post.len() < checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES
        || !only_descriptor_metadata_lanes(metadata_attempt)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay descriptor metadata is internally inconsistent",
        ));
    }
    Ok(())
}
