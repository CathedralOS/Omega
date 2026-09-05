use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use psi_core::MachineId;

use crate::{
    AllocatorAvailabilityIdentity, LiveRangeIdentity, LiveRangePoint, VirtualFixedConstraintSite,
};

pub use omega_register_homes::AllocationLegalityIdentity;

/// Exact physical-view legality before allocation. This is analysis output,
/// not a home assignment: incompatible fixed views remain explicit transition
/// requirements rather than being silently reconciled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationLegalityPlan {
    pub ranges: LiveRangeIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub functions: Vec<FunctionAllocationLegality>,
    pub structural_unit_functions: Vec<FunctionAllocationLegality>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAllocationLegality {
    pub machine: MachineId,
    pub virtual_registers: Vec<VirtualRegisterAllocationLegality>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualRegisterAllocationLegality {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub points: Vec<VirtualPointLegality>,
    pub early_clobber_points: Vec<VirtualEarlyClobberPointLegality>,
    pub entry_transitions: Vec<EntryFixedViewTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualPointLegality {
    pub block: SelectedBlockId,
    pub point: LiveRangePoint,
    /// Canonical view-ID-sorted candidates legal at this exact phase.
    pub candidates: Vec<RegisterViewId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualEarlyClobberPointLegality {
    pub block: SelectedBlockId,
    pub position: crate::LivenessPosition,
    pub instruction: SelectedInstructionId,
    pub operand: u16,
    pub point: LiveRangePoint,
    /// Canonical candidates whose write footprint is legal at the early phase.
    pub candidates: Vec<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryFixedViewTransition {
    pub from_view: RegisterViewId,
    pub to_site: VirtualFixedConstraintSite,
    pub to_view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocationLegalityValidationReceipt {
    pub(crate) identity: AllocationLegalityIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) function_count: usize,
    pub(crate) structural_unit_function_count: usize,
    pub(crate) virtual_register_count: usize,
    pub(crate) point_count: usize,
    pub(crate) candidate_count: usize,
    pub(crate) early_clobber_point_count: usize,
    pub(crate) early_clobber_candidate_count: usize,
    pub(crate) entry_transition_count: usize,
}

impl AllocationLegalityValidationReceipt {
    pub const fn identity(self) -> AllocationLegalityIdentity {
        self.identity
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
    }
    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }
    pub const fn point_count(self) -> usize {
        self.point_count
    }
    pub const fn candidate_count(self) -> usize {
        self.candidate_count
    }
    pub const fn early_clobber_point_count(self) -> usize {
        self.early_clobber_point_count
    }
    pub const fn early_clobber_candidate_count(self) -> usize {
        self.early_clobber_candidate_count
    }
    pub const fn entry_transition_count(self) -> usize {
        self.entry_transition_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAllocationLegality {
    pub(crate) plan: AllocationLegalityPlan,
    pub(crate) receipt: AllocationLegalityValidationReceipt,
}

impl ValidatedAllocationLegality {
    pub const fn plan(&self) -> &AllocationLegalityPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> AllocationLegalityValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationLegalityError {
    RootMismatch,
    UnknownClass {
        function: usize,
        register: u32,
        class: u16,
    },
    UnknownFixedView {
        function: usize,
        register: u32,
        view: u16,
    },
    IllegalFixedView {
        function: usize,
        register: u32,
        view: u16,
    },
    NoCandidateViews {
        function: usize,
        register: u32,
        block: u32,
        point: u32,
    },
    PointOverflow {
        function: usize,
    },
    FunctionMismatch {
        function: usize,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    NonCanonicalRows {
        function: usize,
        register: u32,
    },
}

impl std::fmt::Display for AllocationLegalityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal allocation-legality derivation failed: {self:?}"
        )
    }
}

impl std::error::Error for AllocationLegalityError {}
