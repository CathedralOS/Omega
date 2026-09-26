//! Readable source-qualified keys and immutable resolutions.

use super::framing::{Reader, Writer};
use super::{Error, Limits};
use crate::declarations::PackageKey;
use crate::resolution::graph::ResolvedSourceIdentity;
use package_source::{
    ExternalLocalLineage, ExternalSourceContext, GitCommitId, GitTransport, GitTreeId,
    ImmutableSourceResolution, SourceContentDigest, SourceLineage, WorkspaceLineageIdentity,
    WorkspaceMemberLineage,
};

pub(super) fn write_source(
    writer: &mut Writer,
    source: &ResolvedSourceIdentity,
) -> Result<(), Error> {
    writer.row("source", &[])?;
    write_key(writer, source.key())?;
    match source.resolution() {
        ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        } => {
            writer
                .budget
                .charge(hex_length(commit.algorithm()) + hex_length(tree.algorithm()) + 64)?;
            writer.row(
                "resolution git",
                &[
                    commit.to_hex().as_bytes(),
                    tree.to_hex().as_bytes(),
                    content.to_hex().as_bytes(),
                ],
            )
        }
        ImmutableSourceResolution::Workspace { content } => {
            writer.budget.charge(64)?;
            writer.row("resolution workspace", &[content.to_hex().as_bytes()])
        }
        ImmutableSourceResolution::ExternalLocal { content } => {
            writer.budget.charge(64)?;
            writer.row("resolution external-local", &[content.to_hex().as_bytes()])
        }
    }
}

pub(super) fn read_source(
    reader: &mut Reader<'_>,
    limits: Limits,
) -> Result<ResolvedSourceIdentity, Error> {
    reader.expect("source")?;
    let key = read_key(reader, limits)?;
    reader.expect("resolution")?;
    let resolution = match reader.atom()? {
        "git" => {
            let commit = GitCommitId::parse_hex(&reader.hex_string(64)?)
                .map_err(|_| Error::new("invalid text Git commit"))?;
            let tree = GitTreeId::parse_hex(&reader.hex_string(64)?)
                .map_err(|_| Error::new("invalid text Git tree"))?;
            let content = read_content(reader)?;
            reader
                .budget
                .charge(ImmutableSourceResolution::git_recovery_owned_bytes(&tree))?;
            let resolution = ImmutableSourceResolution::git(commit, tree)
                .map_err(|_| Error::new("invalid text Git resolution"))?;
            if resolution.content() != &content {
                return Err(Error::new("text Git content does not match its tree"));
            }
            resolution
        }
        "workspace" => ImmutableSourceResolution::workspace(read_content(reader)?),
        "external-local" => ImmutableSourceResolution::external_local(read_content(reader)?),
        _ => return Err(Error::new("unknown text source resolution")),
    };
    ResolvedSourceIdentity::new(key, resolution)
        .map_err(|_| Error::new("text resolution disagrees with its lineage"))
}

fn read_content(reader: &mut Reader<'_>) -> Result<SourceContentDigest, Error> {
    SourceContentDigest::parse_hex(&reader.hex_string(64)?)
        .map_err(|_| Error::new("invalid text content digest"))
}

pub(super) fn write_key(writer: &mut Writer, key: &PackageKey) -> Result<(), Error> {
    writer.row("name", &[key.name().as_str().as_bytes()])?;
    write_lineage(writer, key.source_lineage())
}

pub(super) fn read_key(reader: &mut Reader<'_>, limits: Limits) -> Result<PackageKey, Error> {
    reader.expect("name")?;
    let name = reader.package_name(limits.maximum_identity_bytes)?;
    Ok(PackageKey::new(name, read_lineage(reader, limits)?))
}

pub(super) fn write_lineage(writer: &mut Writer, lineage: &SourceLineage) -> Result<(), Error> {
    match lineage {
        SourceLineage::GitHub(lineage) => writer.row(
            "lineage github",
            &[lineage.owner().as_bytes(), lineage.repository().as_bytes()],
        ),
        SourceLineage::GitLab(lineage) => {
            writer.row("lineage gitlab", &[lineage.repository_path().as_bytes()])
        }
        SourceLineage::Git(lineage) => {
            let mut digits = [0u8; 5];
            let mut start = digits.len();
            if let Some(mut port) = lineage.port() {
                loop {
                    start -= 1;
                    digits[start] = b'0' + u8::try_from(port % 10).expect("decimal digit");
                    port /= 10;
                    if port == 0 {
                        break;
                    }
                }
            }
            let transport = match lineage.transport() {
                GitTransport::Https => "lineage git https",
                GitTransport::SshUrl => "lineage git ssh",
                GitTransport::ScpLike => "lineage git scp",
            };
            writer.row(
                transport,
                &[
                    lineage.user().unwrap_or_default().as_bytes(),
                    lineage.host().as_bytes(),
                    &digits[start..],
                    lineage.repository_path().as_bytes(),
                ],
            )
        }
        SourceLineage::Workspace(lineage) => {
            writer.budget.charge(64)?;
            writer.row(
                "lineage workspace",
                &[
                    lineage.workspace_identity().to_hex().as_bytes(),
                    lineage.member_path().as_str().as_bytes(),
                ],
            )
        }
        SourceLineage::ExternalLocal(lineage) => {
            writer.budget.charge(64)?;
            writer.row(
                "lineage external-local",
                &[
                    lineage.source_context().to_hex().as_bytes(),
                    lineage
                        .canonical_absolute_path()
                        .to_str()
                        .ok_or_else(|| Error::new("text external-local lineage requires UTF-8"))?
                        .as_bytes(),
                ],
            )
        }
    }
}

