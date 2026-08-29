//! Canonical baseline codec and identity encoding.

use super::validation::replay_record_limits;
use super::{
    CHECKSUM_DOMAIN, REPLAY_PARENT_BINDING_DOMAIN, ReviewOnlyBaselineError,
    ReviewOnlyBaselineLimits,
};
use crate::identity::{PackageKey, PackageName};
use crate::review::records::ReviewOnlyCanonicalRow;
use omega_build_evaluation::{
    ReviewOnlyBuildFilesystemReplayRecord, recover_review_only_build_filesystem_replay_record,
};
use omega_package_review::encoding::{
    PackageReviewCanonicalRowRecoveryLimits, decode_package_review_canonical_row_with_limits,
};
use omega_package_source::{
    ExternalLocalLineage, ExternalSourceContext, GitCommitId, GitTransport, GitTreeId,
    ImmutableSourceResolution, SourceContentDigest, SourceLineage, SourceRelativePath,
    WorkspaceLineageIdentity, WorkspaceMemberLineage,
};
use sha2::{Digest, Sha256};

pub(super) fn replay_parent_binding(parent: [u8; 32], replay: [u8; 32]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(REPLAY_PARENT_BINDING_DOMAIN);
    digest.update(parent);
    digest.update(replay);
    digest.finalize().into()
}

pub(super) fn encode_replay_record_option(
    encoder: &mut Encoder,
    replay: Option<&ReviewOnlyBuildFilesystemReplayRecord>,
) -> Result<(), ReviewOnlyBaselineError> {
    match replay {
        None => encoder.byte(0),
        Some(replay) => {
            encoder.byte(1);
            encoder.bytes(replay.canonical_bytes())?;
        }
    }
    Ok(())
}

pub(super) fn decode_replay_record_option(
    decoder: &mut Decoder<'_>,
    limits: ReviewOnlyBaselineLimits,
) -> Result<Option<ReviewOnlyBuildFilesystemReplayRecord>, ReviewOnlyBaselineError> {
    match decoder.byte()? {
        0 => Ok(None),
        1 => recover_review_only_build_filesystem_replay_record(
            decoder.bytes(limits.maximum_capsule_bytes)?,
            replay_record_limits(limits),
        )
        .map(Some)
        .map_err(|_| ReviewOnlyBaselineError::new("invalid compiler filesystem replay record")),
        _ => Err(ReviewOnlyBaselineError::new(
            "invalid filesystem-replay-record option tag",
        )),
    }
}

pub(super) fn capsule_checksum(prefix: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CHECKSUM_DOMAIN);
    digest.update(
        u64::try_from(prefix.len())
            .expect("bounded capsule length fits u64")
            .to_le_bytes(),
    );
    digest.update(prefix);
    digest.finalize().into()
}

pub(super) fn clone_baseline_bytes(
    bytes: &[u8],
    allocation_error: &'static str,
) -> Result<Vec<u8>, ReviewOnlyBaselineError> {
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(bytes.len())
        .map_err(|_| ReviewOnlyBaselineError::new(allocation_error))?;
    owned.extend_from_slice(bytes);
    Ok(owned)
}

pub(super) fn ensure_bounded_string(
    value: &str,
    maximum_bytes: usize,
    error: &'static str,
) -> Result<(), ReviewOnlyBaselineError> {
    if value.is_empty() || value.len() > maximum_bytes {
        Err(ReviewOnlyBaselineError::new(error))
    } else {
        Ok(())
    }
}

pub(super) fn validate_package_key_bounds(
    key: &PackageKey,
    maximum_identity_bytes: usize,
) -> Result<(), ReviewOnlyBaselineError> {
    let check = |value: &str| {
        ensure_bounded_string(
            value,
            maximum_identity_bytes,
            "review baseline package identity violates its byte bounds",
        )
    };
    check(key.name().as_str())?;
    match key.source_lineage() {
        SourceLineage::GitHub(lineage) => {
            check(lineage.owner())?;
            check(lineage.repository())?;
        }
        SourceLineage::GitLab(lineage) => check(lineage.repository_path())?,
        SourceLineage::Git(lineage) => {
            if let Some(user) = lineage.user() {
                check(user)?;
            }
            check(lineage.host())?;
            check(lineage.repository_path())?;
        }
        SourceLineage::Workspace(lineage) => check(lineage.member_path().as_str())?,
        SourceLineage::ExternalLocal(lineage) => {
            check(lineage.canonical_absolute_path().to_str().ok_or_else(|| {
                ReviewOnlyBaselineError::new("external source path is not UTF-8")
            })?)?
        }
    }
    Ok(())
}

