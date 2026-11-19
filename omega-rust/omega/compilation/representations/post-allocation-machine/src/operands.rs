// We define physical register storage, read/write unit footprints, and
// instruction-qualified physical dependency reads (`QualifiedPhysicalRead`).
// A fused branch has no selected operands of its own, so its dependency is
// qualified by its owning selected instruction rather than represented as
// branch operand zero.

use register_model::{
    RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId, RegisterWriteSemantics,
};
use selected_instructions::{SelectedInstructionId, VirtualRegisterId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedPhysicalRead {
    pub source_instruction: SelectedInstructionId,
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
    pub units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalOperandFootprint {
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
    pub access: RegisterOperandAccess,
    pub storage_units: Vec<RegisterUnitId>,
    pub read_units: Vec<RegisterUnitId>,
    pub write_units: Vec<RegisterUnitId>,
    pub write_semantics: Option<RegisterWriteSemantics>,
}
