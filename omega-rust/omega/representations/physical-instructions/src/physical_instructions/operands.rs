//! Register storage, reads, and writes for each authored operand.

use register_model::{
    RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId, RegisterWriteSemantics,
};
use selected_instructions::VirtualRegisterId;

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
