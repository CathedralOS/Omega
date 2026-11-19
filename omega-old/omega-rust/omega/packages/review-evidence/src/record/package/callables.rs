// Checked implementation body service-reach state (`PackageReviewCheckedServiceReach`).

use super::super::identity::PackageReviewNominalIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewCheckedServiceReach {
    NoCheckedBody,
    CheckedBody {
        realized: Vec<PackageReviewNominalIdentity>,
        concrete: Vec<PackageReviewNominalIdentity>,
    },
}

impl PackageReviewCheckedServiceReach {
    pub fn realized(&self) -> Option<&[PackageReviewNominalIdentity]> {
        match self {
            Self::NoCheckedBody => None,
            Self::CheckedBody { realized, .. } => Some(realized),
        }
    }

    pub fn concrete(&self) -> Option<&[PackageReviewNominalIdentity]> {
        match self {
            Self::NoCheckedBody => None,
            Self::CheckedBody { concrete, .. } => Some(concrete),
        }
    }
}
