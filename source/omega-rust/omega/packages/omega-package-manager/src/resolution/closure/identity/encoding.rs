use super::validation::{validate_package_key, validate_source_lineage};
use super::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubjectError,
    CanonicalSourceClosureSubjectFingerprint, CanonicalSourceClosureSubjectLimits,
    SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION, SOURCE_CLOSURE_SUBJECT_FINGERPRINT_DOMAIN,
    SOURCE_CLOSURE_SUBJECT_MAGIC,
};
use crate::resolution::closure::ResolvedSourceIdentity;
use omega_package_source::{
    AliasName, ExternalLocalLineage, ExternalSourceContext, GitCommitId, GitTransport, GitTreeId,
    ImmutableSourceResolution, PackageKey, PackageName, SourceContentDigest, SourceLineage,
    WorkspaceLineageIdentity, WorkspaceMemberLineage, WorkspaceMemberPath,
};
use sha2::{Digest, Sha256};

pub(super) fn encode_subject(
    root: &CanonicalRootSourceSelection,
    packages: &[ResolvedSourceIdentity],
    dependency_requests: &[CanonicalDependencySourceSelection],
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<Vec<u8>, CanonicalSourceClosureSubjectError> {
    let mut encoder = Encoder::new();
    encoder.fixed(SOURCE_CLOSURE_SUBJECT_MAGIC);
    encoder.u16(SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION);
    encode_root_selection(&mut encoder, root, limits)?;
    encoder.count(packages.len())?;
    for source in packages {
        encode_source_identity(&mut encoder, source, limits.maximum_identity_bytes)?;
    }
    encoder.count(dependency_requests.len())?;
    for request in dependency_requests {
        encode_dependency_selection(&mut encoder, request, limits)?;
    }
    Ok(encoder.finish())
}

fn encode_root_selection(
    encoder: &mut Encoder,
    root: &CanonicalRootSourceSelection,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match &root.request {
        CanonicalRootSourceRequest::Git {
            requested_locator,
            requested_revision,
        } => {
            encoder.byte(0);
            encoder.bytes_bounded(requested_locator.as_bytes(), limits.maximum_request_bytes)?;
            encoder.bytes_bounded(requested_revision.as_bytes(), limits.maximum_request_bytes)?;
        }
        CanonicalRootSourceRequest::WorkspaceMember {
            workspace_root_source,
            member_path,
            requested_workspace_root,
        } => {
            encoder.byte(1);
            encode_source_lineage(
                encoder,
                workspace_root_source,
                limits.maximum_identity_bytes,
            )?;
            encoder.bytes_bounded(
                member_path.as_str().as_bytes(),
                limits.maximum_request_bytes,
            )?;
            encoder.bytes_bounded(requested_workspace_root, limits.maximum_request_bytes)?;
        }
        CanonicalRootSourceRequest::ExternalLocal {
            requested_root,
            source_context,
        } => {
            encoder.byte(2);
            encoder.bytes_bounded(requested_root, limits.maximum_request_bytes)?;
            encoder.fixed(&decode_hex_32(&source_context.to_hex())?);
        }
    }
    encode_source_identity(encoder, &root.selected, limits.maximum_identity_bytes)
}

pub(super) fn decode_root_selection(
    decoder: &mut Decoder<'_>,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<CanonicalRootSourceSelection, CanonicalSourceClosureSubjectError> {
    let request = match decoder.byte()? {
        0 => CanonicalRootSourceRequest::Git {
            requested_locator: decoder.string(limits.maximum_request_bytes)?,
            requested_revision: decoder.string(limits.maximum_request_bytes)?,
        },
        1 => CanonicalRootSourceRequest::WorkspaceMember {
            workspace_root_source: decode_source_lineage(decoder, limits.maximum_identity_bytes)?,
            member_path: WorkspaceMemberPath::parse(&decoder.string(limits.maximum_request_bytes)?)
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new(
                        "invalid workspace member path in root request",
                    )
                })?,
            requested_workspace_root: decoder.bytes(limits.maximum_request_bytes)?.to_vec(),
        },
        2 => CanonicalRootSourceRequest::ExternalLocal {
            requested_root: decoder.bytes(limits.maximum_request_bytes)?.to_vec(),
            source_context: ExternalSourceContext::parse_hex(&encode_hex(&decoder.array_32()?))
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new(
                        "invalid external source context in root request",
                    )
                })?,
        },
        _ => {
            return Err(CanonicalSourceClosureSubjectError::new(
                "invalid root source-request tag",
            ));
        }
    };
    let selected = decode_source_identity(decoder, limits.maximum_identity_bytes)?;
    Ok(CanonicalRootSourceSelection { request, selected })
}

