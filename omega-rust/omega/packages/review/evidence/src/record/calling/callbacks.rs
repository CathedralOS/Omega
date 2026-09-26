//! Receipt-free callback relationships within one exact calling application.

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
pub struct PackagePolicyCallbackBinder {
    pub(crate) parameter: PackageReviewNominalIdentity,
    pub(crate) static_parameter_ordinal: u32,
    pub(crate) static_machine_ordinal: u32,
    pub(crate) requirement: PackageReviewNominalIdentity,
}

/// Native positions are relative to the containing policy's authored native
/// telescope. Field destinations reference this component's canonical layout
/// catalog; the catalog retains the complete named path, never compact IDs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyCallbackDestination {
    Parameter {
        native_ordinal: u32,
    },
    Field {
        native_ordinal: u32,
        layout_index: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyCallbackDemand {
    pub(crate) destination: PackagePolicyCallbackDestination,
    pub(crate) requirement: PackageReviewNominalIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyCallbackMaterialization {
    pub(crate) binder_index: u32,
    pub(crate) destination: PackagePolicyCallbackDestination,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyCallbackLayout {
    pub(crate) formal_ordinal: u32,
    pub(crate) native_ordinal: u32,
    pub(crate) root_layout: PackagePolicyCallbackLayoutApplication,
    pub(crate) inline_field: Option<PackagePolicyCallbackInlineField>,
    pub(crate) terminal_slot: PackagePolicyClosedConformanceApplication,
    pub(crate) terminal_offset: u64,
    pub(crate) terminal_byte_size: u64,
    pub(crate) terminal_alignment: u64,
    pub(crate) composed_offset: u64,
}

/// Exact supported layout application `Policy<Schema>` and target geometry.
/// The policy declaration is authored selection; its evaluator machine and
/// generated concrete data name are implementation details, not this identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyCallbackLayoutApplication {
    pub(crate) policy: PackageReviewNominalIdentity,
    pub(crate) schema: PackageReviewTypeIdentity,
    pub(crate) byte_size: u64,
    pub(crate) alignment: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyCallbackInlineField {
    pub(crate) field: PackageReviewNominalIdentity,
    pub(crate) offset: u64,
    pub(crate) extent: u64,
    pub(crate) alignment: u64,
    pub(crate) child_layout: PackagePolicyCallbackLayoutApplication,
}

impl PackagePolicyCallbacks {
    pub fn binders(&self) -> &[PackagePolicyCallbackBinder] {
        &self.binders
    }
    pub fn demands(&self) -> &[PackagePolicyCallbackDemand] {
        &self.demands
    }
    pub fn materializations(&self) -> &[PackagePolicyCallbackMaterialization] {
        &self.materializations
    }
    pub fn layouts(&self) -> &[PackagePolicyCallbackLayout] {
        &self.layouts
    }
}
impl PackagePolicyCallbackBinder {
    pub const fn parameter(&self) -> &PackageReviewNominalIdentity {
        &self.parameter
    }
    pub const fn static_parameter_ordinal(&self) -> u32 {
        self.static_parameter_ordinal
    }
    pub const fn static_machine_ordinal(&self) -> u32 {
        self.static_machine_ordinal
    }
    pub const fn requirement(&self) -> &PackageReviewNominalIdentity {
        &self.requirement
    }
}
impl PackagePolicyCallbackDemand {
    pub const fn destination(&self) -> &PackagePolicyCallbackDestination {
        &self.destination
    }
    pub const fn requirement(&self) -> &PackageReviewNominalIdentity {
        &self.requirement
    }
}
impl PackagePolicyCallbackMaterialization {
    pub const fn binder_index(&self) -> u32 {
        self.binder_index
    }
    pub const fn destination(&self) -> &PackagePolicyCallbackDestination {
        &self.destination
    }
}
impl PackagePolicyCallbackLayout {
    pub const fn formal_ordinal(&self) -> u32 {
        self.formal_ordinal
    }
    pub const fn native_ordinal(&self) -> u32 {
        self.native_ordinal
    }
    pub const fn root_layout(&self) -> &PackagePolicyCallbackLayoutApplication {
        &self.root_layout
    }
    pub const fn inline_field(&self) -> Option<&PackagePolicyCallbackInlineField> {
        self.inline_field.as_ref()
    }
    pub const fn terminal_slot(&self) -> &PackagePolicyClosedConformanceApplication {
        &self.terminal_slot
    }
    pub const fn terminal_offset(&self) -> u64 {
        self.terminal_offset
    }
    pub const fn terminal_byte_size(&self) -> u64 {
        self.terminal_byte_size
    }
    pub const fn terminal_alignment(&self) -> u64 {
        self.terminal_alignment
    }
    pub const fn composed_offset(&self) -> u64 {
        self.composed_offset
    }
}
impl PackagePolicyCallbackLayoutApplication {
    pub const fn policy(&self) -> &PackageReviewNominalIdentity {
        &self.policy
    }
    pub const fn schema(&self) -> &PackageReviewTypeIdentity {
        &self.schema
    }
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }
    pub const fn alignment(&self) -> u64 {
        self.alignment
    }
}
impl PackagePolicyCallbackInlineField {
    pub const fn field(&self) -> &PackageReviewNominalIdentity {
        &self.field
    }
    pub const fn offset(&self) -> u64 {
        self.offset
    }
    pub const fn extent(&self) -> u64 {
        self.extent
    }
    pub const fn alignment(&self) -> u64 {
        self.alignment
    }
    pub const fn child_layout(&self) -> &PackagePolicyCallbackLayoutApplication {
        &self.child_layout
    }
}
