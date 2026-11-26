// Package identity lines shared by the editable review document and the
// read-only inspection report. Both texts are compared or pinned byte for byte,
// so each caller keeps its own prefixes and Git field labels.

use crate::Hex;
use crate::declarations::PackageKey;
use crate::lock::PackageCheckedContext;
use source_acquisition::{GitTransport, ImmutableSourceResolution, SourceLineage};
use std::fmt::{self, Write};

/// Whether a Git lineage line names its fields (`user … host … port … path …`)
/// or lists them positionally.
#[derive(Clone, Copy)]
pub(super) enum GitFields {
    Positional,
    Labelled,
}

pub(super) fn checked_context(
    output: &mut impl Write,
    prefix: &str,
    context: PackageCheckedContext,
) -> fmt::Result {
    writeln!(
        output,
        "{prefix} occurrence {} target {} execution {}",
        context.purpose().name(),
        context.target().target_name(),
        context
            .build_execution_profile()
            .map_or("none", |profile| profile.target_name())
    )
}

pub(super) fn package_key(
    output: &mut impl Write,
    prefix: &str,
    key: &PackageKey,
    git: GitFields,
) -> fmt::Result {
    write!(
        output,
        "{prefix} {:?} {} ",
        key.name().as_str(),
        Hex(&key.identity().digest())
    )?;
    source_lineage(output, key.source_lineage(), git)
}

pub(super) fn source_lineage(
    output: &mut impl Write,
    lineage: &SourceLineage,
    git: GitFields,
) -> fmt::Result {
    match lineage {
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
            let [user, host, port, path] = match git {
                GitFields::Positional => [""; 4],
                GitFields::Labelled => ["user ", "host ", "port ", "path "],
            };
            writeln!(
                output,
                "git {transport} {user}{:?} {host}{:?} {port}{:?} {path}{:?}",
                source.user(),
                source.host(),
                source.port(),
                source.repository_path()
            )
        }
        SourceLineage::Bundled { library, directory } => writeln!(
            output,
            "bundled {:?} library {}",
            directory.as_str(),
            library.to_hex()
        ),
        SourceLineage::Workspace(source) => {
            writeln!(output, "workspace {:?}", source.member_path().as_str())
        }
        SourceLineage::ExternalLocal(source) => {
            writeln!(output, "local {:?}", source.canonical_absolute_path())
        }
    }
}

pub(super) fn resolution(
    output: &mut impl Write,
    prefix: &str,
    resolution: &ImmutableSourceResolution,
) -> fmt::Result {
    match resolution {
        ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        } => writeln!(
            output,
            "{prefix} git commit {} tree {} content {}",
            commit.to_hex(),
            tree.to_hex(),
            content.to_hex()
        ),
        ImmutableSourceResolution::Bundled { content } => {
            writeln!(output, "{prefix} bundled content {}", content.to_hex())
        }
        ImmutableSourceResolution::Workspace { content } => {
            writeln!(output, "{prefix} workspace content {}", content.to_hex())
        }
        ImmutableSourceResolution::ExternalLocal { content } => {
            writeln!(output, "{prefix} local content {}", content.to_hex())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The review document and the inspection report differ only in how a Git
    // lineage names its fields; both must keep the bytes they wrote before the
    // renderers were shared.
    #[test]
    fn git_lineage_keeps_each_callers_field_labels() {
        let lineage = SourceLineage::git("ssh://git@example.com:2222/team/kernel.git").unwrap();
        let SourceLineage::Git(source) = &lineage else {
            panic!("a generic Git host keeps a generic Git lineage");
        };
        let (user, host, port, path) = (
            source.user(),
            source.host(),
            source.port(),
            source.repository_path(),
        );
        for (git, expected) in [
            (
                GitFields::Positional,
                format!("git ssh {user:?} {host:?} {port:?} {path:?}\n"),
            ),
            (
                GitFields::Labelled,
                format!("git ssh user {user:?} host {host:?} port {port:?} path {path:?}\n"),
            ),
        ] {
            let mut text = String::new();
            source_lineage(&mut text, &lineage, git).unwrap();
            assert_eq!(text, expected);
        }
    }
}
