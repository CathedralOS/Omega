use crate::record::{
    PackageReviewBoundaryCallingPolicy, PackageReviewBoundaryValuePlacement,
    PackageReviewMachineRegister,
};

/// Physical placement and interrupted-state promises only. This component
/// deliberately contains no callback IDs, selected entries, evidence receipts,
/// or claims to be the complete requirement-pinned boundary policy.
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
    pub const fn policy(&self) -> PackageReviewBoundaryCallingPolicy {
        self.policy
    }

    /// Native ABI order, including any private whole-parameter callbacks.
    /// Their semantic origins are not inferred from these physical placements.
    pub fn parameters(&self) -> &[PackageReviewBoundaryValuePlacement] {
        &self.parameters
    }

    pub const fn result(&self) -> Option<&PackageReviewBoundaryValuePlacement> {
        self.result.as_ref()
    }

    /// Canonical register order, with no duplicates.
    pub fn ordinary_clobbers(&self) -> &[PackageReviewMachineRegister] {
        &self.ordinary_clobbers
    }

    pub const fn stack_alignment(&self) -> u16 {
        self.stack_alignment
    }

    pub const fn shadow_bytes(&self) -> u16 {
        self.shadow_bytes
    }

    pub const fn entry_control(&self) -> PackagePolicyEntryControl {
        self.entry_control
    }

    pub const fn state(&self) -> &PackagePolicyStatePlan {
        &self.state
    }
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

/// A canonical set of named state classes, not compiler bit assignments.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyEntryStack {
    Interrupted,
    Dedicated { class: u16 },
    ProviderSelected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyPreemption {
    NotApplicable,
    Masked,
    Nestable { maximum_depth: u16 },
    ProviderDefined,
}

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
    pub const fn initial_regime(&self) -> PackagePolicyMachineRegime {
        self.initial_regime
    }

    pub const fn interrupted_state(&self) -> &PackagePolicyMachineStateSet {
        &self.interrupted_state
    }

    pub const fn saved_state(&self) -> &PackagePolicyMachineStateSet {
        &self.saved_state
    }

    pub const fn restored_state(&self) -> &PackagePolicyMachineStateSet {
        &self.restored_state
    }

    pub const fn permitted_transitive_use(&self) -> &PackagePolicyMachineStateSet {
        &self.permitted_transitive_use
    }

    pub const fn stack(&self) -> PackagePolicyEntryStack {
        self.stack
    }

    pub const fn preemption(&self) -> PackagePolicyPreemption {
        self.preemption
    }
}
