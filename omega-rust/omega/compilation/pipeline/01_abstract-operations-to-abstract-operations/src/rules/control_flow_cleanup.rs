//! Optimizer module role: executable entrance. Control-flow cleanup: the
//! pass's exact rule order, one file per graph transformation beneath it --
//! constant-conditional folding, linear and path-qualified empty-block
//! threading (sharing `empty_block_thread_bindings`), adjacent and
//! non-adjacent block merges (sharing `block_merge_substitutions` and
//! `merge_boundary_ownership` with shared-jump fusion), and unreachable
//! private-machine pruning.

use crate::rules::catalog::BuiltInRuleRegistration;

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

pub use adjacent_block_merge::AdjacentBlockMergeRule;
pub use constant_conditional_fold::ConstantConditionalFoldRule;
pub use linear_empty_block_thread::LinearEmptyBlockThreadRule;
pub use non_adjacent_block_merge::NonAdjacentBlockMergeRule;
pub use path_qualified_empty_block_thread::PathQualifiedEmptyBlockThreadRule;
pub use shared_jump_fusion::SharedJumpFusionRule;
pub use unreachable_private_machine_prune::UnreachablePrivateMachinePruneRule;

#[cfg(test)]
pub(super) use unreachable_private_machine_prune::rule_unreachable_private_machine_complement;

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
