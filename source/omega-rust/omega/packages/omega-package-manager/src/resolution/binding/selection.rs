//! Evidence establishing which package root was selected from one source.

#[cfg(test)]
use super::git_selection::MAX_BUILD_DECLARATION_BYTES;
use super::git_selection::{
    GitWorkspaceMemberBuild, GitWorkspaceSelectionError, GitWorkspaceSelectionPlan,
    account_declaration_bytes,
};
use omega_build_declarations::WorkspaceMemberPath;
use std::fmt;
#[cfg(test)]
use std::fs::File;
#[cfg(test)]
use std::io::{self, Read};
#[cfg(test)]
use std::path::Path;

/// Recheckable source-selection evidence retained outside package source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSourceSelectionEvidence {
    /// The source root itself is the selected package.
    Root,
    /// A declared member was selected from an authenticated Git workspace.
    GitWorkspace(GitWorkspaceSelectionEvidence),
}

/// Raw authenticated declarations retained separately from the selected
/// member's compilation root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorkspaceSelectionDeclarations {
    root: Vec<u8>,
    members: Vec<(WorkspaceMemberPath, Vec<u8>)>,
}

impl GitWorkspaceSelectionDeclarations {
    pub fn new(root: Vec<u8>, members: Vec<(WorkspaceMemberPath, Vec<u8>)>) -> Self {
        Self { root, members }
    }

    pub fn root(&self) -> &[u8] {
        &self.root
    }

    pub fn members(&self) -> &[(WorkspaceMemberPath, Vec<u8>)] {
        &self.members
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitWorkspaceSelectionEvidence {
    plan: GitWorkspaceSelectionPlan,
    declarations: GitWorkspaceSelectionDeclarations,
}

impl GitWorkspaceSelectionEvidence {
    pub fn new(
        plan: GitWorkspaceSelectionPlan,
        declarations: GitWorkspaceSelectionDeclarations,
    ) -> Self {
        Self { plan, declarations }
    }

    pub const fn plan(&self) -> &GitWorkspaceSelectionPlan {
        &self.plan
    }

    pub const fn declarations(&self) -> &GitWorkspaceSelectionDeclarations {
        &self.declarations
    }

    pub fn for_declared_member(&self, member_path: &WorkspaceMemberPath) -> Option<Self> {
        self.plan
            .for_declared_member(member_path)
            .map(|plan| Self::new(plan, self.declarations.clone()))
    }

    pub fn revalidate(&self) -> Result<(), PackageSourceSelectionEvidenceError> {
        let root_build = self.declarations().root();
        let mut total_bytes = account_declaration_bytes(0, root_build)
            .map_err(PackageSourceSelectionEvidenceError::Selection)?;
        let mut member_builds = Vec::with_capacity(self.plan().members().len());
        for (member_path, bytes) in self.declarations().members() {
            total_bytes = account_declaration_bytes(total_bytes, bytes)
                .map_err(PackageSourceSelectionEvidenceError::Selection)?;
            member_builds.push((member_path.clone(), bytes.as_slice()));
        }
        let supplied = member_builds
            .iter()
            .map(|(member_path, bytes)| GitWorkspaceMemberBuild::new(member_path, bytes))
            .collect::<Vec<_>>();
        self.plan()
            .replay(root_build, &supplied)
            .map_err(PackageSourceSelectionEvidenceError::Selection)
    }
}

impl PackageSourceSelectionEvidence {
    pub const fn git_workspace(&self) -> Option<&GitWorkspaceSelectionPlan> {
        match self {
            Self::Root => None,
            Self::GitWorkspace(evidence) => Some(evidence.plan()),
        }
    }

    pub fn revalidate(&self) -> Result<(), PackageSourceSelectionEvidenceError> {
        let Self::GitWorkspace(evidence) = self else {
            return Ok(());
        };
        evidence.revalidate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSourceSelectionEvidenceError {
    Selection(GitWorkspaceSelectionError),
}

impl fmt::Display for PackageSourceSelectionEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Selection(error) => write!(
                formatter,
                "retained Git workspace selection no longer replays: {error}"
            ),
        }
    }
}

impl std::error::Error for PackageSourceSelectionEvidenceError {}

#[cfg(test)]
pub(super) fn read_bounded_declaration(path: &Path) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let maximum_read = u64::try_from(MAX_BUILD_DECLARATION_BYTES)
        .expect("compiler-owned declaration ceiling fits u64")
        .saturating_add(1);
    let mut bytes = Vec::new();
    file.take(maximum_read).read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn declaration_reader_never_allocates_past_the_rejection_sentinel() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time follows Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "omega-git-declaration-limit-{}-{stamp}",
            std::process::id()
        ));
        std::fs::write(&path, vec![b'x'; MAX_BUILD_DECLARATION_BYTES + 4096])
            .expect("write oversized declaration");

        let bytes = read_bounded_declaration(&path).expect("read bounded declaration");

        assert_eq!(bytes.len(), MAX_BUILD_DECLARATION_BYTES + 1);
        let _ = std::fs::remove_file(path);
    }
}
