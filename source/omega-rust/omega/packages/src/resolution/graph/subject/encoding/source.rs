//! Canonical package key, source lineage, and immutable resolution encoding.

use super::super::CanonicalSourceClosureSubjectError;
use super::super::validation::{validate_package_key, validate_source_lineage};
use super::framing::{Decoder, Encoder, decode_hex_32, encode_hex};
use crate::declarations::{PackageKey, PackageName};
use crate::resolution::graph::ResolvedSourceIdentity;
use omega_package_source::{
    ExternalLocalLineage, ExternalSourceContext, GitCommitId, GitTransport, GitTreeId,
    ImmutableSourceResolution, SourceContentDigest, SourceLineage, SourceRelativePath,
    WorkspaceLineageIdentity, WorkspaceMemberLineage,
};

pub(super) fn encode_source_identity(
    encoder: &mut Encoder,
    source: &ResolvedSourceIdentity,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encode_package_key(encoder, source.key(), maximum_identity_bytes)?;
    encode_resolution(encoder, source.resolution())
}

pub(in super::super) fn decode_source_identity(
    decoder: &mut Decoder<'_>,
    maximum_identity_bytes: usize,
) -> Result<ResolvedSourceIdentity, CanonicalSourceClosureSubjectError> {
    ResolvedSourceIdentity::new(
        decode_package_key(decoder, maximum_identity_bytes)?,
        decode_resolution(decoder)?,
    )
    .map_err(|_| {
        CanonicalSourceClosureSubjectError::new(
            "decoded source resolution disagrees with package lineage",
        )
    })
}

pub(super) fn encode_package_key(
    encoder: &mut Encoder,
    key: &PackageKey,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    validate_package_key(key, maximum_identity_bytes)?;
    encoder.bytes_bounded(key.name().as_str().as_bytes(), maximum_identity_bytes)?;
    encode_source_lineage(encoder, key.source_lineage(), maximum_identity_bytes)
}

pub(super) fn decode_package_key(
    decoder: &mut Decoder<'_>,
    maximum_identity_bytes: usize,
) -> Result<PackageKey, CanonicalSourceClosureSubjectError> {
    let name = PackageName::parse(decoder.string(maximum_identity_bytes)?)
        .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid package name"))?;
    let lineage = decode_source_lineage(decoder, maximum_identity_bytes)?;
    Ok(PackageKey::new(name, lineage))
}

pub(super) fn encode_source_lineage(
    encoder: &mut Encoder,
    lineage: &SourceLineage,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    validate_source_lineage(lineage, maximum_identity_bytes)?;
    match lineage {
        SourceLineage::GitHub(lineage) => {
            encoder.byte(0);
            encoder.bytes_bounded(lineage.owner().as_bytes(), maximum_identity_bytes)?;
            encoder.bytes_bounded(lineage.repository().as_bytes(), maximum_identity_bytes)?;
        }
        SourceLineage::GitLab(lineage) => {
            encoder.byte(1);
            encoder.bytes_bounded(lineage.repository_path().as_bytes(), maximum_identity_bytes)?;
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
                    encoder.bytes_bounded(user.as_bytes(), maximum_identity_bytes)?;
                }
            }
            encoder.bytes_bounded(lineage.host().as_bytes(), maximum_identity_bytes)?;
            match lineage.port() {
                None => encoder.byte(0),
                Some(port) => {
                    encoder.byte(1);
                    encoder.u16(port);
                }
            }
            encoder.bytes_bounded(lineage.repository_path().as_bytes(), maximum_identity_bytes)?;
        }
        SourceLineage::Workspace(lineage) => {
            encoder.byte(3);
            encoder.fixed(&decode_hex_32(&lineage.workspace_identity().to_hex())?);
            encoder.bytes_bounded(
                lineage.member_path().as_str().as_bytes(),
                maximum_identity_bytes,
            )?;
        }
        SourceLineage::ExternalLocal(lineage) => {
            encoder.byte(4);
            encoder.fixed(&decode_hex_32(&lineage.source_context().to_hex())?);
            encoder.bytes_bounded(
                lineage
                    .canonical_absolute_path()
                    .to_str()
                    .ok_or_else(|| {
                        CanonicalSourceClosureSubjectError::new(
                            "external-local lineage path is not UTF-8",
                        )
                    })?
                    .as_bytes(),
                maximum_identity_bytes,
            )?;
        }
    }
    Ok(())
}

