//! Git object-ID parsing and collision-checked object hashing.

use crate::error::SourceResolveError;
use crate::identity::GitObjectIdAlgorithm;
use crate::identity::digest::format_hex;
use sha1_checked::Sha1 as CheckedSha1;
use sha2::{Digest, Sha256};

pub(crate) fn verify_git_object_identity(
    expected: &str,
    kind: &[u8],
    payload: &[u8],
    algorithm: GitObjectIdAlgorithm,
) -> Result<(), SourceResolveError> {
    if git_object_algorithm(expected)? != algorithm {
        return Err(git_object_invalid(
            expected,
            "object ID uses a different hash algorithm than its graph",
        ));
    }
    if git_object_identity(kind, payload, algorithm)? != expected {
        return Err(git_object_invalid(
            expected,
            "object bytes do not match the declared object ID",
        ));
    }
    Ok(())
}

pub(crate) fn git_object_identity(
    kind: &[u8],
    payload: &[u8],
    algorithm: GitObjectIdAlgorithm,
) -> Result<String, SourceResolveError> {
    let length = payload.len().to_string();
    match algorithm {
        GitObjectIdAlgorithm::Sha1 => {
            let mut hasher = CheckedSha1::new();
            hasher.update(kind);
            hasher.update(b" ");
            hasher.update(length.as_bytes());
            hasher.update([0]);
            hasher.update(payload);
            finalize_checked_sha1(hasher)
        }
        GitObjectIdAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(kind);
            hasher.update(b" ");
            hasher.update(length.as_bytes());
            hasher.update([0]);
            hasher.update(payload);
            Ok(format_hex(&hasher.finalize()))
        }
    }
}

pub(crate) fn finalize_checked_sha1(hasher: CheckedSha1) -> Result<String, SourceResolveError> {
    let result = hasher.try_finalize();
    if result.has_collision() {
        return Err(git_object_invalid(
            "sha1-collision",
            "Git object bytes match a known SHA-1 collision attack",
        ));
    }
    Ok(format_hex(result.hash()))
}

pub(crate) fn git_object_algorithm(oid: &str) -> Result<GitObjectIdAlgorithm, SourceResolveError> {
    if !is_object_id(oid) {
        return Err(git_object_invalid(oid, "object ID has an invalid spelling"));
    }
    Ok(if oid.len() == 40 {
        GitObjectIdAlgorithm::Sha1
    } else {
        GitObjectIdAlgorithm::Sha256
    })
}

pub(super) fn decode_git_object_id(
    oid: &str,
    algorithm: GitObjectIdAlgorithm,
) -> Result<Vec<u8>, SourceResolveError> {
    if git_object_algorithm(oid)? != algorithm {
        return Err(git_object_invalid(
            oid,
            "child object uses a different hash algorithm than its tree",
        ));
    }
    let mut bytes = Vec::with_capacity(oid.len() / 2);
    for pair in oid.as_bytes().as_chunks::<2>().0 {
        let high = hex_digit(pair[0])
            .ok_or_else(|| git_object_invalid(oid, "object ID contains a non-hexadecimal digit"))?;
        let low = hex_digit(pair[1])
            .ok_or_else(|| git_object_invalid(oid, "object ID contains a non-hexadecimal digit"))?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

pub(crate) fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(crate) fn is_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) fn git_object_invalid(
    oid: impl Into<String>,
    message: impl Into<String>,
) -> SourceResolveError {
    SourceResolveError::GitObjectInvalid {
        oid: oid.into(),
        message: message.into(),
    }
}
