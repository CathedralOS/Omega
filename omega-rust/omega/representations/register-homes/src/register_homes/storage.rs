//! Function-local physical views assigned to virtual registers.

pub mod fixed_precolored_segment_homes;
pub use fixed_precolored_segment_homes::*;

use register_model::{RegisterClassId, RegisterViewId};
use selected_instructions::VirtualRegisterId;
use semantic_vocabulary::MachineId;

use crate::{AllocationLegalityIdentity, AllocatorAvailabilityIdentity};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::LiveRangeIdentity;

/// Bounded physical homes. This table is data, not allocation admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterHomePlan {
    pub legality: AllocationLegalityIdentity,
    pub ranges: LiveRangeIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub functions: Vec<FunctionRegisterHomes>,
    pub structural_unit_functions: Vec<FunctionRegisterHomes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRegisterHomes {
    pub machine: MachineId,
    pub assignments: Vec<VirtualRegisterHome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualRegisterHome {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
}
