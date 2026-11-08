use crate::record::{PackageReviewNominalIdentity, PackageReviewTypeIdentity};

/// The complete selected application, independent of compiler replay receipts.
/// Lifetime ordinals refer to the containing policy's ordered binder telescope.
/// Static argument categories preserve their order within the exact named
/// declaration's telescope, including the independently typed const carriers.
/// This records policy meaning; it grants no conformance or package authority.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyClosedConformanceApplication {
    pub(crate) declaration: PackageReviewNominalIdentity,
    pub(crate) lifetime_arguments: Vec<u32>,
    pub(crate) type_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) const_arguments: Vec<PackagePolicyConformanceConstArgument>,
    pub(crate) machine_arguments: Vec<PackageReviewNominalIdentity>,
    pub(crate) trait_identity: PackageReviewNominalIdentity,
    pub(crate) trait_lifetime_arguments: Vec<u32>,
    pub(crate) trait_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) rows: Vec<PackagePolicyConformanceRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackagePolicyConformanceConstArgument {
    Evaluated {
        parameter_carrier: PackageReviewTypeIdentity,
        declared_carrier: PackageReviewTypeIdentity,
        canonical_value_encoding: String,
    },
    CallerExpression {
        parameter_carrier: PackageReviewTypeIdentity,
        expression: PackageReviewTypeIdentity,
    },
    CallerBinder {
        parameter_carrier: PackageReviewTypeIdentity,
        binder: PackageReviewNominalIdentity,
        binder_carrier: PackageReviewTypeIdentity,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackagePolicyConformanceRow {
    pub(crate) declaring_trait: PackageReviewNominalIdentity,
    pub(crate) requirement: PackageReviewNominalIdentity,
    pub(crate) realization_machine: PackageReviewNominalIdentity,
    pub(crate) realization_state: PackageReviewNominalIdentity,
}

impl PackagePolicyConformanceRow {}
