//! Durable allocation legality data; not validation or allocation authority.

mod identity;
pub use identity::allocation_legality_identity;

use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use semantic_vocabulary::MachineId;

use crate::AllocatorAvailabilityIdentity;
use selected_instructions::{LiveRangeIdentity, LiveRangePoint, VirtualFixedConstraintSite};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocationLegalityIdentity(pub(crate) [u8; 32]);

impl AllocationLegalityIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

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
    pub position: selected_instructions::LivenessPosition,
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
