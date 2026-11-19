// Physical placement and interrupted-state promises (`PackagePolicyPhysicalCallingContract`).
//
// This component contains no callback IDs, selected entries, or evidence
// receipts; `parameters` are in native ABI order (including private
// whole-parameter callbacks) and `ordinary_clobbers` are in deduplicated
// canonical register order. `PackagePolicyMachineStateSet` stores a sorted,
// deduplicated set of named state classes rather than compiler bit assignments.

use crate::record::accessors::field_projection_accessors;
use crate::record::{
    PackageReviewBoundaryCallingPolicy, PackageReviewBoundaryValuePlacement,
    PackageReviewMachineRegister,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyPhysicalCallingContract {
    pub(crate) policy: PackageReviewBoundaryCallingPolicy,
    pub(crate) parameters: Vec<PackageReviewBoundaryValuePlacement>,
    pub(crate) result: Option<PackageReviewBoundaryValuePlacement>,
    pub(crate) ordinary_clobbers: Vec<PackageReviewMachineRegister>,
    pub(crate) stack_alignment: u16,
    pub(crate) shadow_bytes: u16,
    pub(crate) entry_control: PackagePolicyEntryControl,
    pub(crate) state: PackagePolicyStatePlan,
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
    pub(crate) initial_regime: PackagePolicyMachineRegime,
    pub(crate) interrupted_state: PackagePolicyMachineStateSet,
    pub(crate) saved_state: PackagePolicyMachineStateSet,
    pub(crate) restored_state: PackagePolicyMachineStateSet,
    pub(crate) permitted_transitive_use: PackagePolicyMachineStateSet,
    pub(crate) stack: PackagePolicyEntryStack,
    pub(crate) preemption: PackagePolicyPreemption,
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
