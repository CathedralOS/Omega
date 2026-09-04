//! Readable source-qualified keys and immutable resolutions.

use super::framing::{Reader, Writer};
use super::{Error, Limits};
use crate::declarations::{PackageKey, PackageName};
use crate::resolution::graph::ResolvedSourceIdentity;
use omega_package_source::{
    ExternalLocalLineage, ExternalSourceContext, GitCommitId, GitTransport, GitTreeId,
    ImmutableSourceResolution, SourceContentDigest, SourceLineage, SourceRelativePath,
    WorkspaceLineageIdentity, WorkspaceMemberLineage,
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
        } => writer.row(
            "resolution git",
            &[
                commit.to_hex().as_bytes(),
                tree.to_hex().as_bytes(),
                content.to_hex().as_bytes(),
            ],
        ),
        ImmutableSourceResolution::Workspace { content } => {
            writer.row("resolution workspace", &[content.to_hex().as_bytes()])
        }
        ImmutableSourceResolution::ExternalLocal { content } => {
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
            let commit = GitCommitId::parse_hex(&reader.string(64)?)
                .map_err(|_| Error::new("invalid text Git commit"))?;
            let tree = GitTreeId::parse_hex(&reader.string(64)?)
                .map_err(|_| Error::new("invalid text Git tree"))?;
            let content = read_content(reader)?;
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
    SourceContentDigest::parse_hex(&reader.string(64)?)
        .map_err(|_| Error::new("invalid text content digest"))
}

pub(super) fn write_key(writer: &mut Writer, key: &PackageKey) -> Result<(), Error> {
    writer.row("name", &[key.name().as_str().as_bytes()])?;
    write_lineage(writer, key.source_lineage())
}

pub(super) fn read_key(reader: &mut Reader<'_>, limits: Limits) -> Result<PackageKey, Error> {
    reader.expect("name")?;
    let name = PackageName::parse(reader.string(limits.maximum_identity_bytes)?)
        .map_err(|_| Error::new("invalid text package name"))?;
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
            let transport = match lineage.transport() {
                GitTransport::Https => "https",
                GitTransport::SshUrl => "ssh",
                GitTransport::ScpLike => "scp",
            };
            writer.row(
                &format!("lineage git {transport}"),
                &[
                    lineage.user().unwrap_or_default().as_bytes(),
                    lineage.host().as_bytes(),
                    lineage
                        .port()
                        .map(|port| port.to_string())
                        .unwrap_or_default()
                        .as_bytes(),
                    lineage.repository_path().as_bytes(),
                ],
            )
        }
        SourceLineage::Workspace(lineage) => writer.row(
            "lineage workspace",
            &[
                lineage.workspace_identity().to_hex().as_bytes(),
                lineage.member_path().as_str().as_bytes(),
            ],
        ),
        SourceLineage::ExternalLocal(lineage) => writer.row(
            "lineage external-local",
            &[
                lineage.source_context().to_hex().as_bytes(),
                lineage
                    .canonical_absolute_path()
                    .to_str()
                    .ok_or_else(|| Error::new("text external-local lineage requires UTF-8"))?
                    .as_bytes(),
            ],
        ),
    }
}

pub(super) fn read_lineage(
    reader: &mut Reader<'_>,
    limits: Limits,
) -> Result<SourceLineage, Error> {
    reader.expect("lineage")?;
    let maximum = limits.maximum_identity_bytes;
    let locator = match reader.atom()? {
        "github" => format!(
            "https://github.com/{}/{}.git",
            reader.string(maximum)?,
            reader.string(maximum)?
        ),
        "gitlab" => format!("https://gitlab.com/{}.git", reader.string(maximum)?),
        "git" => {
            let transport = reader.atom()?;
            let user = reader.string(maximum)?;
            let host = reader.string(maximum)?;
            let port = reader.string(5)?;
            if !port.is_empty() && port.parse::<u16>().is_err() {
                return Err(Error::new("invalid text Git port"));
            }
            let path = reader.string(maximum)?;
            let user = if user.is_empty() {
                String::new()
            } else {
                format!("{user}@")
            };
            let port = if port.is_empty() {
                String::new()
            } else {
                format!(":{port}")
            };
            match transport {
                "https" => format!("https://{user}{host}{port}/{path}"),
                "ssh" => format!("ssh://{user}{host}{port}/{path}"),
                "scp" if port.is_empty() => format!("{user}{host}:{path}"),
                _ => return Err(Error::new("invalid text Git transport")),
            }
        }
        "workspace" => {
            let workspace = WorkspaceLineageIdentity::parse_hex(&reader.string(64)?)
                .map_err(|_| Error::new("invalid text workspace identity"))?;
            let member = SourceRelativePath::parse(&reader.string(maximum)?)
                .map_err(|_| Error::new("invalid text workspace member"))?;
            return Ok(SourceLineage::Workspace(WorkspaceMemberLineage::new(
                workspace, member,
            )));
        }
        "external-local" => {
            let context = ExternalSourceContext::parse_hex(&reader.string(64)?)
                .map_err(|_| Error::new("invalid text external context"))?;
            return ExternalLocalLineage::from_recovered_canonical_path(
                reader.string(maximum)?,
                context,
            )
            .map(SourceLineage::ExternalLocal)
            .map_err(|_| Error::new("invalid text external-local lineage"));
        }
        _ => return Err(Error::new("unknown text source lineage")),
    };
    SourceLineage::git(&locator).map_err(|_| Error::new("invalid text Git lineage"))
}
