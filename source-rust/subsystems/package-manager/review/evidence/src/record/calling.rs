// Receipt-free components of the published boundary calling contract.
//
// The physical component is not a complete boundary policy on its own: exact
// requirement, target, native telescope, and named callback associations belong
// beside it.

mod application;

use crate::record::accessors::field_projection_accessors;
use crate::record::{
    PackagePolicyClosedConformanceApplication, PackageReviewBoundaryCallingPolicy,
    PackageReviewBoundaryValuePlacement, PackageReviewMachineRegister,
    PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
    PackageReviewOpaqueRepresentationOccurrence, PackageReviewTypeIdentity,
};
pub use application::{
    PackagePolicyCallingParameter, PackagePolicyCallingPlan, PackagePolicyNativeParameter,
    PackagePolicyNativeParameterOrigin,
};

// Receipt-free callback relationships within one exact calling application.
//
// Native positions in `PackagePolicyCallbackDestination` are relative to the
// containing policy's authored native telescope, and field destinations index
// into the canonical layout catalog retaining the complete named path.
// `PackagePolicyCallbackLayoutApplication` records the authored layout policy
// and target geometry (`Policy<Schema>`), not its evaluator machine or
// generated concrete data name.

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

// Selected opaque meanings actually used by one exact calling signature.
//
// The containing calling policy owns the complete shape graph, and occurrence
// roots refer to that graph rather than to compiler handles or equal-looking
// shapes. Unused selections and producer availability are not calling-use rows.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyCallingOpaqueUse {
    pub(crate) opaque: PackageReviewNominalIdentity,
    pub(crate) carrier: PackageReviewNominalIdentity,
    pub(crate) selection_owner: PackageReviewNominalOwner,
    pub(crate) application: PackagePolicyClosedConformanceApplication,
    pub(crate) origin: PackageReviewOpaqueRepresentationApplicationOrigin,
    pub(crate) lifecycle: PackageReviewOpaqueRepresentationLifecycleDisposition,
    pub(crate) copy_disposition: PackageReviewOpaqueRepresentationCopyDisposition,
    pub(crate) occurrences: Vec<PackageReviewOpaqueRepresentationOccurrence>,
}

// Physical placement and interrupted-state promises (`PackagePolicyPhysicalCallingContract`).
//
// This component contains no callback IDs, selected entries, or evidence
// receipts; `parameters` are in native ABI order (including private
// whole-parameter callbacks) and `ordinary_clobbers` are in deduplicated
// canonical register order. `PackagePolicyMachineStateSet` stores a sorted,
// deduplicated set of named state classes rather than compiler bit assignments.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyPhysicalCallingContract {
    pub policy: PackageReviewBoundaryCallingPolicy,
    pub parameters: Vec<PackageReviewBoundaryValuePlacement>,
    pub result: Option<PackageReviewBoundaryValuePlacement>,
    pub ordinary_clobbers: Vec<PackageReviewMachineRegister>,
    pub stack_alignment: u16,
    pub shadow_bytes: u16,
    pub entry_control: PackagePolicyEntryControl,
    pub state: PackagePolicyStatePlan,
}

impl PackagePolicyPhysicalCallingContract {
    field_projection_accessors!(copied policy: PackageReviewBoundaryCallingPolicy);

    pub fn parameters(&self) -> &[PackageReviewBoundaryValuePlacement] {
        &self.parameters
    }

    field_projection_accessors!(optional result: Option<&PackageReviewBoundaryValuePlacement>);

    pub fn ordinary_clobbers(&self) -> &[PackageReviewMachineRegister] {
        &self.ordinary_clobbers
    }

    field_projection_accessors!(copied stack_alignment: u16, shadow_bytes: u16);
    field_projection_accessors!(copied entry_control: PackagePolicyEntryControl);
    field_projection_accessors!(borrowed state: &PackagePolicyStatePlan);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyEntryControl {
    CallReturn,
    SupervisorCall {
        number_register: PackageReviewMachineRegister,
        immediate: u16,
    },
    InterruptReturn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyMachineRegime {
    X86Long64,
    Aarch64A64 { exception_level: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyMachineState {
    GeneralRegisters,
    VectorRegisters,
    Flags,
    InstructionPointer,
    StackPointer,
    SegmentState,
    ControlState,
    DebugState,
    ExtendedState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyMachineStateSet(Vec<PackagePolicyMachineState>);

impl PackagePolicyMachineStateSet {
    pub fn new(states: impl IntoIterator<Item = PackagePolicyMachineState>) -> Self {
        let mut states: Vec<_> = states.into_iter().collect();
        states.sort_unstable();
        states.dedup();
        Self(states)
    }

    pub fn as_slice(&self) -> &[PackagePolicyMachineState] {
        &self.0
    }

    pub(crate) fn from_canonical(states: Vec<PackagePolicyMachineState>) -> Option<Self> {
        states
            .windows(2)
            .all(|pair| pair[0] < pair[1])
            .then_some(Self(states))
    }
}

pub type PackagePolicyEntryStack = calling_conventions::EntryStack;

pub type PackagePolicyPreemption = calling_conventions::Preemption;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyStatePlan {
    pub initial_regime: PackagePolicyMachineRegime,
    pub interrupted_state: PackagePolicyMachineStateSet,
    pub saved_state: PackagePolicyMachineStateSet,
    pub restored_state: PackagePolicyMachineStateSet,
    pub permitted_transitive_use: PackagePolicyMachineStateSet,
    pub stack: PackagePolicyEntryStack,
    pub preemption: PackagePolicyPreemption,
}

impl PackagePolicyStatePlan {
    field_projection_accessors!(copied initial_regime: PackagePolicyMachineRegime);
    field_projection_accessors!(borrowed interrupted_state: &PackagePolicyMachineStateSet);
    field_projection_accessors!(borrowed saved_state: &PackagePolicyMachineStateSet);
    field_projection_accessors!(borrowed restored_state: &PackagePolicyMachineStateSet);
    field_projection_accessors!(borrowed permitted_transitive_use: &PackagePolicyMachineStateSet);
    field_projection_accessors!(copied stack: PackagePolicyEntryStack);
    field_projection_accessors!(copied preemption: PackagePolicyPreemption);
}
