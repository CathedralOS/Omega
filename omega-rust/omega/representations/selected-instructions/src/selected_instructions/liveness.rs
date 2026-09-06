//! Durable selected-instruction liveness facts and their canonical identity.
//!
//! Raw data carries no analysis admission authority; independent replay belongs
//! to the selected-instruction transformation stage.

mod identity;
pub use identity::liveness_identity;

use crate::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use optimization_core::OptimizationUnitIdentity;
use register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId};
use semantic_vocabulary::{BlockId, EdgeId, FuelScheduleIdentity, MachineId};
use target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LivenessPosition(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LivenessIdentity(pub(crate) [u8; 32]);

impl LivenessIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LivenessPlan {
    pub selected: SelectedInstructionPlanIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub functions: Vec<FunctionLiveness>,
    /// Structural-ABI Unit functions retain their own roster even though the
    /// current exact form has no allocator-managed virtual registers. Keeping
    /// this separate prevents a zero-VReg result from erasing function, call,
    /// return, or architectural-unit custody.
    pub structural_unit_functions: Vec<FunctionLiveness>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionLiveness {
    pub machine: MachineId,
    pub entry_definitions: Vec<EntryDefinition>,
    pub operand_positions: Vec<OperandPosition>,
    pub blocks: Vec<BlockLiveness>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryDefinition {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub fixed_view: Option<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperandPosition {
    pub position: LivenessPosition,
    pub instruction: SelectedInstructionId,
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub access: RegisterOperandAccess,
    pub class: RegisterClassId,
    pub fixed_view: Option<RegisterViewId>,
    pub tied_to: Option<u16>,
    pub early_clobber: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockLiveness {
    pub block: SelectedBlockId,
    pub source_block: BlockId,
    pub virtual_live_in: Vec<VirtualRegisterId>,
    pub virtual_live_out: Vec<VirtualRegisterId>,
    pub unit_live_in: Vec<RegisterUnitId>,
    pub unit_live_out: Vec<RegisterUnitId>,
    pub instructions: Vec<InstructionLiveness>,
    pub successors: Vec<SuccessorLiveness>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionLiveness {
    pub position: LivenessPosition,
    pub instruction: SelectedInstructionId,
    pub virtual_uses: Vec<VirtualRegisterId>,
    pub virtual_defs: Vec<VirtualRegisterId>,
    pub virtual_live_in: Vec<VirtualRegisterId>,
    pub virtual_live_out: Vec<VirtualRegisterId>,
    pub unit_uses: Vec<RegisterUnitId>,
    pub unit_defs: Vec<RegisterUnitId>,
    pub unit_clobbers: Vec<RegisterUnitId>,
    pub unit_live_in: Vec<RegisterUnitId>,
    pub unit_live_out: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessorLiveness {
    pub terminator: SelectedInstructionId,
    /// Canonical branch polarity: zero is nonzero/true, one is zero/false.
    pub polarity_ordinal: u8,
    pub psi_edge: EdgeId,
    pub target: SelectedBlockId,
    pub virtual_live: Vec<VirtualRegisterId>,
    pub unit_live: Vec<RegisterUnitId>,
}
