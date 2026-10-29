//! Evidence establishing which package root was selected from one source.

use super::git::workspace::{GitWorkspaceSelectionError, GitWorkspaceSelectionEvidence};
use std::fmt;

/// Recheckable source-selection evidence retained outside package source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PackageSourceSelectionEvidence {
    /// The source root itself is the selected package.
    Root,
    /// A declared member was selected from an authenticated Git workspace.
    GitWorkspace(GitWorkspaceSelectionEvidence),
}

impl PackageSourceSelectionEvidence {
    pub(crate) fn observations(&self) -> &[super::WorkspaceMemberObservation] {
        match self {
            Self::Root => &[],
            Self::GitWorkspace(evidence) => evidence.observations(),
        }
    }

    pub fn revalidate(&self) -> Result<(), PackageSourceSelectionEvidenceError> {
        let Self::GitWorkspace(evidence) = self else {
            return Ok(());
        };
        evidence.revalidate()
    }
}

/// Why retained source-selection evidence no longer replays.
///
/// The cause vocabulary stays crate-internal: callers outside this crate
/// report the refusal through [`fmt::Display`] rather than re-deriving it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceSelectionEvidenceError(PackageSourceSelectionEvidenceFailure);

#[derive(Debug, Clone, PartialEq, Eq)]
enum PackageSourceSelectionEvidenceFailure {
    Selection(GitWorkspaceSelectionError),
    Executed(String),
}

impl PackageSourceSelectionEvidenceError {
    pub(crate) fn executed(reason: String) -> Self {
        Self(PackageSourceSelectionEvidenceFailure::Executed(reason))
    }
}

impl From<GitWorkspaceSelectionError> for PackageSourceSelectionEvidenceError {
    fn from(error: GitWorkspaceSelectionError) -> Self {
        Self(PackageSourceSelectionEvidenceFailure::Selection(error))
    }
}

impl fmt::Display for PackageSourceSelectionEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            PackageSourceSelectionEvidenceFailure::Selection(error) => write!(
                formatter,
                "retained Git workspace selection no longer replays: {error}"
            ),
            PackageSourceSelectionEvidenceFailure::Executed(reason) => write!(
                formatter,
                "executed workspace selection no longer agrees: {reason}"
            ),
        }
    }
}

impl std::error::Error for PackageSourceSelectionEvidenceError {}
