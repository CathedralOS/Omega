use super::{
    AttemptShape, BuildFilesystemReplayRecordError, ShapeResult, ShapeReturnedPath, clone_bytes,
};

const READ_LINK_OPERATION_TAG: u16 = 21;
const READ_LINK_PAYLOAD_KIND: u8 = 0;
const COMPLETE: u8 = 0;
const LIMIT_REACHED: u8 = 1;

pub(super) fn validate_source_read_link_shape(
    attempt: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let [(count_ordinal, super::ShapeScalar::U64(requested_count))] = attempt.scalars.as_slice()
    else {
        return Err(read_link_shape_error());
    };
    let [rooted] = attempt.rooted_paths.as_slice() else {
        return Err(read_link_shape_error());
    };
    let [returned] = attempt.returned_paths.as_slice() else {
        return Err(read_link_shape_error());
    };
    let [(resolution_ordinal, resolution)] = attempt.mutable_byte_resolutions.as_slice() else {
        return Err(read_link_shape_error());
    };
    let [carrier] = attempt.mutable_bytes.as_slice() else {
        return Err(read_link_shape_error());
    };
    let [authorized] = attempt.authorized_paths.as_slice() else {
        return Err(read_link_shape_error());
    };
    let ShapeResult::Scalar(result) = attempt.result else {
        return Err(read_link_shape_error());
    };
    let Ok(result_length) = usize::try_from(result) else {
        return Err(read_link_shape_error());
    };
    let Ok(requested_capacity) = usize::try_from(*requested_count) else {
        return Err(read_link_shape_error());
    };

    if attempt.operation != READ_LINK_OPERATION_TAG
        || attempt.provider != 2
        || *count_ordinal != 2
        || rooted.ordinal != 0
        || rooted.root != 0
        || !checked_interpreter::filesystem_root_relative_path_is_canonical(rooted.bytes, false)
        || returned.ordinal != 1
        || returned.kind != READ_LINK_PAYLOAD_KIND
        || returned.bytes.len() != result_length
        || returned.bytes.contains(&0)
        || !returned_completeness_matches(returned, result_length, requested_capacity)
        || *resolution_ordinal != 1
        || carrier.ordinal != 1
        || *resolution != carrier.pre
        || carrier.pre.len() != carrier.post.len()
        || requested_capacity > carrier.post.len()
        || result_length > requested_capacity
        || carrier.post[..result_length] != *returned.bytes
        || carrier.pre[result_length..] != carrier.post[result_length..]
        || authorized.ordinal != 0
        || authorized.access != 0
        || authorized.root != 0
        || !checked_interpreter::filesystem_root_relative_path_is_canonical(authorized.bytes, false)
        || !only_read_link_lanes(attempt)
    {
        return Err(read_link_shape_error());
    }
    Ok(())
}

pub(super) fn rehydrate_source_read_link_shape(
    attempt: &AttemptShape<'_>,
) -> Result<
    checked_interpreter::FilesystemSourceReadLinkReplayRecord,
    BuildFilesystemReplayRecordError,
> {
    let [(_, super::ShapeScalar::U64(requested_count))] = attempt.scalars.as_slice() else {
        unreachable!("validated Source read_link has one count")
    };
    let [rooted] = attempt.rooted_paths.as_slice() else {
        unreachable!("validated Source read_link has one rooted input")
    };
    let [returned] = attempt.returned_paths.as_slice() else {
        unreachable!("validated Source read_link has one returned path")
    };
    let [(_, resolution)] = attempt.mutable_byte_resolutions.as_slice() else {
        unreachable!("validated Source read_link has one mutable resolution")
    };
    let [carrier] = attempt.mutable_bytes.as_slice() else {
        unreachable!("validated Source read_link has one mutable carrier")
    };
    let [authorized] = attempt.authorized_paths.as_slice() else {
        unreachable!("validated Source read_link has one authorized target")
    };
    let ShapeResult::Scalar(result) = attempt.result else {
        unreachable!("validated Source read_link returns a scalar")
    };
    let completeness = match returned.completeness {
        COMPLETE => checked_interpreter::FilesystemReturnedPathCompleteness::Complete,
        LIMIT_REACHED => checked_interpreter::FilesystemReturnedPathCompleteness::LimitReached,
        _ => unreachable!("decoded returned-path completeness is closed"),
    };

    checked_interpreter::FilesystemSourceReadLinkReplayRecord::new(
        crate::BUILD_SOURCE_ROOT_IDENTITY,
        clone_bytes(rooted.bytes)?,
        crate::BUILD_SOURCE_ROOT_IDENTITY,
        clone_bytes(authorized.bytes)?,
        *requested_count,
        result,
        attempt.post_error,
        clone_bytes(resolution)?,
        clone_bytes(carrier.pre)?,
        clone_bytes(carrier.post)?,
        completeness,
        clone_bytes(returned.bytes)?,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay Source read_link could not be rehydrated",
        )
    })
}

fn returned_completeness_matches(
    returned: &ShapeReturnedPath<'_>,
    result_length: usize,
    requested_capacity: usize,
) -> bool {
    match returned.completeness {
        COMPLETE => !returned.bytes.is_empty() && result_length <= requested_capacity,
        LIMIT_REACHED => result_length == requested_capacity,
        _ => false,
    }
}

fn only_read_link_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.returned_path_count == 1
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.inputs.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

fn read_link_shape_error() -> BuildFilesystemReplayRecordError {
    BuildFilesystemReplayRecordError::new(
        "filesystem replay Source read_link is internally inconsistent",
    )
}
