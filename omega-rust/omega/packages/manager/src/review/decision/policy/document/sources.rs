//! Source identities and paths; package-origin strings are always quoted data.

use super::output::{Hex, Output};
use crate::declarations::PackageKey;
use crate::review::PackagePolicyDependencyPath;
use package_source::{GitTransport, ImmutableSourceResolution, SourceLineage};
use std::fmt::{self, Write};

pub(super) fn package_key(output: &mut Output, prefix: &str, key: &PackageKey) -> fmt::Result {
    write!(
        output,
        "{prefix} {:?} {} ",
        key.name().as_str(),
        Hex(&key.identity().digest())
    )?;
    match key.source_lineage() {
        SourceLineage::GitHub(source) => writeln!(
            output,
            "github {:?} {:?}",
            source.owner(),
            source.repository()
        ),
        SourceLineage::GitLab(source) => writeln!(output, "gitlab {:?}", source.repository_path()),
        SourceLineage::Git(source) => {
            let transport = match source.transport() {
                GitTransport::Https => "https",
                GitTransport::SshUrl => "ssh",
                GitTransport::ScpLike => "scp",
            };
            writeln!(
                output,
                "git {transport} {:?} {:?} {:?} {:?}",
                source.user(),
                source.host(),
                source.port(),
                source.repository_path()
            )
        }
        SourceLineage::Workspace(source) => {
            writeln!(output, "workspace {:?}", source.member_path().as_str())
        }
        SourceLineage::ExternalLocal(source) => {
            writeln!(output, "local {:?}", source.canonical_absolute_path())
        }
    }
}

pub(super) fn resolution(
    output: &mut Output,
    prefix: &str,
    resolution: Option<&ImmutableSourceResolution>,
) -> fmt::Result {
    match resolution {
        None => writeln!(output, "{prefix} none"),
        Some(ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        }) => writeln!(
            output,
            "{prefix} git commit {} tree {} content {}",
            commit.to_hex(),
            tree.to_hex(),
            content.to_hex()
        ),
        Some(ImmutableSourceResolution::Workspace { content }) => {
            writeln!(output, "{prefix} workspace content {}", content.to_hex())
        }
        Some(ImmutableSourceResolution::ExternalLocal { content }) => {
            writeln!(output, "{prefix} local content {}", content.to_hex())
        }
    }
}

pub(super) fn path(
    output: &mut Output,
    prefix: &str,
    path: Option<&PackagePolicyDependencyPath>,
) -> fmt::Result {
    let Some(path) = path else {
        return writeln!(output, "{prefix} none");
    };
    write!(output, "{prefix} {}", Hex(&path.root().digest()))?;
    for step in path.steps() {
        write!(
            output,
            " -> {:?} {}",
            step.alias(),
            Hex(&step.target().digest())
        )?;
    }
    writeln!(output)
}
