//! Canonical package key, source lineage, and immutable resolution encoding.

use super::super::CanonicalSourceClosureSubjectError;
use super::super::validation::{validate_package_key, validate_source_lineage};
use super::framing::Encoder;
use crate::declarations::PackageKey;
use crate::resolution::graph::ResolvedSourceIdentity;
use package_source::{GitTransport, ImmutableSourceResolution, SourceLineage};

pub(super) fn encode_source_identity(
    encoder: &mut Encoder,
    source: &ResolvedSourceIdentity,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encode_package_key(encoder, source.key(), maximum_identity_bytes)?;
    encode_resolution(encoder, source.resolution())
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
            encoder.fixed(lineage.workspace_identity().digest());
            encoder.bytes_bounded(
                lineage.member_path().as_str().as_bytes(),
                maximum_identity_bytes,
            )?;
        }
        SourceLineage::ExternalLocal(lineage) => {
            encoder.byte(4);
            encoder.fixed(lineage.source_context().digest());
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
            encoder.scratch(super::super::hex_length(commit.algorithm()))?;
            encoder.bytes_bounded(commit.to_hex().as_bytes(), 64)?;
            encoder.scratch(super::super::hex_length(tree.algorithm()))?;
            encoder.bytes_bounded(tree.to_hex().as_bytes(), 64)?;
            encoder.fixed(content.digest());
        }
        ImmutableSourceResolution::Workspace { content } => {
            encoder.byte(1);
            encoder.fixed(content.digest());
        }
        ImmutableSourceResolution::ExternalLocal { content } => {
            encoder.byte(2);
            encoder.fixed(content.digest());
        }
    }
    Ok(())
}
