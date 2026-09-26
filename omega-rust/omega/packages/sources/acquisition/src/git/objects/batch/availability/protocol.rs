//! One exact full-OID batch-check line, with no stderr or trailing response.

use super::{
    ExactGitObjectAvailability as Availability, ExactGitObjectKind as Kind, OPERATION,
    validate_requested_oid,
};
use crate::error::SourceResolveError;
use crate::git::objects::identity::git_object_invalid;

pub(super) fn response(
    oid: &str,
    success: bool,
    status: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<Availability, SourceResolveError> {
    validate_requested_oid(oid)?;
    if !success {
        return Err(SourceResolveError::Git {
            operation: OPERATION.to_owned(),
            status,
            stderr: String::from_utf8_lossy(stderr).into_owned(),
        });
    }
    let invalid = || {
        git_object_invalid(
            oid,
            "batch-check did not return one exact object availability response",
        )
    };
    if !stderr.is_empty() {
        return Err(invalid());
    }
    let line = stdout.strip_suffix(b"\n").ok_or_else(invalid)?;
    let remaining = line
        .strip_prefix(oid.as_bytes())
        .and_then(|tail| tail.strip_prefix(b" "))
        .ok_or_else(invalid)?;
    if remaining == b"missing" {
        return Ok(Availability::Missing);
    }
    let separator = remaining
        .iter()
        .position(|byte| *byte == b' ')
        .ok_or_else(invalid)?;
    let (kind, tail) = remaining.split_at(separator);
    let size = &tail[1..];
    let kind = match kind {
        b"commit" => Kind::Commit,
        b"tree" => Kind::Tree,
        b"blob" => Kind::Blob,
        b"tag" => Kind::Tag,
        _ => return Err(invalid()),
    };
    if size.is_empty() || (size.len() > 1 && size[0] == b'0') {
        return Err(invalid());
    }
    let size = size
        .iter()
        .try_fold(0u64, |total, byte| {
            if !byte.is_ascii_digit() {
                return None;
            }
            total.checked_mul(10)?.checked_add(u64::from(byte - b'0'))
        })
        .ok_or_else(invalid)?;
    Ok(Availability::Present { kind, size })
}
