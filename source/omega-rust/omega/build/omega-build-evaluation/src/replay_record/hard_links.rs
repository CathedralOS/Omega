use super::{
    AttemptShape, BuildFilesystemReplayRecordError, ShapeResult, ShapeRootedPath, ShapeScalar,
    clone_bytes,
};

const PORTABLE_HARD_LINK_OPERATION_TAG: u16 = 19;
const WINDOWS_HARD_LINK_OPERATION_TAG: u16 = 27;

pub(super) fn validate_output_hard_link_shape(
    attempt: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let (kind, existing, output, expected_authorized_ordinals, scalar_shape_matches) =
        hard_link_shape(attempt)?;
    let [authorized_existing, authorized_output] = attempt.authorized_paths.as_slice() else {
        return Err(hard_link_shape_error());
    };
    if attempt.provider != 2
        || attempt.result != ShapeResult::Scalar(kind.result())
        || attempt.post_error != 0
        || existing.root != 1
        || output.root != 1
        || existing.root != output.root
        || !valid_output_path(existing.bytes)
        || !valid_output_path(output.bytes)
        || existing.bytes == output.bytes
        || authorized_existing.ordinal != expected_authorized_ordinals[0]
        || authorized_output.ordinal != expected_authorized_ordinals[1]
        || !authorization_matches(authorized_existing, existing)
        || !authorization_matches(authorized_output, output)
        || !scalar_shape_matches
        || !only_hard_link_lanes(attempt)
    {
        return Err(hard_link_shape_error());
    }
    Ok(())
}

pub(super) fn output_hard_link_paths<'a>(
    attempt: &'a AttemptShape<'a>,
) -> Result<(&'a [u8], &'a [u8]), BuildFilesystemReplayRecordError> {
    let (_, existing, output, _, _) = hard_link_shape(attempt)?;
    Ok((existing.bytes, output.bytes))
}

pub(super) fn rehydrate_output_hard_link_shape(
    attempt: &AttemptShape<'_>,
) -> Result<
    psi_checked_interpreter::FilesystemOutputHardLinkReplayRecord,
    BuildFilesystemReplayRecordError,
> {
    let (kind, existing, output, _, _) = hard_link_shape(attempt)?;
    psi_checked_interpreter::FilesystemOutputHardLinkReplayRecord::new(
        kind.interpreter_kind(),
        crate::BUILD_OUTPUT_ROOT_IDENTITY,
        clone_bytes(existing.bytes)?,
        clone_bytes(output.bytes)?,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay Output hard link could not be rehydrated",
        )
    })
}

#[derive(Clone, Copy)]
enum HardLinkKind {
    Portable,
    Windows,
}

impl HardLinkKind {
    const fn result(self) -> i64 {
        match self {
            Self::Portable => 0,
            Self::Windows => 1,
        }
    }

    const fn interpreter_kind(self) -> psi_checked_interpreter::FilesystemOutputHardLinkReplayKind {
        match self {
            Self::Portable => psi_checked_interpreter::FilesystemOutputHardLinkReplayKind::Portable,
            Self::Windows => psi_checked_interpreter::FilesystemOutputHardLinkReplayKind::Windows,
        }
    }
}

type HardLinkShape<'a> = (
    HardLinkKind,
    &'a ShapeRootedPath<'a>,
    &'a ShapeRootedPath<'a>,
    [u8; 2],
    bool,
);

fn hard_link_shape<'a>(
    attempt: &'a AttemptShape<'a>,
) -> Result<HardLinkShape<'a>, BuildFilesystemReplayRecordError> {
    let [first, second] = attempt.rooted_paths.as_slice() else {
        return Err(hard_link_shape_error());
    };
    if first.ordinal != 0 || second.ordinal != 1 {
        return Err(hard_link_shape_error());
    }
    match attempt.operation {
        PORTABLE_HARD_LINK_OPERATION_TAG => Ok((
            HardLinkKind::Portable,
            first,
            second,
            [0, 1],
            attempt.scalars.is_empty(),
        )),
        WINDOWS_HARD_LINK_OPERATION_TAG => Ok((
            HardLinkKind::Windows,
            second,
            first,
            [1, 0],
            attempt.scalars.as_slice() == [(2, ShapeScalar::I64(0))],
        )),
        _ => Err(hard_link_shape_error()),
    }
}

fn valid_output_path(path: &[u8]) -> bool {
    path.len() <= psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES
        && psi_checked_interpreter::filesystem_root_relative_path_is_canonical(path, false)
}

fn authorization_matches(
    authorized: &super::ShapeAuthorizedPath<'_>,
    rooted: &ShapeRootedPath<'_>,
) -> bool {
    authorized.access == 1 && authorized.root == rooted.root && authorized.bytes == rooted.bytes
}

fn only_hard_link_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_i64_resolution_count == 0
        && attempt.mutable_bytes.is_empty()
        && attempt.mutable_i64_count == 0
        && attempt.inputs.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

fn hard_link_shape_error() -> BuildFilesystemReplayRecordError {
    BuildFilesystemReplayRecordError::new(
        "receipted build output hard link is internally inconsistent",
    )
}