pub(super) fn decode_source_lineage(
    decoder: &mut Decoder<'_>,
    maximum_identity_bytes: usize,
) -> Result<SourceLineage, CanonicalSourceClosureSubjectError> {
    match decoder.byte()? {
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
                _ => {
                    return Err(CanonicalSourceClosureSubjectError::new(
                        "invalid Git transport tag",
                    ));
                }
            };
            let user = match decoder.byte()? {
                0 => None,
                1 => Some(decoder.string(maximum_identity_bytes)?),
                _ => {
                    return Err(CanonicalSourceClosureSubjectError::new(
                        "invalid Git user option tag",
                    ));
                }
            };
            let host = decoder.string(maximum_identity_bytes)?;
            let port = match decoder.byte()? {
                0 => None,
                1 => Some(decoder.u16()?),
                _ => {
                    return Err(CanonicalSourceClosureSubjectError::new(
                        "invalid Git port option tag",
                    ));
                }
            };
            let path = decoder.string(maximum_identity_bytes)?;
            SourceLineage::git(&generic_git_locator(
                transport,
                user.as_deref(),
                &host,
                port,
                &path,
            ))
        }
        3 => {
            let workspace = WorkspaceLineageIdentity::parse_hex(&encode_hex(&decoder.array_32()?))
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new("invalid workspace lineage identity")
                })?;
            let member = SourceRelativePath::parse(&decoder.string(maximum_identity_bytes)?)
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new("invalid workspace member path")
                })?;
            return Ok(SourceLineage::Workspace(WorkspaceMemberLineage::new(
                workspace, member,
            )));
        }
        4 => {
            let context = ExternalSourceContext::parse_hex(&encode_hex(&decoder.array_32()?))
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new("invalid external source context")
                })?;
            let path = decoder.string(maximum_identity_bytes)?;
            return ExternalLocalLineage::from_recovered_canonical_path(path, context)
                .map(SourceLineage::ExternalLocal)
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new("invalid external-local lineage path")
                });
        }
        _ => {
            return Err(CanonicalSourceClosureSubjectError::new(
                "invalid source-lineage tag",
            ));
        }
    }
    .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid source lineage"))
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

fn encode_resolution(
    encoder: &mut Encoder,
    resolution: &ImmutableSourceResolution,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match resolution {
        ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        } => {
            encoder.byte(0);
            encoder.bytes_bounded(commit.to_hex().as_bytes(), 64)?;
            encoder.bytes_bounded(tree.to_hex().as_bytes(), 64)?;
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

fn decode_resolution(
    decoder: &mut Decoder<'_>,
) -> Result<ImmutableSourceResolution, CanonicalSourceClosureSubjectError> {
    let content = |decoder: &mut Decoder<'_>| {
        SourceContentDigest::parse_hex(&encode_hex(&decoder.array_32()?))
            .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid source content digest"))
    };
    match decoder.byte()? {
        0 => {
            let commit = GitCommitId::parse_hex(&decoder.string(64)?)
                .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid Git commit ID"))?;
            let tree = GitTreeId::parse_hex(&decoder.string(64)?)
                .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid Git tree ID"))?;
            let encoded_content = content(decoder)?;
            let resolution = ImmutableSourceResolution::git(commit, tree).map_err(|_| {
                CanonicalSourceClosureSubjectError::new("invalid Git source resolution")
            })?;
            if resolution.content() != &encoded_content {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "Git source content digest does not match its root tree",
                ));
            }
            Ok(resolution)
        }
        1 => Ok(ImmutableSourceResolution::workspace(content(decoder)?)),
        2 => Ok(ImmutableSourceResolution::external_local(content(decoder)?)),
        _ => Err(CanonicalSourceClosureSubjectError::new(
            "invalid immutable-resolution tag",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_git_resolution_rejects_content_not_derived_from_its_tree() {
        let resolution = ImmutableSourceResolution::git(
            GitCommitId::parse_hex(&"01".repeat(20)).unwrap(),
            GitTreeId::parse_hex(&"02".repeat(20)).unwrap(),
        )
        .unwrap();
        let mut encoder = Encoder::new();
        encode_resolution(&mut encoder, &resolution).unwrap();
        let mut encoded = encoder.finish();

        let mut decoder = Decoder::new(&encoded);
        assert_eq!(decode_resolution(&mut decoder).unwrap(), resolution);
        decoder.finish().unwrap();

        *encoded.last_mut().unwrap() ^= 1;
        assert!(decode_resolution(&mut Decoder::new(&encoded)).is_err());
    }
}
