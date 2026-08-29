//! Global value numbering, arranged by expression identity and traversal scope.

mod catalog;

pub(in crate::rules) use catalog::built_in_registrations;

use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use omega_optimization_unit::{
    DominatingScalarCommonSubexpressionRewrite, LocalScalarCommonSubexpressionRewrite,
    NodeLocation, OptimizationFact, PhiTranslatedScalarGvnRewrite, PhiTranslatedScalarIncoming,
    ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite,
    PsiRewriteCandidate,
};
use psi_core::{BlockId, IntegerType, IntegerValue, MachineId, OperationId, ScalarType, ValueId};

use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::{GLOBAL_VALUE_NUMBERING_PASS_NAME, accepted_obligation_fact, support::block_dominates};

mod accounting;
mod dominating;
mod expression_keys;
mod identities;
mod local;
mod phi_translated;

pub use dominating::*;
pub use identities::*;
pub use local::*;
pub use phi_translated::*;

pub(in crate::rules::passes) use accounting::local_cse_accounting;
use accounting::*;
use expression_keys::*;
pub(in crate::rules::passes) use expression_keys::{
    compatible_policy_scalar_leader, compatible_policy_scalar_redundant,
    proof_certified_scalar_expression,
};
