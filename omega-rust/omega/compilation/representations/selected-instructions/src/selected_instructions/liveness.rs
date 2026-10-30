//! Durable selected-instruction liveness facts and their canonical identity.
//!
//! Raw data carries no analysis admission authority; independent replay belongs
//! to the selected-instruction transformation stage.

mod identity;
pub use identity::{liveness_function_digest, liveness_identity_from_digests};

use crate::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use arena::{Arena, HandleSpan};
use optimization_core::OptimizationUnitIdentity;
use register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId};
use semantic_vocabulary::{BlockId, EdgeId, FuelScheduleIdentity, MachineId};
use target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LivenessPosition(pub u32);

crate::identity::sha256_identity!(LivenessIdentity);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LivenessPlan {
    pub selected: SelectedInstructionPlanIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    /// One row per selected function, in plan order. Rows are shared between
    /// the successive plans runtime-spill recovery analyzes: a step that
    /// rewrites one function recomputes that function's row and shares every
    /// other row instead of copying it.
    pub functions: Vec<std::sync::Arc<FunctionLiveness>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionLiveness {
    pub machine: MachineId,
    pub entry_definitions: Vec<EntryDefinition>,
    /// Instruction-owned operand spans retain the producer's grouping. Consumers
    /// must not reconstruct it by searching every operand for each instruction.
    pub operand_positions: Arena<OperandPosition>,
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

impl Default for OperandPosition {
    fn default() -> Self {
        Self {
            position: LivenessPosition(0),
            instruction: SelectedInstructionId(0),
            operand: 0,
            virtual_register: VirtualRegisterId(0),
            access: RegisterOperandAccess::Use,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }
    }
}

impl LivenessPlan {
    /// Every function's blocks, in function order.
    pub fn blocks(&self) -> impl Iterator<Item = &BlockLiveness> + '_ {
        self.functions.iter().flat_map(|function| &function.blocks)
    }

    /// Every function's instruction liveness rows, in function then block order.
    pub fn instructions(&self) -> impl Iterator<Item = &InstructionLiveness> + '_ {
        self.functions
            .iter()
            .flat_map(|function| function.instructions())
    }
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

impl FunctionLiveness {
    /// Every block's instruction liveness rows, in block order.
    pub fn instructions(&self) -> impl Iterator<Item = &InstructionLiveness> + '_ {
        self.blocks.iter().flat_map(|block| &block.instructions)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionLiveness {
    pub position: LivenessPosition,
    pub instruction: SelectedInstructionId,
    pub operands: HandleSpan<OperandPosition>,
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
    pub psi_edge: Option<EdgeId>,
    pub target: SelectedBlockId,
    pub virtual_live: Vec<VirtualRegisterId>,
    pub unit_live: Vec<RegisterUnitId>,
}
