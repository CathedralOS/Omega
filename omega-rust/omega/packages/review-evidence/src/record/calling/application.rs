//! Complete normalized calling application, independent of realization receipts.

mod validation;

use super::{
    PackagePolicyCallbacks, PackagePolicyCallingOpaqueUse, PackagePolicyPhysicalCallingContract,
};
use crate::record::{
    PackagePolicyTypeParameter, PackageReviewBoundaryShapeGraph, PackageReviewNominalIdentity,
    PackageReviewRepresentationTarget, PackageReviewTypeIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyCallingPlan {
    pub(crate) boundary_trait: PackageReviewNominalIdentity,
    pub(crate) boundary_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) boundary_lifetime_parameter_count: u32,
    pub(crate) boundary_lifetime_arguments: Vec<u32>,
    pub(crate) application_lifetime_parameter_count: u32,
    pub(crate) requirement: PackageReviewNominalIdentity,
    pub(crate) requirement_trait: PackageReviewNominalIdentity,
    pub(crate) requirement_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) requirement_lifetime_arguments: Vec<u32>,
    pub(crate) requirement_lifetime_parameter_count: u32,
    pub(crate) static_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) target: PackageReviewRepresentationTarget,
    pub(crate) shape_graph: PackageReviewBoundaryShapeGraph,
    pub(crate) semantic_parameters: Vec<PackagePolicyCallingParameter>,
    pub(crate) semantic_result: Option<PackageReviewTypeIdentity>,
    pub(crate) native_parameters: Vec<PackagePolicyNativeParameter>,
    pub(crate) callbacks: PackagePolicyCallbacks,
    pub(crate) opaque_uses: Vec<PackagePolicyCallingOpaqueUse>,
    pub(crate) physical: PackagePolicyPhysicalCallingContract,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackagePolicyCallingParameter {
    pub(crate) name: String,
    pub(crate) value_type: PackageReviewTypeIdentity,
    pub(crate) is_mutable: bool,
    pub(crate) is_const: bool,
    pub(crate) shape_root: u16,
}

/// Position in this ordered vector is the authored native ordinal. Names are
/// nominal parameter coordinates; physical shapes do not identify parameters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyNativeParameter {
    pub(crate) name: String,
    pub(crate) origin: PackagePolicyNativeParameterOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackagePolicyNativeParameterOrigin {
    SemanticFormal {
        formal_ordinal: u32,
        shape_root: u16,
    },
    PrivateCallback {
        binder_index: u32,
        byte_size: u16,
        alignment: u16,
    },
}

impl PackagePolicyCallingPlan {
    crate::record::accessors::record_accessors!(static_parameters);
    pub const fn boundary_trait(&self) -> &PackageReviewNominalIdentity {
        &self.boundary_trait
    }
    pub fn boundary_arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.boundary_arguments
    }
    pub fn boundary_lifetime_arguments(&self) -> &[u32] {
        &self.boundary_lifetime_arguments
    }
    pub const fn application_lifetime_parameter_count(&self) -> u32 {
        self.application_lifetime_parameter_count
    }
    pub const fn boundary_lifetime_parameter_count(&self) -> u32 {
        self.boundary_lifetime_parameter_count
    }
    pub const fn requirement(&self) -> &PackageReviewNominalIdentity {
        &self.requirement
    }
    pub const fn requirement_trait(&self) -> &PackageReviewNominalIdentity {
        &self.requirement_trait
    }
    pub fn requirement_arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.requirement_arguments
    }
    pub fn requirement_lifetime_arguments(&self) -> &[u32] {
        &self.requirement_lifetime_arguments
    }
    pub const fn requirement_lifetime_parameter_count(&self) -> u32 {
        self.requirement_lifetime_parameter_count
    }

    pub const fn target(&self) -> &PackageReviewRepresentationTarget {
        &self.target
    }
    pub const fn shape_graph(&self) -> &PackageReviewBoundaryShapeGraph {
        &self.shape_graph
    }
    pub const fn semantic_result(&self) -> Option<&PackageReviewTypeIdentity> {
        self.semantic_result.as_ref()
    }
    pub fn native_parameters(&self) -> &[PackagePolicyNativeParameter] {
        &self.native_parameters
    }
    pub const fn callbacks(&self) -> &PackagePolicyCallbacks {
        &self.callbacks
    }
    pub(crate) fn opaque_uses(&self) -> &[PackagePolicyCallingOpaqueUse] {
        &self.opaque_uses
    }
    pub const fn physical(&self) -> &PackagePolicyPhysicalCallingContract {
        &self.physical
    }
}

impl PackagePolicyCallingParameter {}

impl PackagePolicyNativeParameter {}
