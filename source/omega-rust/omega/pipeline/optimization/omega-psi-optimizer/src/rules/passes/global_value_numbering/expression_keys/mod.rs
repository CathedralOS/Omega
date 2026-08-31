//! Optimizer module role: stage group. Canonical scalar-expression identities shared by every GVN traversal.
//!
//! [`model`] owns the closed key vocabularies and operand translation,
//! [`total`] classifies obligation-free operations, [`proof_certified`]
//! classifies proof-bearing operations, and [`compatible_policy`] owns the
//! deliberately asymmetric leader/redundant policy join.

mod compatible_policy;
mod model;
mod proof_certified;
mod total;

use super::*;

pub(in crate::rules::passes) use compatible_policy::{
    compatible_policy_scalar_leader, compatible_policy_scalar_redundant,
};
pub(in crate::rules::passes) use model::{
    CompatiblePolicyScalarExpressionKey, ProofCertifiedScalarExpressionKey,
};
pub(super) use model::TotalScalarExpressionKey;
pub(in crate::rules::passes) use proof_certified::proof_certified_scalar_expression;
pub(super) use total::total_scalar_expression;

type ScalarExpressionRow<K> = (K, OperationId, ValueId, ScalarType);

fn canonical_pair(left: ValueId, right: ValueId) -> (ValueId, ValueId) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}
