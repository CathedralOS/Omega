// Complete normalized calling application (`PackagePolicyCallingPlan`),
// independent of realization receipts.
//
// `PackagePolicyNativeParameter` positions in `native_parameters` define the
// authored native ordinals; parameter names are nominal parameter coordinates
// rather than physical shape identities.

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
    pub abi: semantic_vocabulary::CallingAbiIdentity,
    pub boundary_trait: PackageReviewNominalIdentity,
    pub boundary_arguments: Vec<PackageReviewTypeIdentity>,
    pub boundary_lifetime_parameter_count: u32,
    pub boundary_lifetime_arguments: Vec<u32>,
    pub application_lifetime_parameter_count: u32,
    pub requirement: PackageReviewNominalIdentity,
    pub requirement_trait: PackageReviewNominalIdentity,
    pub requirement_arguments: Vec<PackageReviewTypeIdentity>,
    pub requirement_lifetime_arguments: Vec<u32>,
    pub requirement_lifetime_parameter_count: u32,
    pub static_parameters: Vec<PackagePolicyTypeParameter>,
    pub target: PackageReviewRepresentationTarget,
    pub shape_graph: PackageReviewBoundaryShapeGraph,
    pub semantic_parameters: Vec<PackagePolicyCallingParameter>,
    pub semantic_result: Option<PackageReviewTypeIdentity>,
    /// Source formal retained for addressing the table and omitted from the foreign argument list.
    pub dispatch_only_formal: Option<u32>,
    pub native_parameters: Vec<PackagePolicyNativeParameter>,
    pub callbacks: PackagePolicyCallbacks,
    pub opaque_uses: Vec<PackagePolicyCallingOpaqueUse>,
    pub physical: PackagePolicyPhysicalCallingContract,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyCallingParameter {
    pub name: String,
    pub value_type: PackageReviewTypeIdentity,
    pub is_mutable: bool,
    pub is_const: bool,
    pub shape_root: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyNativeParameter {
    pub name: String,
    pub origin: PackagePolicyNativeParameterOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyNativeParameterOrigin {
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
    field_projection_accessors!(borrowed abi: &semantic_vocabulary::CallingAbiIdentity);
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
    field_projection_accessors!(borrowed physical: &PackagePolicyPhysicalCallingContract);
}

impl PackagePolicyCallingParameter {}

impl PackagePolicyNativeParameter {}
