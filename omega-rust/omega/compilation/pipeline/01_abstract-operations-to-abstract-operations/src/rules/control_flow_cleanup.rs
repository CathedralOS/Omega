//! Optimizer module role: executable entrance. Control-flow cleanup: the
//! pass's exact rule order, one file per graph transformation beneath it --
//! constant-conditional folding, linear and path-qualified empty-block
//! threading (sharing `empty_block_thread_bindings`), adjacent and
//! non-adjacent block merges (sharing `block_merge_substitutions` and
//! `merge_boundary_ownership` with shared-jump fusion), and unreachable
//! private-machine pruning.

use crate::rules::catalog::BuiltInRuleRegistration;
use optimization_unit::{
    FuelSettlement, NodeLocation, OptimizationBlock, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationFunction, PsiProvenance, PsiRealizationSite,
};
use semantic_vocabulary::BlockId;
use std::collections::BTreeSet;

mod adjacent_block_merge;
mod block_merge_substitutions;
mod constant_conditional_fold;
mod empty_block_thread_bindings;
mod linear_empty_block_thread;
mod merge_boundary_ownership;
mod non_adjacent_block_merge;
mod path_qualified_empty_block_thread;
mod shared_jump_fusion;
mod unreachable_private_machine_prune;

pub(crate) use adjacent_block_merge::AdjacentBlockMergeRule;
pub(crate) use constant_conditional_fold::ConstantConditionalFoldRule;
pub(crate) use linear_empty_block_thread::LinearEmptyBlockThreadRule;
pub(crate) use non_adjacent_block_merge::NonAdjacentBlockMergeRule;
pub(crate) use path_qualified_empty_block_thread::PathQualifiedEmptyBlockThreadRule;
pub(crate) use shared_jump_fusion::SharedJumpFusionRule;
pub(crate) use unreachable_private_machine_prune::UnreachablePrivateMachinePruneRule;

/// The exact local rule order for this pass.
pub(super) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, ConstantConditionalFoldRule),
        BuiltInRuleRegistration::new(1, LinearEmptyBlockThreadRule),
        BuiltInRuleRegistration::new(2, PathQualifiedEmptyBlockThreadRule),
        BuiltInRuleRegistration::new(3, AdjacentBlockMergeRule),
        BuiltInRuleRegistration::new(4, SharedJumpFusionRule),
        BuiltInRuleRegistration::new(5, UnreachablePrivateMachinePruneRule),
        BuiltInRuleRegistration::new(6, NonAdjacentBlockMergeRule),
    ]
}

/// One realized-at rewrite shared by every cleanup rule's site stamp: the
/// disposition lands at `realized_at` while provenance and fuel move over from
/// the stamped source.
pub(super) fn provenance_realized_at(
    input: PsiRealizationSite,
    realized_at: PsiRealizationSite,
    sources: &[PsiProvenance],
    fuel: &[FuelSettlement],
) -> ProvenanceRewrite {
    ProvenanceRewrite {
        input,
        disposition: ProvenanceDisposition::RealizedAt(realized_at),
        sources: sources.to_vec(),
        fuel: fuel.to_vec(),
    }
}

/// The unreachable-site twin of `provenance_realized_at`: the disposition lands
/// at `unreachable_at` while provenance and fuel move over from the source.
pub(super) fn provenance_proven_unreachable_at(
    input: PsiRealizationSite,
    unreachable_at: PsiRealizationSite,
    sources: &[PsiProvenance],
    fuel: &[FuelSettlement],
) -> ProvenanceRewrite {
    ProvenanceRewrite {
        input,
        disposition: ProvenanceDisposition::ProvenUnreachableAt(unreachable_at),
        sources: sources.to_vec(),
        fuel: fuel.to_vec(),
    }
}

/// One block's node scan against the running effect ledger: a node whose
/// declared input or output no longer matches the expected ordinal marks its
/// block affected and stamps realized provenance at the node site. Shared by
/// the cleanup rules' accounting epilogues; `excluded` is the one decision
/// site exempt from the drift check.
pub(super) fn record_block_effect_drift(
    function: &PsiOptimizationFunction,
    block: &OptimizationBlock,
    expected_effect: &mut u64,
    excluded: Option<NodeLocation>,
    affected: &mut BTreeSet<BlockId>,
    realized: &mut Vec<ProvenanceRewrite>,
) -> Option<()> {
    for (node_index, node) in block.nodes.iter().enumerate() {
        let location = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(node_index).ok()?,
        };
        let effect_changes = node.effect.input != *expected_effect
            || node.effect.output != expected_effect.checked_add(1)?;
        if effect_changes && excluded != Some(location) {
            affected.insert(block.id);
            if !node.provenance.is_empty() {
                let site = PsiRealizationSite::Node(location);
                realized.push(provenance_realized_at(
                    site,
                    site,
                    &node.provenance,
                    &node.fuel,
                ));
            }
        }
        *expected_effect = expected_effect.checked_add(1)?;
    }
    Some(())
}