pub(super) fn read_lineage(
    reader: &mut Reader<'_>,
    limits: Limits,
) -> Result<SourceLineage, Error> {
    reader.expect("lineage")?;
    let maximum = limits.maximum_identity_bytes;
    let locator = match reader.atom()? {
        "github" => {
            let owner = reader.string(maximum)?;
            let repository = reader.string(maximum)?;
            join(
                reader,
                &["https://github.com/", &owner, "/", &repository, ".git"],
            )?
        }
        "gitlab" => {
            let path = reader.string(maximum)?;
            join(reader, &["https://gitlab.com/", &path, ".git"])?
        }
        "git" => {
            let transport = reader.atom()?;
            let user = reader.string(maximum)?;
            let host = reader.string(maximum)?;
            let port = reader.string(5)?;
            if !port.is_empty() && port.parse::<u16>().is_err() {
                return Err(Error::new("invalid text Git port"));
            }
            let path = reader.string(maximum)?;
            let user_separator = if user.is_empty() { "" } else { "@" };
            let port_separator = if port.is_empty() { "" } else { ":" };
            match transport {
                "https" => join(
                    reader,
                    &[
                        "https://",
                        &user,
                        user_separator,
                        &host,
                        port_separator,
                        &port,
                        "/",
                        &path,
                    ],
                )?,
                "ssh" => join(
                    reader,
                    &[
                        "ssh://",
                        &user,
                        user_separator,
                        &host,
                        port_separator,
                        &port,
                        "/",
                        &path,
                    ],
                )?,
                "scp" if port.is_empty() => {
                    join(reader, &[&user, user_separator, &host, ":", &path])?
                }
                _ => return Err(Error::new("invalid text Git transport")),
            }
        }
        "workspace" => {
            let workspace = WorkspaceLineageIdentity::parse_hex(&reader.hex_string(64)?)
                .map_err(|_| Error::new("invalid text workspace identity"))?;
            let member = reader.relative_path(maximum)?;
            return Ok(SourceLineage::Workspace(WorkspaceMemberLineage::new(
                workspace, member,
            )));
        }
        "external-local" => {
            let context = ExternalSourceContext::parse_hex(&reader.hex_string(64)?)
                .map_err(|_| Error::new("invalid text external context"))?;
            let path = reader.string(maximum)?;
            reader.budget.charge(
                ExternalLocalLineage::recovery_owned_bytes(&path)
                    .ok_or_else(|| Error::new("external-local recovery allowance overflow"))?,
            )?;
            return ExternalLocalLineage::from_recovered_canonical_path(path, context)
                .map(SourceLineage::ExternalLocal)
                .map_err(|_| Error::new("invalid text external-local lineage"));
        }
        _ => return Err(Error::new("unknown text source lineage")),
    };
    reader.budget.charge(
        SourceLineage::git_recovery_owned_bytes(&locator)
            .ok_or_else(|| Error::new("Git lineage recovery allowance overflow"))?,
    )?;
    SourceLineage::git(&locator).map_err(|_| Error::new("invalid text Git lineage"))
}

fn join(reader: &mut Reader<'_>, parts: &[&str]) -> Result<String, Error> {
    let length = parts
        .iter()
        .try_fold(0usize, |length, part| length.checked_add(part.len()))
        .ok_or_else(|| Error::new("text Git locator length overflow"))?;
    let mut bytes = reader.budget.reserve(length)?;
    for part in parts {
        bytes.extend_from_slice(part.as_bytes());
    }
    Ok(String::from_utf8(bytes).expect("joined UTF-8 locator"))
}

fn hex_length(algorithm: package_source::GitObjectIdAlgorithm) -> usize {
    match algorithm {
        package_source::GitObjectIdAlgorithm::Sha1 => 40,
        package_source::GitObjectIdAlgorithm::Sha256 => 64,
    }
}
