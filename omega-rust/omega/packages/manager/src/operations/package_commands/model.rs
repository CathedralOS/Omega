use std::fmt;
use std::path::PathBuf;
use target::TargetProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageCommandKind {
    Install,
    Update,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCommand {
    Install {
        source: String,
        revision: Option<String>,
        alias: Option<String>,
    },
    Update {
        packages: Vec<String>,
        revision: Option<String>,
    },
    Resume {
        kind: PackageCommandKind,
    },
    DiscardReview,
}

#[derive(Debug, Clone)]
pub struct PackageCommandOptions {
    pub project_root: PathBuf,
    /// Explicit targets augment every previously accepted target. Empty uses
    /// existing targets, or the compiler's default target on first admission.
    pub targets: Vec<TargetProfile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageCommandStatus {
    Published,
    ReviewRequired,
    ReviewDiscarded,
}

#[derive(Debug)]
pub struct PackageCommandOutcome {
    pub status: PackageCommandStatus,
    pub report: String,
    pub review_paths: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct PackageCommandError(String);

impl fmt::Display for PackageCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
impl std::error::Error for PackageCommandError {}

pub(super) fn failure(message: impl fmt::Display) -> PackageCommandError {
    PackageCommandError(message.to_string())
}