pub(super) fn validate_recovery_row<'a>(
    row: &'a ReviewOnlyCanonicalRow,
    key: &PackageKey,
    target: &str,
    limits: PackageReviewCanonicalRowRecoveryLimits,
) -> Result<&'a [u8], ReviewOnlyBaselineError> {
    let recovery_bytes = row.recovery_bytes().ok_or_else(|| {
        ReviewOnlyBaselineError::new("review baseline contains a non-recoverable comparison row")
    })?;
    let decoded = decode_package_review_canonical_row_with_limits(recovery_bytes, limits)
        .map_err(|_| ReviewOnlyBaselineError::new("invalid recovered compiler review row"))?;
    if decoded.package() != key.identity()
        || decoded.target().target_name() != target
        || decoded.kind() != row.kind()
        || decoded.risk() != row.risk()
        || decoded.key_bytes() != row.key_bytes()
        || decoded.canonical_bytes() != row.canonical_bytes()
        || decoded.source() != row.source()
    {
        return Err(ReviewOnlyBaselineError::new(
            "recovered compiler review row disagrees with review-only comparison metadata",
        ));
    }
    Ok(recovery_bytes)
}

pub(super) fn encode_package_key(
    encoder: &mut Encoder,
    key: &PackageKey,
    maximum_identity_bytes: usize,
) -> Result<(), ReviewOnlyBaselineError> {
    validate_package_key_bounds(key, maximum_identity_bytes)?;
    encoder.string(key.name().as_str())?;
    match key.source_lineage() {
        SourceLineage::GitHub(lineage) => {
            encoder.byte(0);
            encoder.string(lineage.owner())?;
            encoder.string(lineage.repository())?;
        }
        SourceLineage::GitLab(lineage) => {
            encoder.byte(1);
            encoder.string(lineage.repository_path())?;
        }
        SourceLineage::Git(lineage) => {
            encoder.byte(2);
            encoder.byte(match lineage.transport() {
                GitTransport::Https => 0,
                GitTransport::SshUrl => 1,
                GitTransport::ScpLike => 2,
            });
            match lineage.user() {
                None => encoder.byte(0),
                Some(user) => {
                    encoder.byte(1);
                    encoder.string(user)?;
                }
            }
            encoder.string(lineage.host())?;
            match lineage.port() {
                None => encoder.byte(0),
                Some(port) => {
                    encoder.byte(1);
                    encoder.u16(port);
                }
            }
            encoder.string(lineage.repository_path())?;
        }
        SourceLineage::Workspace(lineage) => {
            encoder.byte(3);
            encoder.fixed(&decode_hex_32(&lineage.workspace_identity().to_hex())?);
            encoder.string(lineage.member_path().as_str())?;
        }
        SourceLineage::ExternalLocal(lineage) => {
            encoder.byte(4);
            encoder.fixed(&decode_hex_32(&lineage.source_context().to_hex())?);
            encoder.string(lineage.canonical_absolute_path().to_str().ok_or_else(|| {
                ReviewOnlyBaselineError::new("external source path is not UTF-8")
            })?)?;
        }
    }
    Ok(())
}

