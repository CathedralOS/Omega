//! Opposite non-strict bounds become ordinary proved equality edges.
//!
//! Only unconditional cited bounds and independently decided carrier bounds
//! participate. Each derived edge retains both proofs; it never extends the
//! caller's semantic axioms. The existing equality graph and definition
//! transport then use the edge for any scalar expression.

use std::collections::BTreeMap;

use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule, decide_primitive};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType};

pub(super) fn derive(context: &PropositionContext, bounds: Vec<ProofNode>) -> Vec<ProofNode> {
    let bounds = bounds
        .into_iter()
        .map(|proof| (proof.conclusion.clone(), proof))
        .collect::<BTreeMap<_, _>>();
    let mut equalities = Vec::new();
    for (proposition, forward) in &bounds {
        if equalities.len() >= super::MAXIMUM_NORMALIZATION_ENDPOINTS {
            break;
        }
        let Proposition::LessOrEqual(left, right) = proposition else {
            continue;
        };
        let ScalarType::Integer(carrier) = left.scalar_type() else {
            continue;
        };
        if carrier.is_address() || right.scalar_type() != left.scalar_type() || left == right {
            continue;
        }
        let reverse = Proposition::LessOrEqual(right.clone(), left.clone());
        let backward = if let Some(proof) = bounds.get(&reverse) {
            proof.clone()
        } else if decide_primitive(context, &reverse, PrimitiveJudgment::IntegerCarrierBound)
            .is_ok()
        {
            ProofNode {
                conclusion: reverse,
                rule: ProofRule::Primitive(PrimitiveJudgment::IntegerCarrierBound),
            }
        } else {
            continue;
        };
        // Definitions use value = term. Orient a literal/value pair this way
        // while preserving the exact endpoint order of each cited bound.
        let (left, right, forward, backward) = if !matches!(left, ScalarTerm::Value { .. })
            && matches!(right, ScalarTerm::Value { .. })
        {
            (right, left, backward, forward.clone())
        } else {
            (left, right, forward.clone(), backward)
        };
        let conclusion = Proposition::Equal(left.clone(), right.clone());
        if equalities
            .iter()
            .any(|proof: &ProofNode| proof.conclusion == conclusion)
        {
            continue;
        }
        equalities.push(ProofNode {
            conclusion,
            rule: ProofRule::IntegerOrderAntisymmetry {
                forward: Box::new(forward),
                backward: Box::new(backward),
            },
        });
    }
    equalities
}

#[cfg(test)]
mod tests;
