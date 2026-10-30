//! Borrowed declaration evidence for whole-provider policy review.
//!
//! This is invocation-owned eligibility, not a portable executable catalog.
//! No source handles or source-query callbacks cross this boundary. Names are
//! checked export coordinates, including the exact nominal numeric declaration.
//! Missing application evidence is data until policy actually requires it;
//! an unused declaration or generic template needs no executable body.

use crate::CheckedBodyCandidate;
use semantic_vocabulary::PackageKeyIdentity;

#[derive(Clone, Copy)]
pub struct BoundaryRequirementReview<'facts> {
    pub schema_identity: &'facts str,
    pub package_identity: Option<PackageKeyIdentity>,
    pub numeric_requirement: Result<&'facts str, &'static str>,
}

pub struct BoundaryOperatorDeclaration<'facts> {
    pub requirement: BoundaryRequirementReview<'facts>,
    pub bodies: Vec<CheckedBodyCandidate<'facts, ()>>,
}

pub struct BoundaryOperatorUse<'facts> {
    pub requirement: BoundaryRequirementReview<'facts>,
    pub application: BoundaryOperatorApplication<'facts>,
}

pub enum BoundaryOperatorApplication<'facts> {
    GenericTemplate,
    Closed(Result<Vec<CheckedBodyCandidate<'facts, ()>>, BoundaryApplicationEvidenceError>),
}

#[derive(Clone, Copy)]
pub enum BoundaryApplicationEvidenceError {
    ApplicationCount(usize),
    MissingBodySpan,
    MissingDeclaration,
    DifferentRequirement,
}

impl std::fmt::Display for BoundaryApplicationEvidenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApplicationCount(count) => write!(
                formatter,
                "selected checked boundary-operator use retains {count} exact application demands; expected one"
            ),
            Self::MissingBodySpan => {
                formatter.write_str("operator application lost its checked body span")
            }
            Self::MissingDeclaration => {
                formatter.write_str("checked operator body lost its declaration")
            }
            Self::DifferentRequirement => {
                formatter.write_str("checked operator body names another requirement")
            }
        }
    }
}

pub struct BoundaryRequirementUse<'facts> {
    pub requirement: BoundaryRequirementReview<'facts>,
    pub diagnostic_name: &'facts str,
}

/// One ordered preparation for admission. Only small borrowed coordinates are
/// collected; declaration strings and checked bodies remain with their owner.
pub struct BoundaryProviderReview<'facts> {
    pub declarations: Vec<BoundaryOperatorDeclaration<'facts>>,
    /// Spelled uses precede named uses, preserving diagnostic precedence.
    pub operator_uses: Vec<BoundaryOperatorUse<'facts>>,
    pub requirement_uses: Vec<BoundaryRequirementUse<'facts>>,
}
