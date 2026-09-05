//! Selected machine alternatives and complete physical register actions.

use crate::PhysicalOperandFootprint;
use register_model::RegisterUnitId;
use selected_instructions::{MachineAlternative, SelectedInstructionId};

/// This is a legality rule, not an optimization level or cost policy. Current
/// target catalogs must partition physical-home configurations so exactly one
/// declared alternative applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineAlternativeChoiceRule {
    UniqueApplicableInCatalogOrderV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachineInstruction {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternative,
    pub operands: Vec<PhysicalOperandFootprint>,
    pub implicit_unit_uses: Vec<RegisterUnitId>,
    pub implicit_unit_defs: Vec<RegisterUnitId>,
    pub implicit_unit_clobbers: Vec<RegisterUnitId>,
    pub unit_uses: Vec<RegisterUnitId>,
    pub unit_defs: Vec<RegisterUnitId>,
    pub unit_clobbers: Vec<RegisterUnitId>,
}
