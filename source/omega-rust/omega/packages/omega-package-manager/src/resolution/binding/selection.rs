//! Evidence establishing which package root was selected from one source.

use super::git_selection::{
    GitWorkspaceMemberBuild, GitWorkspaceSelectionError, GitWorkspaceSelectionPlan,
    MAX_BUILD_DECLARATION_BYTES, account_declaration_bytes,
};
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Recheckable source-selection evidence retained outside package source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSourceSelectionEvidence {
    /// The source root itself is the selected package.
    Root,
    /// A declared member was selected from an authenticated Git workspace.
    GitWorkspace(GitWorkspaceSelectionPlan),
}

impl PackageSourceSelectionEvidence {
    pub const fn git_workspace(&self) -> Option<&GitWorkspaceSelectionPlan> {
        match self {
            Self::Root => None,
            Self::GitWorkspace(plan) => Some(plan),
        }
    }

    pub fn revalidate(
        &self,
        acquisition_root: &Path,
    ) -> Result<(), PackageSourceSelectionEvidenceError> {
        let Self::GitWorkspace(plan) = self else {
            return Ok(());
        };
        let root_build = read_declaration(acquisition_root.join("build.omg"))?;
        let mut total_bytes = account_declaration_bytes(0, &root_build)
            .map_err(PackageSourceSelectionEvidenceError::Selection)?;
        let mut member_builds = Vec::with_capacity(plan.members().len());
        for member in plan.members() {
            let path = acquisition_root
                .join(member.member_path().as_str())
                .join("build.omg");
            let bytes = read_declaration(path)?;
            total_bytes = account_declaration_bytes(total_bytes, &bytes)
                .map_err(PackageSourceSelectionEvidenceError::Selection)?;
            member_builds.push((member.member_path().clone(), bytes));
        }
        let supplied = member_builds
            .iter()
            .map(|(member_path, bytes)| GitWorkspaceMemberBuild::new(member_path, bytes.as_slice()))
            .collect::<Vec<_>>();
        plan.replay(&root_build, &supplied)
            .map_err(PackageSourceSelectionEvidenceError::Selection)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSourceSelectionEvidenceError {
    Read { path: PathBuf, message: String },
    Selection(GitWorkspaceSelectionError),
}

impl fmt::Display for PackageSourceSelectionEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, message } => write!(
                formatter,
                "cannot read retained Git declaration `{}`: {message}",
                path.display()
            ),
            Self::Selection(error) => write!(
                formatter,
                "retained Git workspace selection no longer replays: {error}"
            ),
        }
    }
}

impl std::error::Error for PackageSourceSelectionEvidenceError {}

fn read_declaration(path: PathBuf) -> Result<Vec<u8>, PackageSourceSelectionEvidenceError> {
    read_bounded_declaration(&path).map_err(|error| PackageSourceSelectionEvidenceError::Read {
        path,
        message: error.to_string(),
    })
}

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
