//! Complete normalized calling application, independent of realization receipts.

mod validation;

use super::{
    PackagePolicyCallbacks, PackagePolicyCallingOpaqueUse, PackagePolicyPhysicalCallingContract,
};
use crate::record::accessors::field_projection_accessors;
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
    field_projection_accessors!(sliced static_parameters: &[PackagePolicyTypeParameter]);
    field_projection_accessors!(borrowed boundary_trait: &PackageReviewNominalIdentity);
    field_projection_accessors!(sliced boundary_arguments: &[PackageReviewTypeIdentity]);
    field_projection_accessors!(sliced boundary_lifetime_arguments: &[u32]);
    field_projection_accessors!(copied application_lifetime_parameter_count: u32);
    field_projection_accessors!(copied boundary_lifetime_parameter_count: u32);
    field_projection_accessors!(borrowed requirement: &PackageReviewNominalIdentity);
    field_projection_accessors!(borrowed requirement_trait: &PackageReviewNominalIdentity);
    field_projection_accessors!(sliced requirement_arguments: &[PackageReviewTypeIdentity]);
    field_projection_accessors!(sliced requirement_lifetime_arguments: &[u32]);
    field_projection_accessors!(copied requirement_lifetime_parameter_count: u32);
    field_projection_accessors!(borrowed target: &PackageReviewRepresentationTarget);
    field_projection_accessors!(borrowed shape_graph: &PackageReviewBoundaryShapeGraph);
    field_projection_accessors!(optional semantic_result: Option<&PackageReviewTypeIdentity>);
    field_projection_accessors!(sliced native_parameters: &[PackagePolicyNativeParameter]);
    field_projection_accessors!(borrowed callbacks: &PackagePolicyCallbacks);
    pub(crate) fn opaque_uses(&self) -> &[PackagePolicyCallingOpaqueUse] {
        &self.opaque_uses
    }
    field_projection_accessors!(borrowed physical: &PackagePolicyPhysicalCallingContract);
}

impl PackagePolicyCallingParameter {}

impl PackagePolicyNativeParameter {}
