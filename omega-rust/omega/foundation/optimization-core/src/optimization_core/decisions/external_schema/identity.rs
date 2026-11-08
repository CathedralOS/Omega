use crate::{
    OptimizationDecisionLogIdentity, OptimizationDecisionSchemaIdentity,
    OptimizationDecisionTargetIdentity,
};

use super::codec::VERSION;
use super::{ExternalDecisionAction, ExternalDecisionContext, ExternalDecisionPoint};

/// Bytes of one external-schema identity domain: the format name, the single
/// wire-version source, and the family's terminator convention.
pub(super) fn versioned_domain(prefix: &str, terminated: bool) -> Vec<u8> {
    let mut domain = format!("{prefix}.v{VERSION}").into_bytes();
    if terminated {
        domain.push(0);
    }
    domain
}

/// Closed feature schema for target-neutral Psi policy decisions. Its domain
/// carries [`VERSION`], so a wire-version bump moves the schema identity with
/// the wire format.
///
/// Candidate rows contain only canonical identities, signed structural cost,
/// the authoritative rule-analysis set, and authoritative typed fact
/// references. Paths, pointers, authored names, arena order, and debug text
/// remain unrepresentable.
pub fn external_psi_decision_schema_identity() -> OptimizationDecisionSchemaIdentity {
    OptimizationDecisionSchemaIdentity::from_canonical_bytes(&versioned_domain(
        "omega.external-psi-decision-schema",
        false,
    ))
}

/// Psi rules run before target selection. That absence is an explicit context
/// identity rather than an omitted or all-zero target field.
pub fn psi_target_neutral_decision_target_identity() -> OptimizationDecisionTargetIdentity {
    OptimizationDecisionTargetIdentity::from_canonical_bytes(&versioned_domain(
        "omega.psi-target-neutral-decision-context",
        false,
    ))
}

pub(super) fn log_identity(
    context: ExternalDecisionContext,
    points: &[ExternalDecisionPoint],
) -> OptimizationDecisionLogIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(&versioned_domain("omega.external-decision-log", true));
    encode_context(&mut canonical, context);
    crate::codec_bytes::push_count_u64(&mut canonical, points.len());
    for point in points {
        canonical.extend_from_slice(&point.identity.bytes());
    }
    OptimizationDecisionLogIdentity::from_canonical_bytes(&canonical)
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
