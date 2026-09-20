//! Decode-level coverage for the description wire's closed vocabularies.
//!
//! The verifier mutation tables in `component_verification/tests.rs` already
//! cover magic, the frontier tag, entry kind and evidence, roster bounds, and
//! canonical ordering. This module pins the vocabulary points that table does
//! not reach: authority class and evidence, custody kind and evidence,
//! obligation kind, realization presence, non-UTF-8 identities, a zero
//! service coordinate, and the overall byte-length bound. Every probe mutates
//! exactly one wire byte, so a vocabulary that silently admitted a
//! non-admitted tag would show up as a missing rejection rather than as a
//! wrong position.

use super::{
    BTreeSet, COMPONENT_DESCRIPTION_SCHEMA_V2, ComponentDescription, ComponentEntry,
    ComponentEntryKind, CustodyConstraint, CustodyEvidence, CustodyKind,
    DescriptionDecodeRejection, DescriptionFrontier, EntryEvidence, ImportSlot,
    InstallationObligation, InstallationServiceBound, MAX_COMPONENT_DESCRIPTION_BYTES,
    ObligationKind, OutgoingAuthority, OutgoingAuthorityClass, OutgoingEvidence, ServiceId,
    decode_component_description, encode_component_description,
};

/// A description whose populated rosters place every closed vocabulary on the
/// wire: each tag byte is present, so a non-admitted value must name the
/// vocabulary in its rejection.
fn complete_roster_description() -> ComponentDescription {
    let module = crate::test_support::bare_module();
    let proof = terminal_psi::ProofBundle::default();
    let record = terminal_codec::build_identity_optimization_execution_record(&module, &proof)
        .expect("identity optimization record");
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_parts(&module, &proof, &record, None)
            .expect("canonical artifact");
    ComponentDescription {
        schema: COMPONENT_DESCRIPTION_SCHEMA_V2,
        frontier: DescriptionFrontier::TerminalArtifactClosure,
        artifact_bytes: artifact.to_bytes(),
        imports: vec![ImportSlot {
            slot: 0,
            requirement_identity: "Unsealed::requirement".into(),
            contract_identity: [8; 32],
        }],
        exports: Vec::new(),
        entries: vec![ComponentEntry {
            kind: ComponentEntryKind::Canonical,
            identity: "entry:canonical".into(),
            evidence: EntryEvidence::ModuleDerived,
        }],
        outgoing: vec![OutgoingAuthority {
            class: OutgoingAuthorityClass::BoundaryRequirement,
            identity: "authority:boundary".into(),
            evidence: OutgoingEvidence::ModuleDerived,
        }],
        service_bounds: vec![InstallationServiceBound {
            requirement_identity: "bound:mask".into(),
            bound: vec![ServiceId::new(1).expect("service identity")],
        }],
        custody: vec![CustodyConstraint {
            kind: CustodyKind::CompletionReceipt,
            identity: "custody:receipt".into(),
            evidence: CustodyEvidence::ModuleDerived,
        }],
        providers: Vec::new(),
        provider_closure_digest: [6; 32],
        obligations: vec![InstallationObligation {
            kind: ObligationKind::ImportBinding,
            identity: "obligation:import".into(),
            detail: "install binds".into(),
        }],
        assumptions: vec![[9; 32]],
        realization_identity: Some([7; 32]),
    }
}

/// Byte range of the embedded Terminal artifact inside the canonical
/// encoding: magic (8) + schema (4) + frontier (1) + length (4), then the
/// artifact payload. The artifact is a nested wire format with its own codec
/// and rejection coverage; this sweep audits the description envelope.
fn artifact_payload_span(canonical: &[u8]) -> std::ops::Range<usize> {
    const PREFIX: usize = 8 + 4 + 1 + 4;
    let artifact_bytes =
        u32::from_le_bytes(canonical[13..PREFIX].try_into().expect("artifact length"));
    PREFIX..PREFIX + artifact_bytes as usize
}

/// Every position whose one-byte corruption rejects, with the rejection the
/// decoder names for it. Positions already holding the fill value are
/// skipped: a no-op mutation proves nothing.
fn rejections_under_fill(canonical: &[u8], fill: u8) -> Vec<(usize, DescriptionDecodeRejection)> {
    let artifact = artifact_payload_span(canonical);
    (0..canonical.len())
        .filter(|&position| canonical[position] != fill && !artifact.contains(&position))
        .filter_map(|position| {
            let mut mutated = canonical.to_vec();
            mutated[position] = fill;
            decode_component_description(&mutated)
                .err()
                .map(|rejection| (position, rejection))
        })
        .collect()
}

#[test]
fn every_closed_wire_vocabulary_rejects_a_non_admitted_tag() {
    let canonical = encode_component_description(&complete_roster_description());
    decode_component_description(&canonical).expect("the complete-roster fixture decodes");

    let rejections = rejections_under_fill(&canonical, 0xee);

    // The wire's closed vocabularies are exactly this audited set: a new tag
    // field added without rejection, or a rejection label moved, fails here.
    let mut found_vocabularies = rejections
        .iter()
        .filter_map(|(_, rejection)| match rejection {
            DescriptionDecodeRejection::UnsupportedTag(label) => Some(*label),
            _ => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    found_vocabularies.sort_unstable();
    let expected_vocabularies = [
        "authority class",
        "authority evidence",
        "custody evidence",
        "custody kind",
        "entry evidence",
        "entry kind",
        "frontier tag",
        "obligation kind",
        "realization presence",
    ];
    assert_eq!(found_vocabularies, expected_vocabularies);

    for label in expected_vocabularies {
        let positions: Vec<_> = rejections
            .iter()
            .filter(|(_, rejection)| {
                *rejection == DescriptionDecodeRejection::UnsupportedTag(label)
            })
            .map(|(position, _)| *position)
            .collect();
        assert!(
            !positions.is_empty(),
            "vocabulary {label} must reject a non-admitted tag value"
        );
    }
}

#[test]
fn non_utf8_identities_reject() {
    let canonical = encode_component_description(&complete_roster_description());
    let rejections = rejections_under_fill(&canonical, 0xee);
    assert!(
        rejections.iter().any(|(_, rejection)| {
            *rejection == DescriptionDecodeRejection::IdentityInvalid("not utf-8")
        }),
        "identity strings must reject bytes outside UTF-8",
    );
}

#[test]
fn zero_service_identity_in_a_bound_rejects() {
    let canonical = encode_component_description(&complete_roster_description());
    let rejections = rejections_under_fill(&canonical, 0);
    assert!(
        rejections.iter().any(|(_, rejection)| {
            *rejection == DescriptionDecodeRejection::Corrupt("zero service identity")
        }),
        "a zeroed service coordinate must reject rather than decode",
    );
}

#[test]
fn oversized_descriptions_reject_before_decoding() {
    let oversized = vec![0; MAX_COMPONENT_DESCRIPTION_BYTES + 1];
    assert_eq!(
        decode_component_description(&oversized),
        Err(DescriptionDecodeRejection::RosterBoundExceeded(
            "description byte length"
        )),
    );
}

#[test]
fn canonical_encoding_round_trips() {
    let description = complete_roster_description();
    let canonical = encode_component_description(&description);
    let decoded = decode_component_description(&canonical).expect("decodes");
    assert_eq!(decoded, description);
    assert_eq!(encode_component_description(&decoded), canonical);
}
