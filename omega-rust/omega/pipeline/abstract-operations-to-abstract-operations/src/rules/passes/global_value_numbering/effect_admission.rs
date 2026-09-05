//! Exact effect admission shared by global-value-numbering rules.

use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{BlockId, MachineId};

use crate::EffectSummaryAnalysis;

pub(super) fn exact_pure_scalar_effect(
    unit: &PsiOptimizationUnit,
    effects: &EffectSummaryAnalysis,
    machine: MachineId,
    block: BlockId,
    node: u32,
) -> bool {
    effects.nodes.iter().any(|row| {
        row.revision == unit.identity
            && row.machine == machine
            && row.block == block
            && row.node == node
            && row.class == crate::EffectClass::PureScalar
            && row.observable == crate::EffectKnowledge::No
            && row.structural_state == crate::EffectKnowledge::No
            && row.crash == crate::EffectKnowledge::No
            && row.suspension == crate::EffectKnowledge::No
    })
}
