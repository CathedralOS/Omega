use crate::manifest::dependency_projection::DependencyProjectionError;
use std::fmt;
use std::path::{Path, PathBuf};

/// A conservative, non-mutating plan for changing one `build.omg` dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildDependencyEditPlan {
    /// The exact requested row is already present.
    Unchanged,
    /// The source has a canonical edit point and may be replaced atomically
    /// after checking `expected_sha256` again.
    Automatic(BuildFileReplacement),
    /// The source is valid Omega but its layout or intent requires a person or
    /// reviewing agent to place the generated row.
    Manual(BuildDependencyManualPatch),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFileReplacement {
    build_path: PathBuf,
    expected_sha256: [u8; 32],
    replacement_source: String,
}

impl BuildFileReplacement {
    pub(super) fn new(
        build_path: PathBuf,
        expected_sha256: [u8; 32],
        replacement_source: String,
    ) -> Self {
        Self {
            build_path,
            expected_sha256,
            replacement_source,
        }
    }

    pub fn build_path(&self) -> &Path {
        &self.build_path
    }

    pub fn expected_sha256(&self) -> &[u8; 32] {
        &self.expected_sha256
    }

    pub fn replacement_source(&self) -> &str {
        &self.replacement_source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildDependencyManualPatch {
    build_path: PathBuf,
    expected_sha256: [u8; 32],
    reason: BuildDependencyManualReason,
    current_statement: Option<String>,
    proposed_statement: String,
}

impl BuildDependencyManualPatch {
    pub(super) fn new(
        build_path: PathBuf,
        expected_sha256: [u8; 32],
        reason: BuildDependencyManualReason,
        current_statement: Option<String>,
        proposed_statement: String,
    ) -> Self {
        Self {
            build_path,
            expected_sha256,
            reason,
            current_statement,
            proposed_statement,
        }
    }

    pub fn build_path(&self) -> &Path {
        &self.build_path
    }

    pub fn expected_sha256(&self) -> &[u8; 32] {
        &self.expected_sha256
    }

    pub fn reason(&self) -> BuildDependencyManualReason {
        self.reason
    }

    /// Canonical, compiler-generated text for the accepted row, when this is a
    /// replacement. This is never copied from package source.
    pub fn current_statement(&self) -> Option<&str> {
        self.current_statement.as_deref()
    }

    /// Canonical, compiler-generated text for the requested row. Every
    /// caller-controlled string is escaped as an Omega literal.
    pub fn proposed_statement(&self) -> &str {
        &self.proposed_statement
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildDependencyManualReason {
    NonCanonicalBuildSignature,
    NonCanonicalBuildBodyLayout,
    NonCanonicalDependencyRows,
    DependencyRowContainsComment,
    AcceptedRequestMissing,
    AcceptedRequestAmbiguous,
    CandidateAlreadyPresent,
    GeneratedEditRejected,
}

impl fmt::Display for BuildDependencyManualReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NonCanonicalBuildSignature => {
                "the build entry is not the canonical `machine build(builder: &mut Build)` form"
            }
            Self::NonCanonicalBuildBodyLayout => {
                "the build entry closing brace has a noncanonical inline layout"
            }
            Self::NonCanonicalDependencyRows => {
                "the parsed dependency rows cannot be mapped uniquely to direct source statements"
            }
            Self::DependencyRowContainsComment => {
                "the accepted dependency row contains a comment that an automatic rewrite would discard"
            }
            Self::AcceptedRequestMissing => {
                "the accepted dependency row is not present in the current build projection"
            }
            Self::AcceptedRequestAmbiguous => {
                "the accepted dependency row occurs more than once in the current build projection"
            }
            Self::CandidateAlreadyPresent => {
                "the candidate dependency row is already present separately from the accepted row"
            }
            Self::GeneratedEditRejected => {
                "the generated source did not project to the exact requested dependency rows"
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildDependencyEditError {
    ReadBuildFile { path: PathBuf, message: String },
    InvalidBuildFileEncoding { path: PathBuf },
    InvalidBuild(DependencyProjectionError),
}

impl fmt::Display for BuildDependencyEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadBuildFile { path, message } => {
                write!(formatter, "cannot read {}: {message}", path.display())
            }
            Self::InvalidBuildFileEncoding { path } => {
                write!(formatter, "{} is not UTF-8 Omega source", path.display())
            }
            Self::InvalidBuild(error) => {
                write!(formatter, "cannot edit invalid package build: {error}")
            }
        }
    }
}

impl std::error::Error for BuildDependencyEditError {}
