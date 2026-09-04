//! control flow unit in the legalized operations program.

use crate::UnitLegalizationRecipe;
use omega_optimization_unit::FuelSettlement;
use omega_target_operations::TerminalPsiProvenance;
use psi_core::BlockId;
use psi_core::EdgeId;
use psi_core::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedUnitFunction {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub recipe: UnitLegalizationRecipe,
    pub entry_block: BlockId,
    pub return_edge: EdgeId,
    pub return_fuel: Vec<FuelSettlement>,
}