fn encode_dependency_selection(
    encoder: &mut Encoder,
    selection: &CanonicalDependencySourceSelection,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encode_package_key(encoder, &selection.requester, limits.maximum_identity_bytes)?;
    encoder.u32(u32::try_from(selection.dependency_index).map_err(|_| {
        CanonicalSourceClosureSubjectError::new("dependency ordinal exceeds canonical range")
    })?);
    match &selection.request {
        CanonicalDependencySourceRequest::Path {
            explicit_alias,
            location,
        } => {
            encoder.byte(0);
            encode_optional_alias(encoder, explicit_alias, limits.maximum_identity_bytes)?;
            encoder.bytes_bounded(location.as_bytes(), limits.maximum_request_bytes)?;
        }
        CanonicalDependencySourceRequest::Git {
            explicit_alias,
            repository,
            revision,
        } => {
            encoder.byte(1);
            encode_optional_alias(encoder, explicit_alias, limits.maximum_identity_bytes)?;
            encoder.bytes_bounded(repository.as_bytes(), limits.maximum_request_bytes)?;
            encoder.bytes_bounded(revision.as_bytes(), limits.maximum_request_bytes)?;
        }
    }
    encoder.bytes_bounded(
        selection.alias.as_str().as_bytes(),
        limits.maximum_identity_bytes,
    )?;
    encode_source_identity(encoder, &selection.selected, limits.maximum_identity_bytes)
}

pub(super) fn decode_dependency_selection(
    decoder: &mut Decoder<'_>,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<CanonicalDependencySourceSelection, CanonicalSourceClosureSubjectError> {
    let requester = decode_package_key(decoder, limits.maximum_identity_bytes)?;
    let dependency_index = usize::try_from(decoder.u32()?).map_err(|_| {
        CanonicalSourceClosureSubjectError::new("dependency ordinal exceeds platform range")
    })?;
    let request = match decoder.byte()? {
        0 => CanonicalDependencySourceRequest::Path {
            explicit_alias: decode_optional_alias(decoder, limits.maximum_identity_bytes)?,
            location: decoder.string(limits.maximum_request_bytes)?,
        },
        1 => CanonicalDependencySourceRequest::Git {
            explicit_alias: decode_optional_alias(decoder, limits.maximum_identity_bytes)?,
            repository: decoder.string(limits.maximum_request_bytes)?,
            revision: decoder.string(limits.maximum_request_bytes)?,
        },
        _ => {
            return Err(CanonicalSourceClosureSubjectError::new(
                "invalid dependency source-request tag",
            ));
        }
    };
    let alias = AliasName::parse(decoder.string(limits.maximum_identity_bytes)?).map_err(|_| {
        CanonicalSourceClosureSubjectError::new("invalid resolved dependency alias")
    })?;
    let selected = decode_source_identity(decoder, limits.maximum_identity_bytes)?;
    Ok(CanonicalDependencySourceSelection {
        requester,
        dependency_index,
        request,
        alias,
        selected,
    })
}

fn encode_optional_alias(
    encoder: &mut Encoder,
    alias: &Option<AliasName>,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match alias {
        None => encoder.byte(0),
        Some(alias) => {
            encoder.byte(1);
            encoder.bytes_bounded(alias.as_str().as_bytes(), maximum_identity_bytes)?;
        }
    }
    Ok(())
}

fn decode_optional_alias(
    decoder: &mut Decoder<'_>,
    maximum_identity_bytes: usize,
) -> Result<Option<AliasName>, CanonicalSourceClosureSubjectError> {
    match decoder.byte()? {
        0 => Ok(None),
        1 => AliasName::parse(decoder.string(maximum_identity_bytes)?)
            .map(Some)
            .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid explicit alias")),
        _ => Err(CanonicalSourceClosureSubjectError::new(
            "invalid explicit-alias option tag",
        )),
    }
}

fn encode_source_identity(
    encoder: &mut Encoder,
    source: &ResolvedSourceIdentity,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encode_package_key(encoder, source.key(), maximum_identity_bytes)?;
    encode_resolution(encoder, source.resolution())
}

