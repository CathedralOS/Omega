use crate::{
    OptimizationDecisionIdentity, OptimizationDecisionLogIdentity,
    OptimizationDecisionSchemaIdentity, OptimizationDecisionTargetIdentity,
    OptimizationRuleIdentity, OptimizationUnitIdentity,
};

use super::{
    ExternalCandidateFeatures, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionPoint,
};

/// Closed v2 feature schema for target-neutral Psi policy decisions.
///
/// Candidate rows contain only canonical identities, signed structural cost,
/// the authoritative rule-analysis set, and authoritative typed fact
/// references. Paths, pointers, authored names, arena order, and debug text
/// remain unrepresentable.
pub fn external_psi_decision_schema_v2_identity() -> OptimizationDecisionSchemaIdentity {
    OptimizationDecisionSchemaIdentity::from_canonical_bytes(
        b"omega.external-psi-decision-schema.v2",
    )
}

/// Psi rules run before target selection. That absence is an explicit context
/// identity rather than an omitted or all-zero target field.
pub fn psi_target_neutral_decision_target_v2_identity() -> OptimizationDecisionTargetIdentity {
    OptimizationDecisionTargetIdentity::from_canonical_bytes(
        b"omega.psi-target-neutral-decision-context.v2",
    )
}

pub(super) fn point_identity(
    input: OptimizationUnitIdentity,
    rule: OptimizationRuleIdentity,
    candidates: &[ExternalCandidateFeatures],
    action: ExternalDecisionAction,
) -> OptimizationDecisionIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.external-decision-point.v2\0");
    canonical.extend_from_slice(&input.bytes());
    canonical.extend_from_slice(&rule.bytes());
    crate::codec_bytes::push_count_u64(&mut canonical, candidates.len());
    for candidate in candidates {
        encode_candidate_features(&mut canonical, candidate);
    }
    encode_action(&mut canonical, action);
    OptimizationDecisionIdentity::from_canonical_bytes(&canonical)
}

pub(super) fn log_identity(
    context: ExternalDecisionContext,
    points: &[ExternalDecisionPoint],
) -> OptimizationDecisionLogIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.external-decision-log.v2\0");
    encode_context(&mut canonical, context);
    crate::codec_bytes::push_count_u64(&mut canonical, points.len());
    for point in points {
        canonical.extend_from_slice(&point.identity.bytes());
    }
    OptimizationDecisionLogIdentity::from_canonical_bytes(&canonical)
}

fn encode_candidate_features(encoded: &mut Vec<u8>, features: &ExternalCandidateFeatures) {
    encoded.extend_from_slice(&features.summary.candidate.bytes());
    crate::codec_bytes::push_encoded(encoded, features.summary.predicted_cost_delta.to_le_bytes());
    encoded.extend_from_slice(&features.consumed_analyses.encode());
    crate::codec_bytes::push_count_u64(encoded, features.consumed_facts.len());
    for fact in &features.consumed_facts {
        encoded.extend_from_slice(&fact.encode());
    }
}

pub(super) fn encode_context(encoded: &mut Vec<u8>, context: ExternalDecisionContext) {
    encoded.extend_from_slice(&context.schema.bytes());
    encoded.extend_from_slice(&context.source.bytes());
    encoded.extend_from_slice(&context.selections.bytes());
    encoded.extend_from_slice(&context.phase_selections.bytes());
    encoded.extend_from_slice(&context.target.bytes());
    encoded.extend_from_slice(&context.rule_set.bytes());
    encoded.extend_from_slice(&context.cost_model.bytes());
}

pub(super) fn encode_action(encoded: &mut Vec<u8>, action: ExternalDecisionAction) {
    match action {
        ExternalDecisionAction::Choose(candidate) => {
            encoded.push(1);
            encoded.extend_from_slice(&candidate.bytes());
        }
        ExternalDecisionAction::Skip(reason) => {
            encoded.push(2);
            encoded.push(reason as u8);
        }
    }
}
