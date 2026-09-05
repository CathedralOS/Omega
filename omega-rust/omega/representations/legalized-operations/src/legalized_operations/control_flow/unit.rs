//! control flow unit in the legalized operations program.

use crate::UnitLegalizationRecipe;
use optimization_unit::FuelSettlement;
use semantic_vocabulary::BlockId;
use semantic_vocabulary::EdgeId;
use semantic_vocabulary::MachineId;
use target_operations::TerminalPsiProvenance;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedUnitFunction {
    pub machine: MachineId,
    pub attachment: Option<semantic_vocabulary::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub recipe: UnitLegalizationRecipe,
    pub entry_block: BlockId,
    pub return_edge: EdgeId,
    pub return_fuel: Vec<FuelSettlement>,
}
