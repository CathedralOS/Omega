//! Canonical codecs for package lineage and immutable source resolution.

use super::{Decoder, Encoder, ensure_bounded_string};
use crate::declarations::{PackageKey, PackageName};
use crate::review::baseline::ReviewOnlyBaselineError;
use package_source::{
    ExternalLocalLineage, ExternalSourceContext, GitCommitId, GitTransport, GitTreeId,
    ImmutableSourceResolution, SourceContentDigest, SourceLineage, SourceRelativePath,
    WorkspaceLineageIdentity, WorkspaceMemberLineage,
};

pub(in crate::review::baseline) fn validate_package_key_bounds(
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

pub(in crate::review::baseline) fn encode_package_key(
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

pub(in crate::review::baseline) fn decode_package_key(
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

pub(in crate::review::baseline) fn encode_resolution(
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

pub(in crate::review::baseline) fn decode_resolution(
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
        .as_chunks::<2>()
        .0
        .iter()
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
