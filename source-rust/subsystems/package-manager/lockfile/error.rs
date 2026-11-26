// Closed failure vocabulary for package lock encoding, recovery, and cross-target structural validation.

use super::HistoricalPackagePolicyError;
use crate::resolution::graph::CanonicalSourceClosureSubjectError;
use review_evidence::encoding::{PackagePolicyMembershipFailure, PackageReviewEncodingError};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PackageLockError {
    UnsupportedVersion,
    MultipleBuildEntries,
    WorkspaceRootInput,
    CompilerIdentity,
    NativeBindings(String),
    InvalidFraming,
    ByteLimitExceeded,
    AllocationLimitExceeded,
    AllocationFailed,
    CountLimitExceeded,
    EmptyTargets,
    TargetOrder,
    TargetMismatch,
    ExecutionProfileMismatch,
    SourceGraphMismatch,
    SourceCoverage,
    OccurrenceCoverage,
    DecisionSourceMismatch,
    Source(CanonicalSourceClosureSubjectError),
    Decisions(HistoricalPackagePolicyError),
    Encoding(PackageReviewEncodingError),
    PolicySourceMembership(PackagePolicyMembershipFailure),
    BoundaryApplicationMismatch,
}

impl fmt::Display for PackageLockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WorkspaceRootInput => "a version 4 package lock has no policy occurrence for a workspace root's inputs",
            Self::MultipleBuildEntries => "a version 4 package lock cannot record multiple Build entries for one package",
            Self::UnsupportedVersion => "unsupported package lock version; archive omega.lock and run omega packages update for a fresh review",
            Self::NativeBindings(reason) => return write!(formatter, "package native bindings: {reason}"),
            Self::CompilerIdentity => "package lock requires a nonzero SHA256 compiler input identity; run omega packages update",
            Self::InvalidFraming => "package lock has invalid canonical framing",
            Self::ByteLimitExceeded => "package lock exceeds its aggregate text-byte limit",
            Self::AllocationLimitExceeded => "package lock exceeds its aggregate owned-storage limit",
            Self::AllocationFailed => "package lock allocation failed",
            Self::CountLimitExceeded => "package lock exceeds an aggregate record-count limit",
            Self::EmptyTargets => "package lock has no checked-target section",
            Self::TargetOrder => "package lock target sections are repeated or not canonically ordered",
            Self::TargetMismatch => "package lock occurrence belongs to a different checked target",
            Self::ExecutionProfileMismatch => "package lock occurrences disagree about the explicit build execution profile",
            Self::SourceGraphMismatch => "package lock target sections disagree about the immutable source graph",
            Self::SourceCoverage => "package lock is missing a source package dependency projection",
            Self::OccurrenceCoverage => "package lock occurrence coverage disagrees with the source graph",
            Self::DecisionSourceMismatch => "package lock decisions belong to a different source graph or target",
            Self::BoundaryApplicationMismatch => "package lock boundary demand has no matching operator telescope in its owning baseline",
            Self::PolicySourceMembership(error) => return error.fmt(formatter),
            Self::Source(error) => return error.fmt(formatter),
            Self::Decisions(error) => return error.fmt(formatter),
            Self::Encoding(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for PackageLockError {}
