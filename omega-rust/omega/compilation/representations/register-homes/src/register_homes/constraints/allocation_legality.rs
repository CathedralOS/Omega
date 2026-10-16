//! Durable allocation legality data; not validation or allocation authority.

mod identity;
pub use identity::{
    allocation_legality_function_digest, allocation_legality_identity_from_digests,
};

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
    /// One row per selected function, in plan order. Rows are shared between
    /// the successive plans runtime-spill recovery analyzes: a step that
    /// rewrites one function recomputes that function's row and shares every
    /// other row instead of copying it.
    pub functions: Vec<std::sync::Arc<FunctionAllocationLegality>>,
}

impl AllocationLegalityPlan {
    /// Every function's register legality rows, in function order.
    pub fn virtual_registers(
        &self,
    ) -> impl Iterator<Item = &VirtualRegisterAllocationLegality> + '_ {
        self.functions
            .iter()
            .flat_map(|function| &function.virtual_registers)
    }
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
    pub spans: Vec<VirtualLegalitySpan>,
    pub early_clobber_points: Vec<VirtualEarlyClobberPointLegality>,
    pub entry_transitions: Vec<EntryFixedViewTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualLegalitySpan {
    pub block: SelectedBlockId,
    pub start: LiveRangePoint,
    pub end: LiveRangePoint,
    /// Canonical view-ID-sorted candidates legal throughout [start, end).
    /// Spans partition each source fragment without crossing its boundary.
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
