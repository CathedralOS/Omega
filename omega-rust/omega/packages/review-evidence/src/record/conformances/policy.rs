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
    pub(crate) subject: Option<PackageReviewTypeIdentity>,
    pub(crate) trait_identity: PackageReviewNominalIdentity,
    pub(crate) trait_lifetime_arguments: Vec<u32>,
    pub(crate) trait_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) rows: Vec<PackagePolicyConformanceRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyConformanceConstArgument {
    Evaluated {
        parameter_carrier: PackageReviewTypeIdentity,
        declared_carrier: PackageReviewTypeIdentity,
        canonical_value_encoding: String,
    },
    CallerBinder {
        parameter_carrier: PackageReviewTypeIdentity,
        binder: PackageReviewNominalIdentity,
        binder_carrier: PackageReviewTypeIdentity,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyConformanceRow {
    pub(crate) declaring_trait: PackageReviewNominalIdentity,
    pub(crate) requirement: PackageReviewNominalIdentity,
    pub(crate) realization_machine: PackageReviewNominalIdentity,
    pub(crate) realization_state: PackageReviewNominalIdentity,
}

impl PackagePolicyClosedConformanceApplication {
    pub const fn declaration(&self) -> &PackageReviewNominalIdentity {
        &self.declaration
    }
    pub fn lifetime_arguments(&self) -> &[u32] {
        &self.lifetime_arguments
    }
    pub fn type_arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.type_arguments
    }
    pub fn const_arguments(&self) -> &[PackagePolicyConformanceConstArgument] {
        &self.const_arguments
    }
    pub fn machine_arguments(&self) -> &[PackageReviewNominalIdentity] {
        &self.machine_arguments
    }
    pub const fn subject(&self) -> Option<&PackageReviewTypeIdentity> {
        self.subject.as_ref()
    }
    pub const fn trait_identity(&self) -> &PackageReviewNominalIdentity {
        &self.trait_identity
    }
    pub fn trait_lifetime_arguments(&self) -> &[u32] {
        &self.trait_lifetime_arguments
    }
    pub fn trait_arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.trait_arguments
    }
    pub fn rows(&self) -> &[PackagePolicyConformanceRow] {
        &self.rows
    }
}

impl PackagePolicyConformanceRow {
    pub const fn declaring_trait(&self) -> &PackageReviewNominalIdentity {
        &self.declaring_trait
    }
    pub const fn requirement(&self) -> &PackageReviewNominalIdentity {
        &self.requirement
    }
    pub const fn realization_machine(&self) -> &PackageReviewNominalIdentity {
        &self.realization_machine
    }
    pub const fn realization_state(&self) -> &PackageReviewNominalIdentity {
        &self.realization_state
    }
}
