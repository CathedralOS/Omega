//! Canonical crash-route, evidence-interface, and machine-contract encoding.

use super::{
    CanonicalBytes, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard,
    EvidenceContractLane, EvidenceContractLaneKind, EvidenceInterfaceIdentity, MachineContract,
    OutcomeSpecificCallEvidence, encode_ids, encode_optional,
};
use crate::identity::proposition_encoding::encode_proposition;

pub(in crate::identity) fn encode_crash_cause(bytes: &mut CanonicalBytes, cause: CrashCause) {
    bytes.u8(match cause {
        CrashCause::Trap => 1,
        CrashCause::Abort => 2,
    });
}

pub(in crate::identity) fn encode_crash_predicate(
    bytes: &mut CanonicalBytes,
    predicate: &CrashPredicateTerm,
) {
    encode_proposition(bytes, predicate.proposition());
}

pub(in crate::identity) fn encode_crash_route_bucket(
    bytes: &mut CanonicalBytes,
    bucket: &CrashRouteBucket,
) {
    encode_crash_cause(bytes, bucket.cause);
    bytes.slice(
        &bucket.alternatives,
        |bytes, alternative| match alternative {
            CrashRouteGuard::Truth => bytes.u8(1),
            CrashRouteGuard::Predicate(predicate) => {
                bytes.u8(2);
                encode_crash_predicate(bytes, predicate);
            }
        },
    );
}

fn encode_evidence_interface(bytes: &mut CanonicalBytes, interface: &EvidenceInterfaceIdentity) {
    bytes.string(&interface.trait_identity);
    bytes.slice(&interface.arguments, |bytes, argument| {
        bytes.string(argument)
    });
    bytes.slice(&interface.requirements, |bytes, requirement| {
        bytes.string(&requirement.declaring_trait_identity);
        bytes.slice(&requirement.declaring_trait_arguments, |bytes, argument| {
            bytes.string(argument)
        });
        bytes.string(&requirement.requirement_identity);
    });
}

pub(in crate::identity) fn encode_outcome_specific_call_evidence(
    bytes: &mut CanonicalBytes,
    evidence: &OutcomeSpecificCallEvidence,
) {
    bytes.id(evidence.guard.result_type);
    bytes.id(evidence.guard.result_case);
    bytes.u32(evidence.position);
    bytes.id(evidence.callee_obligation);
    bytes.id(evidence.callee_term);
    bytes.string(&evidence.output_field);
    bytes.id(evidence.callee_proposition);
    bytes.id(evidence.instantiated_proposition);
    bytes.id(evidence.output);
    encode_optional(
        bytes,
        evidence.result_substitution.as_ref(),
        |bytes, substitution| {
            bytes.u32(substitution.argument_position);
            bytes.id(substitution.callee_result);
            bytes.id(substitution.caller_result);
        },
    );
    bytes.id(evidence.validity.result);
    encode_ids(bytes, &evidence.validity.proposition_dependencies);
    encode_evidence_interface(bytes, &evidence.validity.evidence_interface);
    encode_ids(bytes, &evidence.validity.interface_dependencies);
}

pub(in crate::identity) fn encode_machine_contract(
    bytes: &mut CanonicalBytes,
    contract: &MachineContract,
) {
    bytes.id(contract.id);
    bytes.slice(&contract.crash_routes, encode_crash_route_bucket);
    bytes.slice(&contract.requires, encode_proposition);
    bytes.slice(&contract.ensures, |bytes, clause| {
        bytes.id(clause.obligation);
        encode_proposition(bytes, &clause.proposition);
    });
    bytes.slice(&contract.outcome_specific_ensures, |bytes, row| {
        bytes.id(row.guard.result_type);
        bytes.id(row.guard.result_case);
        bytes.u32(row.position);
        bytes.id(row.obligation);
        encode_proposition(bytes, &row.proposition);
        encode_optional(bytes, row.evidence.as_ref(), |bytes, evidence| {
            bytes.id(evidence.term);
            bytes.string(&evidence.output_field);
        });
    });
}

pub(in crate::identity) fn encode_evidence_contract_lane(
    bytes: &mut CanonicalBytes,
    lane: &EvidenceContractLane,
) {
    bytes.id(lane.machine);
    bytes.u8(match lane.kind {
        EvidenceContractLaneKind::Requires => 1,
        EvidenceContractLaneKind::Ensures => 2,
    });
    bytes.u32(lane.position);
    bytes.id(lane.term);
    encode_optional(bytes, lane.output_field.as_ref(), |bytes, output| {
        bytes.string(output)
    });
}
