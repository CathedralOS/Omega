//! calls projected returns in the legalized operations program.

use crate::ProjectedStructuralCallReturnLegalizationRecipe;
use optimization_unit::EffectLink;
use optimization_unit::FuelSettlement;
use optimization_unit::OwnershipEvent;
use semantic_vocabulary::BlockId;

/// Exact target and optimizer custody for one two-function projected-roster
/// closure. Keeping the pair atomic prevents either local function from
/// acquiring qualification authority without its matching peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedProjectedStructuralCallReturn {
    pub recipe: ProjectedStructuralCallReturnLegalizationRecipe,
    pub caller: target_operations::TargetFunction,
    pub callee: target_operations::TargetFunction,
    pub caller_entry_block: BlockId,
    pub callee_entry_block: BlockId,
    pub caller_nodes: Vec<LegalizedStructuralNodeCustody>,
    pub callee_nodes: Vec<LegalizedStructuralNodeCustody>,
}

/// Optimizer metadata retained beside an identity-legalized structural node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedStructuralNodeCustody {
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}
