//! control flow scalar in the legalized operations program.

use crate::{LegalizationRecipe, LegalizedCondition, LegalizedLeaf};
use abstract_operations::ValueBinding;
use optimization_unit::FuelSettlement;
use semantic_vocabulary::BlockId;
use semantic_vocabulary::EdgeId;
use semantic_vocabulary::MachineId;
use semantic_vocabulary::ValueId;
use target_operations::TerminalPsiProvenance;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedFunction {
    Conditional(LegalizedConditionalFunction),
}

impl LegalizedFunction {
    #[cfg(test)]
    pub(crate) fn conditional_mut(&mut self) -> &mut LegalizedConditionalFunction {
        match self {
            Self::Conditional(function) => function,
        }
    }
    pub const fn machine(&self) -> MachineId {
        match self {
            Self::Conditional(function) => function.machine,
        }
    }

    pub const fn provenance(&self) -> &TerminalPsiProvenance {
        match self {
            Self::Conditional(function) => &function.provenance,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedConditionalFunction {
    pub machine: MachineId,
    pub attachment: Option<semantic_vocabulary::StructuralTypeId>,
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
