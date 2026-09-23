//! Regression coverage for subject-sealed canonical proof sections.
//!
//! A sealed proof section names the exact reconstructed semantic subject its
//! bundle was admitted for. A section sealed for one subject must not be
//! replayed for another module even when every compact obligation coordinate
//! (obligation identity and proposition) coincides.

use super::{canonical_artifact, kernel_bundle, machine_id, semantic_module};
use proof_admission::AdmissionProfile;
use terminal_codec::{
    CanonicalTerminalArtifact, CanonicalTerminalArtifactError, PccReceiverPolicy,
    PccVerificationOutcome, ProofCodecError, admission_profile_identity,
    build_identity_optimization_execution_record, build_psi_proof_sidecar, decode_proof_bundle,
    decode_proof_section, decode_proof_section_for, encode_module, encode_proof_bundle,
    encode_proof_section, encode_psi_optimization_execution_record, terminal_psi_identity,
    verify_psi_proof_sidecar, verify_terminal_artifact_proof,
};
use terminal_verifier::verify_module;

/// A module that differs from `semantic_module` only in machine identity, so
/// its reconstructed obligation set keeps every compact coordinate of the
/// original while its semantic identity changes.
fn foreign_subject_module() -> terminal_psi::TerminalModule {
    let mut foreign = semantic_module();
    foreign.entry = machine_id(2);
    foreign.machines[0].id = machine_id(2);
    foreign
}

#[test]
fn sealed_proof_section_round_trips_and_exposes_the_reconstructed_subject() {
    let module = semantic_module();
    let bundle = kernel_bundle();
    let section = encode_proof_section(&module, &bundle).expect("sealed proof section");
    assert_eq!(&section[..8], b"PSIPSC\0\0");

    let (claimed, decoded) = decode_proof_section(&section).expect("decode sealed section");
    assert_eq!(claimed, terminal_psi_identity(&module).unwrap());
    assert_eq!(decoded, bundle);
    assert_eq!(
        decode_proof_section_for(&module, &section).expect("own subject"),
        bundle
    );
}

#[test]
fn sealed_proof_section_rejects_replay_for_another_subject_with_coinciding_coordinates() {
    let module = semantic_module();
    let bundle = kernel_bundle();
    let sealed = encode_proof_section(&module, &bundle).unwrap();
    let foreign = foreign_subject_module();

    // The semantic subjects differ, but every compact obligation coordinate
    // coincides: the bare bundle still verifies for the foreign module.
    let module_subject = terminal_psi_identity(&module).unwrap();
    let foreign_subject = terminal_psi_identity(&foreign).unwrap();
    assert_ne!(module_subject, foreign_subject);
    verify_module(&foreign, &bundle, &AdmissionProfile::default())
        .expect("compact coordinates coincide: the bare bundle still verifies");

    // The sealed section cannot be replayed for the foreign subject.
    let mismatch = Err(ProofCodecError::ProofSubjectMismatch {
        claimed: module_subject,
        reconstructed: foreign_subject,
    });
    assert_eq!(decode_proof_section_for(&foreign, &sealed), mismatch);

    // Legacy in-process decode still yields the bundle but discards the seal.
    assert_eq!(decode_proof_bundle(&sealed).unwrap(), bundle);

    // An unsealed bundle is not a proof section: the subject-paired decode
    // rejects it before any module comparison.
    let bare = encode_proof_bundle(&bundle).unwrap();
    assert!(matches!(
        decode_proof_section_for(&foreign, &bare),
        Err(ProofCodecError::InvalidMagic)
    ));
}

#[test]
fn canonical_artifact_rejects_a_proof_section_sealed_for_another_subject() {
    let module = semantic_module();
    let bundle = kernel_bundle();
    let artifact = canonical_artifact(&module, &bundle, None);

    // Canonical artifacts carry the sealed proof section.
    assert_eq!(&artifact.proof_bytes()[..8], b"PSIPSC\0\0");

    let foreign = foreign_subject_module();
    let foreign_semantic = encode_module(&foreign).unwrap();
    let foreign_optimization = encode_psi_optimization_execution_record(
        &build_identity_optimization_execution_record(&foreign, &bundle).unwrap(),
    );

    // Forge an envelope pairing the foreign semantic section with the
    // original artifact's sealed proof section.
    let mut forged = Vec::new();
    forged.extend_from_slice(b"PSIART\0\0");
    forged.extend_from_slice(&2_u16.to_le_bytes());
    for len in [
        foreign_semantic.len(),
        artifact.proof_bytes().len(),
        foreign_optimization.len(),
    ] {
        forged.extend_from_slice(&u64::try_from(len).unwrap().to_le_bytes());
    }
    forged.push(0);
    forged.extend_from_slice(&foreign_semantic);
    forged.extend_from_slice(artifact.proof_bytes());
    forged.extend_from_slice(&foreign_optimization);

    assert!(matches!(
        CanonicalTerminalArtifact::from_bytes(&forged),
        Err(CanonicalTerminalArtifactError::Proof(
            ProofCodecError::ProofSubjectMismatch { .. }
        ))
    ));
}

#[test]
fn pcc_replay_reports_the_qualified_subject_ledger_and_admissions() {
    let module = semantic_module();
    let bundle = kernel_bundle();
    let artifact = canonical_artifact(&module, &bundle, None);
    let profile = AdmissionProfile::default();

    let verdict =
        verify_terminal_artifact_proof(&artifact, &profile).expect("artifact proof replays");
    assert_eq!(
        verdict.semantic_subject,
        terminal_psi_identity(&module).unwrap()
    );
    assert_eq!(verdict.obligation_ledger, artifact.manifest().obligations());
    // The profile names the module's OWN vocabulary marker, so derive it: a
    // spelled number here only restates the constant until the constant moves,
    // and then it fails for a reason that has nothing to do with replay.
    assert_eq!(
        verdict.semantic_profile,
        format!(
            "terminal-psi-vocabulary-{}",
            terminal_psi::VocabularyMarker::CURRENT.get()
        )
    );
    assert_eq!(
        verdict.checker_profile,
        admission_profile_identity(&profile)
    );
    assert!(verdict.admissions.is_empty());

    let artifact_bytes = artifact.to_bytes();
    let sidecar =
        build_psi_proof_sidecar(&artifact, &profile, &artifact_bytes).expect("rebuilt claim");
    let policy = PccReceiverPolicy::for_offered_claim(&sidecar, profile);
    let PccVerificationOutcome::Complete(product) =
        verify_psi_proof_sidecar(&artifact_bytes, &sidecar.to_bytes(), &policy)
    else {
        panic!("expected a complete verified pair");
    };
    assert_eq!(
        product.semantic_subject,
        terminal_psi_identity(&module).unwrap()
    );
    assert_eq!(product.obligation_ledger, artifact.manifest().obligations());
    assert_eq!(product.semantic_profile, verdict.semantic_profile);
    assert_eq!(product.checker_profile, verdict.checker_profile);
}
