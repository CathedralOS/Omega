//! The PCC replay gate at the interpretation boundary: every public artifact
//! entry decodes and independently verifies before it constructs execution
//! state, so hostile or substituted evidence rejects before interpretation.
//! An artifact carrying no PCC claims still executes under the ordinary
//! admission profile.

use super::{
    AcceptTerminalEffects, AdmissionProfile, ProofBundle, TerminalExecutionResult,
    TerminalStructuralInputs, encode_module, encode_proof_section, machine_id,
    payloadless_call_module, unit_module,
};
use terminal_codec::{CanonicalTerminalArtifact, build_identity_optimization_execution_record};
use terminal_interpreter::{
    TerminalArtifactInterpretError, admit_provider_installation_from_artifact,
    interpret_serialized_terminal_artifact_measured, interpret_terminal_artifact,
};

fn encoded(module: &terminal_psi::TerminalModule) -> (Vec<u8>, Vec<u8>) {
    (
        encode_module(module).expect("semantic section encodes"),
        encode_proof_section(module, &ProofBundle::default()).expect("proof section encodes"),
    )
}

fn envelope(module: &terminal_psi::TerminalModule) -> Vec<u8> {
    let proof = ProofBundle::default();
    let optimization =
        build_identity_optimization_execution_record(module, &proof).expect("identity record");
    CanonicalTerminalArtifact::from_parts(module, &proof, &optimization, None)
        .expect("canonical artifact")
        .to_bytes()
}

fn is_entry_stage(error: &TerminalArtifactInterpretError) -> bool {
    matches!(
        error,
        TerminalArtifactInterpretError::ArtifactDecode(_)
            | TerminalArtifactInterpretError::SemanticDecode(_)
            | TerminalArtifactInterpretError::ProofDecode(_)
            | TerminalArtifactInterpretError::Verification(_)
    )
}

#[test]
fn ordinary_artifact_without_pcc_claims_still_interprets() {
    let (semantic, proof) = encoded(&unit_module());
    assert!(matches!(
        interpret_terminal_artifact(&semantic, &proof, &AdmissionProfile::default(), &[],),
        Ok(TerminalExecutionResult::Unit)
    ));
}

#[test]
fn proof_section_sealed_for_another_subject_rejects_before_interpretation() {
    let (semantic, _proof) = encoded(&unit_module());
    let (_foreign_semantic, foreign_proof) = encoded(&payloadless_call_module());
    assert!(matches!(
        interpret_terminal_artifact(&semantic, &foreign_proof, &AdmissionProfile::default(), &[],),
        Err(TerminalArtifactInterpretError::ProofDecode(_))
    ));
}

#[test]
fn forged_module_rejects_against_the_sealed_proof() {
    // Renaming the only machine keeps the forged module representation-valid
    // and independently verifiable, so the only thing stopping replay is the
    // evidence seal naming the original subject.
    let mut forged = unit_module();
    forged.machines[0].id = machine_id(2);
    forged.entry = machine_id(2);
    let (forged_semantic, forged_proof) = encoded(&forged);
    let (_semantic, proof) = encoded(&unit_module());
    assert!(matches!(
        interpret_terminal_artifact(&forged_semantic, &proof, &AdmissionProfile::default(), &[],),
        Err(TerminalArtifactInterpretError::ProofDecode(_))
    ));
    assert!(
        interpret_terminal_artifact(
            &forged_semantic,
            &forged_proof,
            &AdmissionProfile::default(),
            &[],
        )
        .is_ok(),
        "the same module carrying honestly sealed evidence still interprets"
    );
}

#[test]
fn mutated_section_bytes_reject_before_any_interpretation() {
    let (semantic, proof) = encoded(&unit_module());
    for name in ["semantic", "proof"] {
        let mut mutated_semantic = semantic.clone();
        let mut mutated_proof = proof.clone();
        match name {
            "semantic" => {
                let last = mutated_semantic.len() - 1;
                mutated_semantic[last] ^= 0xFF;
            }
            _ => {
                let last = mutated_proof.len() - 1;
                mutated_proof[last] ^= 0xFF;
            }
        }
        let outcome = interpret_terminal_artifact(
            &mutated_semantic,
            &mutated_proof,
            &AdmissionProfile::default(),
            &[],
        );
        assert!(
            matches!(outcome, Err(ref error) if is_entry_stage(error)),
            "mutated {name} section must reject before interpretation: {outcome:?}"
        );
    }
}

#[test]
fn serialized_envelope_entry_rejects_noncanonical_and_truncated_bytes() {
    let bytes = envelope(&unit_module());
    assert!(
        interpret_serialized_terminal_artifact_measured(
            &bytes,
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs::default(),
            &mut AcceptTerminalEffects,
        )
        .is_ok(),
        "a canonical envelope interprets"
    );

    let (semantic, proof) = encoded(&unit_module());
    let mut bare_sections = semantic.clone();
    bare_sections.extend_from_slice(&proof);
    for (name, hostile) in [
        ("concatenated bare sections", bare_sections),
        ("truncated envelope", bytes[..bytes.len() - 1].to_vec()),
        ("corrupted envelope magic", {
            let mut forged = bytes.clone();
            forged[0] ^= 0xFF;
            forged
        }),
    ] {
        assert!(
            matches!(
                interpret_serialized_terminal_artifact_measured(
                    &hostile,
                    &AdmissionProfile::default(),
                    &[],
                    TerminalStructuralInputs::default(),
                    &mut AcceptTerminalEffects,
                ),
                Err(TerminalArtifactInterpretError::ArtifactDecode(_))
            ),
            "{name} must reject at artifact decode before interpretation"
        );
    }
}

#[test]
fn provider_installation_rejects_evidence_sealed_for_another_subject() {
    let (semantic, _proof) = encoded(&unit_module());
    let (_foreign_semantic, foreign_proof) = encoded(&payloadless_call_module());
    assert!(matches!(
        admit_provider_installation_from_artifact(
            &semantic,
            &foreign_proof,
            &AdmissionProfile::default(),
            &[],
        ),
        Err(terminal_interpreter::ProviderInstallationError::ProofDecode(_))
    ));
}
