//! Native query open, final path query, error observation and handle
//! close shapes.

use crate::replay_record::BuildFilesystemReplayRecordError;
use crate::replay_record::attempt_codec::{
    AttemptShape, ShapeLogicalInput, ShapeLogicalInputResolution, ShapeResult, ShapeScalar,
};
use crate::replay_record::lane_selection::{
    only_close_lanes, only_native_error_observation_lanes, only_native_final_path_query_lanes,
    only_native_query_open_lanes,
};

/// Identity acquired by one constrained Source `open_path_handle` (tag 28)
/// under the bounded query-only contract. The scalar row commits to access
/// zero, full read/write/delete sharing, null security attributes,
/// `OPEN_EXISTING`, and `FILE_FLAG_BACKUP_SEMANTICS` without
/// `FILE_FLAG_DELETE_ON_CLOSE`; the single handle input is the null template.
pub(crate) fn validate_native_query_open_shape(
    open: &AttemptShape<'_>,
) -> Result<u64, BuildFilesystemReplayRecordError> {
    if open.operation != 28
        || open.provider != 2
        || open.scalars.as_slice()
            != [
                (1, ShapeScalar::U32(0)),
                (2, ShapeScalar::U32(0x7)),
                (3, ShapeScalar::I64(0)),
                (4, ShapeScalar::U32(3)),
                (5, ShapeScalar::U32(0x0200_0000)),
            ]
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record is not a bounded native-handle query chain",
        ));
    }
    let Some(output) = open.output else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle open has no handle output",
        ));
    };
    let identity = output.identity;
    let [open_rooted] = open.rooted_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle open has no unique rooted source path",
        ));
    };
    let [open_authorized] = open.authorized_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle open has no unique authorized source path",
        ));
    };
    if output.kind != 1
        || output.source != 0
        || output.source_identity.is_some()
        || open.result != ShapeResult::Handle(identity)
        || open_rooted.ordinal != 0
        || open_rooted.root != 0
        || !checked_interpreter::filesystem_root_relative_path_is_canonical(
            open_rooted.bytes,
            false,
        )
        || open_authorized.ordinal != 0
        || open_authorized.access != 0
        || open_authorized.root != 0
        || open_authorized.bytes != open_rooted.bytes
        || open.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 6,
                kind: 1,
                resolution: ShapeLogicalInputResolution::Null,
            }]
        || !only_native_query_open_lanes(open)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record has inconsistent native-handle acquisition",
        ));
    }
    Ok(identity)
}

/// One `final_path_name_by_handle` (tag 31) observation on `identity` inside
/// a bounded query-release chain. The buffer custody arithmetic mirrors the
/// checked-interpreter record contract: the resolved snapshot precedes the
/// call, the post state carries the returned path plus its NUL terminator
/// over an unchanged tail, and the scalar result is the returned path
/// length.
pub(crate) fn validate_native_final_path_query_shape(
    query: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let ShapeResult::Scalar(result) = query.result else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle final-path query has a non-scalar result",
        ));
    };
    let [
        (2, ShapeScalar::U64(capacity)),
        (3, ShapeScalar::U32(_flags)),
    ] = query.scalars.as_slice()
    else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle final-path query has no exact capacity and flags",
        ));
    };
    let [returned] = query.returned_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle final-path query has no unique returned path",
        ));
    };
    let [(resolution_ordinal, resolution)] = query.mutable_byte_resolutions.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle final-path query has no unique mutable resolution",
        ));
    };
    let [carrier] = query.mutable_bytes.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle final-path query has no unique mutable carrier",
        ));
    };
    let path_length = returned.bytes.len();
    let capacity_fits = usize::try_from(*capacity)
        .is_ok_and(|capacity| capacity <= carrier.post.len() && path_length < capacity);
    if query.operation != 31
        || query.provider != 2
        || path_length == 0
        || i64::try_from(path_length) != Ok(result)
        || !capacity_fits
        || returned.ordinal != 1
        || returned.kind != 2
        || returned.completeness != 0
        || *resolution_ordinal != 1
        || carrier.ordinal != 1
        || *resolution != carrier.pre
        || carrier.pre.len() != carrier.post.len()
        || carrier.post[..path_length] != returned.bytes[..]
        || carrier.post[path_length] != 0
        || carrier.post[path_length + 1..] != carrier.pre[path_length + 1..]
        || query.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 0,
                kind: 1,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            }]
        || !only_native_final_path_query_lanes(query)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay native-handle final-path query is internally inconsistent",
        ));
    }
    Ok(())
}

/// One handle-free `get_last_error` (tag 35) error-slot read inside a
/// bounded query-release chain. The read observes the slot without clearing
/// it, so the scalar result equals the recorded post-error.
pub(crate) fn validate_native_error_observation_shape(
    operation: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if operation.operation != 35
        || operation.provider != 2
        || operation.result != ShapeResult::Scalar(i64::from(operation.post_error))
        || !only_native_error_observation_lanes(operation)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay native-handle error observation is internally inconsistent",
        ));
    }
    Ok(())
}

/// The successful `close_handle` (tag 29) that retires `identity` at the end
/// of a bounded query-release chain.
pub(crate) fn validate_native_handle_close_shape(
    close: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if close.operation != 29
        || close.provider != 2
        || close.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 0,
                kind: 1,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            }]
        || !matches!(close.result, ShapeResult::Scalar(result) if result != 0)
        || close.retired.as_slice() != [identity]
        || !only_close_lanes(close)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record has inconsistent native-handle retirement",
        ));
    }
    Ok(())
}
