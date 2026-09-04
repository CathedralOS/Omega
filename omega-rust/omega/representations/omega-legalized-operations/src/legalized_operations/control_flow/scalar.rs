//! control flow scalar in the legalized operations program.

use crate::{LegalizationRecipe, LegalizedCondition, LegalizedLeaf};
use omega_abstract_operations::ValueBinding;
use omega_optimization_unit::FuelSettlement;
use omega_target_operations::TerminalPsiProvenance;
use psi_core::BlockId;
use psi_core::EdgeId;
use psi_core::MachineId;
use psi_core::ValueId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedFunction {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub recipe: LegalizationRecipe,
    pub condition_source: ValueId,
    pub condition: LegalizedCondition,
    pub entry_block: BlockId,
    pub true_block: BlockId,
    pub false_block: BlockId,
    pub branch_true_edge: EdgeId,
    pub branch_false_edge: EdgeId,
    pub branch_true_fuel: Vec<FuelSettlement>,
    pub branch_false_fuel: Vec<FuelSettlement>,
    pub branch_true_bindings: Vec<ValueBinding>,
    pub branch_false_bindings: Vec<ValueBinding>,
    pub when_true: LegalizedLeaf,
    pub when_false: LegalizedLeaf,
}