pub(super) fn decode_package_key(
    decoder: &mut Decoder<'_>,
    maximum_identity_bytes: usize,
) -> Result<PackageKey, ReviewOnlyBaselineError> {
    let name = PackageName::parse(decoder.string(maximum_identity_bytes)?.to_owned())
        .map_err(|_| ReviewOnlyBaselineError::new("invalid package name in review baseline"))?;
    let lineage = match decoder.byte()? {
        0 => SourceLineage::git(&format!(
            "https://github.com/{}/{}.git",
            decoder.string(maximum_identity_bytes)?,
            decoder.string(maximum_identity_bytes)?
        )),
        1 => SourceLineage::git(&format!(
            "https://gitlab.com/{}.git",
            decoder.string(maximum_identity_bytes)?
        )),
        2 => {
            let transport = match decoder.byte()? {
                0 => GitTransport::Https,
                1 => GitTransport::SshUrl,
                2 => GitTransport::ScpLike,
                _ => return Err(ReviewOnlyBaselineError::new("invalid Git transport tag")),
            };
            let user = match decoder.byte()? {
                0 => None,
                1 => Some(decoder.string(maximum_identity_bytes)?.to_owned()),
                _ => return Err(ReviewOnlyBaselineError::new("invalid Git user option tag")),
            };
            let host = decoder.string(maximum_identity_bytes)?.to_owned();
            let port = match decoder.byte()? {
                0 => None,
                1 => Some(decoder.u16()?),
                _ => return Err(ReviewOnlyBaselineError::new("invalid Git port option tag")),
            };
            let path = decoder.string(maximum_identity_bytes)?.to_owned();
            let locator = generic_git_locator(transport, user.as_deref(), &host, port, &path);
            SourceLineage::git(&locator)
        }
        3 => {
            let workspace = WorkspaceLineageIdentity::parse_hex(&encode_hex(&decoder.array_32()?))
                .map_err(|_| ReviewOnlyBaselineError::new("invalid workspace identity"));
            let member = SourceRelativePath::parse(decoder.string(maximum_identity_bytes)?)
                .map_err(|_| ReviewOnlyBaselineError::new("invalid workspace member path"));
            return Ok(PackageKey::new(
                name,
                SourceLineage::Workspace(WorkspaceMemberLineage::new(workspace?, member?)),
            ));
        }
        4 => {
            let context = ExternalSourceContext::parse_hex(&encode_hex(&decoder.array_32()?));
            let path = decoder.string(maximum_identity_bytes)?.to_owned();
            context
                .and_then(|context| {
                    ExternalLocalLineage::from_recovered_canonical_path(path, context)
                })
                .map(SourceLineage::ExternalLocal)
        }
        _ => return Err(ReviewOnlyBaselineError::new("invalid source-lineage tag")),
    }
    .map_err(|_| ReviewOnlyBaselineError::new("invalid source lineage in review baseline"))?;
    Ok(PackageKey::new(name, lineage))
}

fn generic_git_locator(
    transport: GitTransport,
    user: Option<&str>,
    host: &str,
    port: Option<u16>,
    path: &str,
) -> String {
    let user = user.map(|user| format!("{user}@")).unwrap_or_default();
    match transport {
        GitTransport::Https => format!(
            "https://{user}{host}{}/{path}",
            port.map(|port| format!(":{port}")).unwrap_or_default()
        ),
        GitTransport::SshUrl => format!(
            "ssh://{user}{host}{}/{path}",
            port.map(|port| format!(":{port}")).unwrap_or_default()
        ),
        GitTransport::ScpLike => format!("{user}{host}:{path}"),
    }
}

pub(super) fn encode_resolution(
    encoder: &mut Encoder,
    resolution: &ImmutableSourceResolution,
) -> Result<(), ReviewOnlyBaselineError> {
    match resolution {
        ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        } => {
            encoder.byte(0);
            encoder.string(&commit.to_hex())?;
            encoder.string(&tree.to_hex())?;
            encoder.fixed(&decode_hex_32(&content.to_hex())?);
        }
        ImmutableSourceResolution::Workspace { content } => {
            encoder.byte(1);
            encoder.fixed(&decode_hex_32(&content.to_hex())?);
        }
        ImmutableSourceResolution::ExternalLocal { content } => {
            encoder.byte(2);
            encoder.fixed(&decode_hex_32(&content.to_hex())?);
        }
    }
    Ok(())
}

pub(super) fn decode_resolution(
    decoder: &mut Decoder<'_>,
) -> Result<ImmutableSourceResolution, ReviewOnlyBaselineError> {
    let content = |decoder: &mut Decoder<'_>| {
        SourceContentDigest::parse_hex(&encode_hex(&decoder.array_32()?))
            .map_err(|_| ReviewOnlyBaselineError::new("invalid source content digest"))
    };
    match decoder.byte()? {
        0 => {
            let commit = GitCommitId::parse_hex(decoder.string(64)?)
                .map_err(|_| ReviewOnlyBaselineError::new("invalid Git commit ID"))?;
            let tree = GitTreeId::parse_hex(decoder.string(64)?)
                .map_err(|_| ReviewOnlyBaselineError::new("invalid Git tree ID"))?;
            let encoded_content = content(decoder)?;
            let resolution = ImmutableSourceResolution::git(commit, tree)
                .map_err(|_| ReviewOnlyBaselineError::new("invalid Git source resolution"))?;
            if resolution.content() != &encoded_content {
                return Err(ReviewOnlyBaselineError::new(
                    "Git source content digest does not match its root tree",
                ));
            }
            Ok(resolution)
        }
        1 => Ok(ImmutableSourceResolution::workspace(content(decoder)?)),
        2 => Ok(ImmutableSourceResolution::external_local(content(decoder)?)),
        _ => Err(ReviewOnlyBaselineError::new(
            "invalid immutable-resolution tag",
        )),
    }
}