pub(super) fn decode_source_identity(
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

fn encode_package_key(
    encoder: &mut Encoder,
    key: &PackageKey,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    validate_package_key(key, maximum_identity_bytes)?;
    encoder.bytes_bounded(key.name().as_str().as_bytes(), maximum_identity_bytes)?;
    encode_source_lineage(encoder, key.source_lineage(), maximum_identity_bytes)
}

fn decode_package_key(
    decoder: &mut Decoder<'_>,
    maximum_identity_bytes: usize,
) -> Result<PackageKey, CanonicalSourceClosureSubjectError> {
    let name = PackageName::parse(decoder.string(maximum_identity_bytes)?)
        .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid package name"))?;
    let lineage = decode_source_lineage(decoder, maximum_identity_bytes)?;
    Ok(PackageKey::new(name, lineage))
}

fn encode_source_lineage(
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

fn decode_source_lineage(
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
            let member = WorkspaceMemberPath::parse(&decoder.string(maximum_identity_bytes)?)
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
        0 => ImmutableSourceResolution::git(
            GitCommitId::parse_hex(&decoder.string(64)?)
                .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid Git commit ID"))?,
            GitTreeId::parse_hex(&decoder.string(64)?)
                .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid Git tree ID"))?,
            content(decoder)?,
        )
        .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid Git source resolution")),
        1 => Ok(ImmutableSourceResolution::workspace(content(decoder)?)),
        2 => Ok(ImmutableSourceResolution::external_local(content(decoder)?)),
        _ => Err(CanonicalSourceClosureSubjectError::new(
            "invalid immutable-resolution tag",
        )),
    }
}

pub(super) fn fingerprint(bytes: &[u8]) -> CanonicalSourceClosureSubjectFingerprint {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_CLOSURE_SUBJECT_FINGERPRINT_DOMAIN);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    CanonicalSourceClosureSubjectFingerprint(hasher.finalize().into())
}

fn decode_hex_32(value: &str) -> Result<[u8; 32], CanonicalSourceClosureSubjectError> {
    let bytes = decode_hex(value).ok_or_else(|| {
        CanonicalSourceClosureSubjectError::new("invalid 32-byte hexadecimal value")
    })?;
    bytes
        .try_into()
        .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid 32-byte hexadecimal value"))
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|digits| {
            let high = hex_value(digits[0])?;
            let low = hex_value(digits[1])?;
            Some((high << 4) | low)
        })
        .collect()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

pub(super) fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn fixed(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.fixed(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.fixed(&value.to_le_bytes());
    }

    fn count(&mut self, value: usize) -> Result<(), CanonicalSourceClosureSubjectError> {
        self.u32(u32::try_from(value).map_err(|_| {
            CanonicalSourceClosureSubjectError::new("canonical sequence count exceeds u32")
        })?);
        Ok(())
    }

    fn bytes_bounded(
        &mut self,
        value: &[u8],
        maximum_bytes: usize,
    ) -> Result<(), CanonicalSourceClosureSubjectError> {
        if value.len() > maximum_bytes {
            return Err(CanonicalSourceClosureSubjectError::new(
                "canonical field exceeds its byte limit",
            ));
        }
        self.count(value.len())?;
        self.fixed(value);
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

pub(super) struct Decoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Decoder<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], CanonicalSourceClosureSubjectError> {
        let end = self.cursor.checked_add(count).ok_or_else(|| {
            CanonicalSourceClosureSubjectError::new("source-closure subject offset overflow")
        })?;
        let bytes = self.bytes.get(self.cursor..end).ok_or_else(|| {
            CanonicalSourceClosureSubjectError::new("truncated source-closure subject")
        })?;
        self.cursor = end;
        Ok(bytes)
    }

    pub(super) fn expect_fixed(
        &mut self,
        expected: &[u8],
    ) -> Result<(), CanonicalSourceClosureSubjectError> {
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(CanonicalSourceClosureSubjectError::new(
                "invalid source-closure subject header",
            ))
        }
    }

    fn byte(&mut self) -> Result<u8, CanonicalSourceClosureSubjectError> {
        Ok(self.take(1)?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16, CanonicalSourceClosureSubjectError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, CanonicalSourceClosureSubjectError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub(super) fn count(
        &mut self,
        maximum: usize,
    ) -> Result<usize, CanonicalSourceClosureSubjectError> {
        let count = usize::try_from(self.u32()?).map_err(|_| {
            CanonicalSourceClosureSubjectError::new("canonical count exceeds platform range")
        })?;
        if count > maximum {
            return Err(CanonicalSourceClosureSubjectError::new(
                "canonical count exceeds its resource limit",
            ));
        }
        Ok(count)
    }

    fn bytes(&mut self, maximum: usize) -> Result<&'a [u8], CanonicalSourceClosureSubjectError> {
        let count = self.count(maximum)?;
        self.take(count)
    }

    fn string(&mut self, maximum: usize) -> Result<String, CanonicalSourceClosureSubjectError> {
        String::from_utf8(self.bytes(maximum)?.to_vec())
            .map_err(|_| CanonicalSourceClosureSubjectError::new("canonical string is not UTF-8"))
    }

    fn array_32(&mut self) -> Result<[u8; 32], CanonicalSourceClosureSubjectError> {
        Ok(self.take(32)?.try_into().unwrap())
    }

    pub(super) fn finish(self) -> Result<(), CanonicalSourceClosureSubjectError> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(CanonicalSourceClosureSubjectError::new(
                "source-closure subject has trailing bytes",
            ))
        }
    }
}
