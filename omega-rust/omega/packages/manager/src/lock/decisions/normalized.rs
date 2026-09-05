//! V2 history of complete normalized obligations, without fresh authority.
mod capture;
pub(super) mod read;
mod validation;
pub(super) mod write;

use super::model::{
    HistoricalPackagePolicyDecision, HistoricalPackagePolicyDecisionSubject as Subject,
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyError as Error,
    HistoricalPackagePolicyLimits as Limits, HistoricalPackagePolicyRecoveryUsage as Usage,
};
use super::text::{disposition_token, parse_digest, parse_number};
use crate::declarations::BuildDeclarationKind;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject as Source, CanonicalSourceClosureSubjectLimits,
    recover_package_key_text, write_package_key_text,
};
use crate::review::{ReviewOnlyRootPolicyDisposition as Disposition, ReviewOnlyRootRoleContract};

pub(super) const HEADER: &str = "omega-policy-decisions 2\n";

fn source_key_error(error: crate::resolution::graph::CanonicalSourceClosureSubjectError) -> Error {
    if error.is_allocation_limit_exceeded() {
        Error::AllocationLimitExceeded
    } else {
        Error::SourceKey
    }
}

fn key_limits(maximum_bytes: usize) -> CanonicalSourceClosureSubjectLimits {
    CanonicalSourceClosureSubjectLimits {
        maximum_record_bytes: maximum_bytes,
        maximum_identity_bytes: 1024 * 1024,
        ..CanonicalSourceClosureSubjectLimits::default()
    }
}

fn disposition(value: &str) -> Result<Disposition, Error> {
    match value {
        "accept" => Ok(Disposition::AcceptCandidateChange),
        "reject" => Ok(Disposition::RejectCandidateChange),
        _ => Err(Error::InvalidFraming),
    }
}

fn role(value: &str) -> Result<BuildDeclarationKind, Error> {
    match value {
        "package" => Ok(BuildDeclarationKind::Package),
        "application" => Ok(BuildDeclarationKind::Application),
        _ => Err(Error::InvalidSubject),
    }
}

fn role_text(value: BuildDeclarationKind) -> Result<&'static str, Error> {
    match value {
        BuildDeclarationKind::Package => Ok("package"),
        BuildDeclarationKind::Application => Ok("application"),
        _ => Err(Error::InvalidSubject),
    }
}

fn contract(value: &str) -> Result<ReviewOnlyRootRoleContract, Error> {
    match value {
        "dependency-compatibility" => Ok(ReviewOnlyRootRoleContract::DependencyCompatibility),
        "application-activation" => Ok(ReviewOnlyRootRoleContract::ApplicationActivation),
        _ => Err(Error::InvalidSubject),
    }
}
