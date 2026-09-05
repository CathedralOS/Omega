use super::HistoricalPackagePolicyError;
use crate::resolution::graph::CanonicalSourceClosureSubjectError;
use omega_package_evidence::encoding::{PackagePolicyRecoveryError, PackageReviewEncodingError};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageLockError {
    UnsupportedVersion,
    InvalidFraming,
    ByteLimitExceeded,
    AllocationLimitExceeded,
    AllocationFailed,
    CountLimitExceeded,
    EmptyTargets,
    TargetOrder,
    TargetMismatch,
    SourceGraphMismatch,
    BaselineCoverage,
    DecisionSourceMismatch,
    Source(CanonicalSourceClosureSubjectError),
    Policy(PackagePolicyRecoveryError),
    Decisions(HistoricalPackagePolicyError),
    Encoding(PackageReviewEncodingError),
}

impl fmt::Display for PackageLockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedVersion => "unsupported package lock version; preserve source pins and recover with a compatible toolchain",
            Self::InvalidFraming => "package lock has invalid canonical framing",
            Self::ByteLimitExceeded => "package lock exceeds its aggregate text-byte limit",
            Self::AllocationLimitExceeded => "package lock exceeds its aggregate owned-storage limit",
            Self::AllocationFailed => "package lock allocation failed",
            Self::CountLimitExceeded => "package lock exceeds an aggregate record-count limit",
            Self::EmptyTargets => "package lock has no checked-target section",
            Self::TargetOrder => "package lock target sections are repeated or not canonically ordered",
            Self::TargetMismatch => "package lock baseline belongs to a different checked target",
            Self::SourceGraphMismatch => "package lock target sections disagree about the immutable source graph",
            Self::BaselineCoverage => "package lock requires one ordered baseline for every source package",
            Self::DecisionSourceMismatch => "package lock decisions belong to a different source graph or target",
            Self::Source(error) => return error.fmt(formatter),
            Self::Policy(error) => return error.fmt(formatter),
            Self::Decisions(error) => return error.fmt(formatter),
            Self::Encoding(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for PackageLockError {}
