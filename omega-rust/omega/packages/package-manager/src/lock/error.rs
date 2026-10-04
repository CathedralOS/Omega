use super::HistoricalPackagePolicyError;
use crate::resolution::graph::CanonicalSourceClosureSubjectError;
use package_evidence::encoding::{PackagePolicyMembershipError, PackageReviewEncodingError};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageLockError {
    UnsupportedVersion,
    CompilerIdentity,
    MissingCompilationInputs,
    TerminalPermissionsMismatch {
        package: semantic_vocabulary::PackageKeyIdentity,
    },
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
    PolicySourceMembership(PackagePolicyMembershipError),
    BoundaryApplicationMismatch,
}

impl fmt::Display for PackageLockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedVersion => "unsupported package lock version; run omega update with this compiler to accept compilation inputs",
            Self::CompilerIdentity => "package lock requires a nonzero SHA256 compiler input identity; run omega update",
            Self::MissingCompilationInputs => "historical package lock lacks accepted compilation inputs; run omega update for fresh review",
            Self::TerminalPermissionsMismatch { package } => return write!(formatter, "package {package:?} terminal permissions differ from retained acceptance; run omega update"),
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