fn decode_hex_32(value: &str) -> Result<[u8; 32], ReviewOnlyBaselineError> {
    let bytes = decode_hex(value)
        .ok_or_else(|| ReviewOnlyBaselineError::new("invalid 32-byte hexadecimal value"))?;
    bytes
        .try_into()
        .map_err(|_| ReviewOnlyBaselineError::new("invalid 32-byte hexadecimal value"))
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = char::from(pair[0]).to_digit(16)?;
            let low = char::from(pair[1]).to_digit(16)?;
            Some(((high << 4) | low) as u8)
        })
        .collect()
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

pub(super) struct Encoder {
    bytes: Vec<u8>,
    maximum_bytes: usize,
    exceeded: bool,
}

impl Encoder {
    pub(super) fn bounded(maximum_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum_bytes,
            exceeded: false,
        }
    }

    pub(super) fn append(&mut self, bytes: &[u8]) {
        if self.exceeded
            || self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|length| length > self.maximum_bytes)
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

    pub(super) fn fixed(&mut self, bytes: &[u8]) {
        self.append(bytes);
    }

    pub(super) fn byte(&mut self, value: u8) {
        self.append(&[value]);
    }

    pub(super) fn u16(&mut self, value: u16) {
        self.append(&value.to_le_bytes());
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.append(&value.to_le_bytes());
    }

    pub(super) fn usize(&mut self, value: usize) -> Result<(), ReviewOnlyBaselineError> {
        self.append(
            &u64::try_from(value)
                .map_err(|_| ReviewOnlyBaselineError::new("baseline length exceeds u64"))?
                .to_le_bytes(),
        );
        Ok(())
    }

    pub(super) fn bytes(&mut self, bytes: &[u8]) -> Result<(), ReviewOnlyBaselineError> {
        self.usize(bytes.len())?;
        self.append(bytes);
        Ok(())
    }

    pub(super) fn string(&mut self, value: &str) -> Result<(), ReviewOnlyBaselineError> {
        self.bytes(value.as_bytes())
    }

    pub(super) fn finish(self) -> Result<Vec<u8>, ReviewOnlyBaselineError> {
        if self.exceeded {
            Err(ReviewOnlyBaselineError::new(
                "review baseline encoding exceeds its byte ceiling",
            ))
        } else {
            Ok(self.bytes)
        }
    }
}

pub(super) struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(super) fn take(&mut self, length: usize) -> Result<&'a [u8], ReviewOnlyBaselineError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| ReviewOnlyBaselineError::new("baseline length overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| ReviewOnlyBaselineError::new("truncated review baseline capsule"))?;
        self.offset = end;
        Ok(value)
    }

    pub(super) fn fixed(&mut self, expected: &[u8]) -> Result<(), ReviewOnlyBaselineError> {
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(ReviewOnlyBaselineError::new(
                "invalid review baseline capsule magic",
            ))
        }
    }

    pub(super) fn byte(&mut self) -> Result<u8, ReviewOnlyBaselineError> {
        Ok(self.take(1)?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16, ReviewOnlyBaselineError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("exact u16 width"),
        ))
    }

    pub(super) fn u32(&mut self) -> Result<u32, ReviewOnlyBaselineError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("exact u32 width"),
        ))
    }

    pub(super) fn usize(&mut self) -> Result<usize, ReviewOnlyBaselineError> {
        usize::try_from(u64::from_le_bytes(
            self.take(8)?.try_into().expect("exact u64 width"),
        ))
        .map_err(|_| ReviewOnlyBaselineError::new("baseline length exceeds usize"))
    }

    pub(super) fn bytes(&mut self, maximum: usize) -> Result<&'a [u8], ReviewOnlyBaselineError> {
        let length = self.usize()?;
        if length > maximum {
            return Err(ReviewOnlyBaselineError::new(
                "review baseline field exceeds its byte ceiling",
            ));
        }
        self.take(length)
    }

    pub(super) fn string(&mut self, maximum: usize) -> Result<&'a str, ReviewOnlyBaselineError> {
        std::str::from_utf8(self.bytes(maximum)?)
            .map_err(|_| ReviewOnlyBaselineError::new("review baseline string is not UTF-8"))
    }

    pub(super) fn array_32(&mut self) -> Result<[u8; 32], ReviewOnlyBaselineError> {
        Ok(self.take(32)?.try_into().expect("exact digest width"))
    }

    pub(super) fn finish(self) -> Result<(), ReviewOnlyBaselineError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ReviewOnlyBaselineError::new(
                "review baseline capsule has trailing bytes",
            ))
        }
    }
}
