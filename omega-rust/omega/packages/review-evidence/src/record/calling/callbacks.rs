// Receipt-free callback relationships within one exact calling application.
//
// Native positions in `PackagePolicyCallbackDestination` are relative to the
// containing policy's authored native telescope, and field destinations index
// into the canonical layout catalog retaining the complete named path.
// `PackagePolicyCallbackLayoutApplication` records the authored layout policy
// and target geometry (`Policy<Schema>`), not its evaluator machine or
// generated concrete data name.

use crate::record::{
    PackagePolicyClosedConformanceApplication, PackageReviewNominalIdentity,
    PackageReviewTypeIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyCallbacks {
    pub(crate) binders: Vec<PackagePolicyCallbackBinder>,
    pub(crate) demands: Vec<PackagePolicyCallbackDemand>,
    pub(crate) materializations: Vec<PackagePolicyCallbackMaterialization>,
    pub(crate) layouts: Vec<PackagePolicyCallbackLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackagePolicyCallbackBinder {
    pub(crate) parameter: PackageReviewNominalIdentity,
    pub(crate) static_parameter_ordinal: u32,
    pub(crate) static_machine_ordinal: u32,
    pub(crate) requirement: PackageReviewNominalIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackagePolicyCallbackDestination {
    Parameter {
        native_ordinal: u32,
    },
    Field {
        native_ordinal: u32,
        layout_index: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackagePolicyCallbackDemand {
    pub(crate) destination: PackagePolicyCallbackDestination,
    pub(crate) requirement: PackageReviewNominalIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackagePolicyCallbackMaterialization {
    pub(crate) binder_index: u32,
    pub(crate) destination: PackagePolicyCallbackDestination,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackagePolicyCallbackLayout {
    pub(crate) formal_ordinal: u32,
    pub(crate) native_ordinal: u32,
    pub(crate) root_layout: PackagePolicyCallbackLayoutApplication,
    pub(crate) inline_fields: Vec<PackagePolicyCallbackInlineField>,
    pub(crate) terminal_slot: PackagePolicyClosedConformanceApplication,
    pub(crate) terminal_placements: Vec<layout_plans::LayoutPlacementReport>,
    pub(crate) terminal_byte_size: u64,
    pub(crate) terminal_alignment: u64,
    pub(crate) composed_placements: Vec<layout_plans::LayoutPlacementReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackagePolicyCallbackLayoutApplication {
    pub(crate) policy: Option<PackageReviewNominalIdentity>,
    pub(crate) schema: PackageReviewTypeIdentity,
    pub(crate) byte_size: u64,
    pub(crate) alignment: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackagePolicyCallbackInlineField {
    pub(crate) field: PackageReviewNominalIdentity,
    pub(crate) offset: u64,
    pub(crate) extent: u64,
    pub(crate) alignment: u64,
    pub(crate) child_layout: PackagePolicyCallbackLayoutApplication,
}

impl PackagePolicyCallbacks {}
impl PackagePolicyCallbackBinder {}
impl PackagePolicyCallbackDemand {}
impl PackagePolicyCallbackMaterialization {}
impl PackagePolicyCallbackLayoutApplication {}
impl PackagePolicyCallbackInlineField {}
