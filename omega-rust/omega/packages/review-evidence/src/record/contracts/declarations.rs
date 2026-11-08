// Public constant and operator declaration shapes (`PackageReviewConstShape`
// and `PackageReviewOperatorShape`).
//
// `PackageReviewConstMeaning` distinguishes a completed evaluated value from an
// admitted target-dependent contract expression so a folded scalar never
// replaces its target dependency.

use crate::record::PackageReviewCallableContract;
use crate::record::PackageReviewCrashRoute;
use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewOperatorCoordinate;
use crate::record::PackageReviewSignatureParameter;
use crate::record::PackageReviewTypeIdentity;
use crate::record::PackageReviewTypeParameter;
use crate::record::accessors::field_projection_accessors;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewConstShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) declared_type: PackageReviewTypeIdentity,
    pub(crate) meaning: PackageReviewConstMeaning,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackageReviewConstMeaning {
    Evaluated(String),
    TargetDependent(super::expressions::PackageReviewContractExpression),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageReviewOperatorShape {
    pub(crate) coordinate: PackageReviewOperatorCoordinate,
    pub(crate) is_boundary: bool,
    pub(crate) spelling: Option<language_core::OperatorSpelling>,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) parameters: Vec<PackageReviewSignatureParameter>,
    pub(crate) return_type: PackageReviewTypeIdentity,
    pub(crate) contracts: Vec<PackageReviewCallableContract>,
    pub(crate) published_crash: Vec<PackageReviewCrashRoute>,
}

impl PackageReviewConstShape {
    field_projection_accessors!(borrowed identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(borrowed declared_type: &PackageReviewTypeIdentity);
}
