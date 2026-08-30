//! Control-flow cleanup, arranged by the graph transformation being performed.

use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use omega_optimization_unit::{
    AdjacentBlockMergeRewrite, ConstantConditionalRewrite, LinearEmptyBlockRewrite, NodeLocation,
    NonAdjacentBlockMergeRewrite, OwnershipFrontierSite, OwnershipFrontierWitness,
    OwnershipFrontierWitnessRow, PathQualifiedEmptyBlockRewrite, ProvenanceDisposition,
    ProvenanceRewrite, PrunedMachineCustody, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, PsiRewriteCandidate, ScalarSubstitution, SharedJumpFusionRewrite,
    UnreachablePrivateMachinesRewrite,
};
use psi_core::{BlockId, MachineId};

use crate::rules::catalog::BuiltInRuleRegistration;
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::{
    CONTROL_FLOW_CLEANUP_PASS_NAME, boolean_constant,
    support::{block_dominates, replacement_dominates_parameter_uses},
};

mod block_merging;
mod constant_conditionals;
mod empty_block_threading;
mod shared_jump_fusion;
mod unreachable_private_machines;

pub use block_merging::*;
pub use constant_conditionals::*;
pub use empty_block_threading::*;
pub use shared_jump_fusion::*;
pub use unreachable_private_machines::*;

use block_merging::adjacent_merge_ownership_is_identity;
#[cfg(test)]
pub(in crate::rules::passes) use unreachable_private_machines::rule_unreachable_private_machine_complement;

/// The exact local rule order for this pass.
pub(in crate::rules) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
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
