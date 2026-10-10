//! Optimizer module role: executable entrance. Control-flow cleanup: the
//! pass's exact rule order, one file per graph transformation beneath it --
//! constant-conditional folding, linear and path-qualified empty-block
//! threading (sharing `empty_block_thread_bindings`), adjacent and
//! non-adjacent block merges (sharing `block_merge_substitutions` and
//! `merge_boundary_ownership` with shared-jump fusion), and unreachable
//! private-machine pruning.

use crate::rules::catalog::BuiltInRuleRegistration;
use optimization_unit::{
    FuelSettlement, ProvenanceDisposition, ProvenanceRewrite, PsiProvenance,
    PsiRealizationSite,
};

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
