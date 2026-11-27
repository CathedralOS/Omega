/*
  Durable selected-instruction liveness facts (`LivenessPlan`,
  `FunctionLiveness`, `BlockLiveness`, `InstructionLiveness`).

  Each `FunctionLiveness` row is shared across runtime-spill recovery steps via
  `Arc<FunctionLiveness>`, so rewriting a single function recomputes only that
  function's liveness.
*/

use crate::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use arena::{Arena, HandleSpan};
use optimization_core::OptimizationUnitIdentity;
use register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, WorkScheduleIdentity};
use target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LivenessPosition(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LivenessPlan {
    pub selected: std::sync::Arc<crate::SelectedInstructionPlan>,
    pub optimization_unit: OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub target: NativeTarget,
    pub functions: Vec<std::sync::Arc<FunctionLiveness>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionLiveness {
    pub machine: MachineId,
    pub entry_definitions: Vec<EntryDefinition>,
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
    pub fn blocks(&self) -> impl Iterator<Item = &BlockLiveness> + '_ {
        self.functions.iter().flat_map(|function| &function.blocks)
    }

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
    pub polarity_ordinal: u8,
    pub psi_edge: Option<EdgeId>,
    pub target: SelectedBlockId,
    pub virtual_live: Vec<VirtualRegisterId>,
    pub unit_live: Vec<RegisterUnitId>,
}
