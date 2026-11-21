// Stable public constant declaration shapes (`PackageReviewConstShape`).
//
// `PackageReviewConstMeaning` distinguishes a completed evaluated value from an
// admitted target-dependent contract expression so a folded scalar never
// replaces its target dependency.

use crate::record::PackageReviewNominalIdentity;

use crate::record::PackageReviewTypeIdentity;

use crate::record::accessors::field_projection_accessors;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewConstShape {
    pub identity: PackageReviewNominalIdentity,
    pub declared_type: PackageReviewTypeIdentity,
    pub meaning: PackageReviewConstMeaning,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewConstMeaning {
    Evaluated(String),
    TargetDependent(super::expressions::PackageReviewContractExpression),
}

impl PackageReviewConstShape {
    field_projection_accessors!(borrowed identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(borrowed declared_type: &PackageReviewTypeIdentity);
}
