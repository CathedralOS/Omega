//! Effects of the current selected program, bound to its exact inputs.
//!
//! This persisted representation contains no optimizer-route or stage ancestry.
//! Decoding checks framing and content identity; pipeline analysis independently
//! checks the rows against the selected instructions before admitting their use.

pub mod encoding;
pub mod identity;
mod instructions;

pub use encoding::PreAllocationMachineEffectDecodeError;
pub use identity::pre_allocation_machine_effect_identity;
pub use instructions::*;

use crate::{MachineEffectCatalogIdentity, SelectedInstructionPlanIdentity};
use omega_optimization_core::OptimizationUnitIdentity;
use omega_register_model::{RegisterConstraintCatalogIdentity, TargetRegisterEnvironmentIdentity};
use omega_target::NativeTarget;
use psi_core::FuelScheduleIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreAllocationMachineEffectIdentity([u8; 32]);

impl PreAllocationMachineEffectIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreAllocationMachineEffectPlan {
    pub identity: PreAllocationMachineEffectIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub register_constraints: RegisterConstraintCatalogIdentity,
    pub machine_effect_catalog: MachineEffectCatalogIdentity,
    pub functions: Vec<FunctionMachineEffects>,
    pub structural_unit_functions: Vec<StructuralUnitFunctionMachineEffects>,
}

impl PreAllocationMachineEffectPlan {
    pub fn encode(&self) -> Vec<u8> {
        encoding::encode_terminal_pre_allocation_machine_effect_plan(self)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, crate::PreAllocationMachineEffectDecodeError> {
        encoding::decode_terminal_pre_allocation_machine_effect_plan(encoded)
    }
}

#[cfg(test)]
mod tests;
