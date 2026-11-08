// We retain recheckable source-selection evidence establishing which package
// root was selected from one source (`Root`, `GitWorkspace`, or `HistoricalGit`
// for old-source comparison).

use super::git::workspace::{GitWorkspaceSelectionError, GitWorkspaceSelectionEvidence};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PackageSourceSelectionEvidence {
    Root,
    GitWorkspace(GitWorkspaceSelectionEvidence),
    HistoricalGit {
        _source: std::sync::Arc<package_source::ResolvedGitSource>,
    },
}

impl PackageSourceSelectionEvidence {
    pub(crate) fn observations(&self) -> &[super::WorkspaceMemberObservation] {
        match self {
            Self::Root | Self::HistoricalGit { .. } => &[],
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
