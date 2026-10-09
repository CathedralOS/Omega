//! Evidence establishing which package root was selected from one source.

use super::git::workspace::{
    GitWorkspaceSelectionError, GitWorkspaceSelectionEvidence, GitWorkspaceSelectionPlan,
};
use std::fmt;

/// Recheckable source-selection evidence retained outside package source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSourceSelectionEvidence {
    /// The source root itself is the selected package.
    Root,
    /// A declared member was selected from an authenticated Git workspace.
    GitWorkspace(GitWorkspaceSelectionEvidence),
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
