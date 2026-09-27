//! The shared artifact admission requires a proof section sealed to the exact
//! module before callers obtain a plan, optimizer input, or native input.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{ProofCodecError, encode_module, encode_proof_bundle, encode_proof_section};
use terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, ArtifactSections, lower_artifact,
};

const SOURCE: &str = r#"
    machine reset(value: &mut u64) -> u64 { value = 0; 7 }
    machine enter(value: &mut u64) {
        let mut scratch: u64 = 91;
        let returned: u64 = reset(&mut scratch);
        value = scratch;
    }
"#;

fn checked_source() -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check")
}

fn artifact_sections<'artifact>(
    semantic_bytes: &'artifact [u8],
    proof_bytes: &'artifact [u8],
) -> ArtifactSections<'artifact> {
    ArtifactSections {
        semantic_bytes,
        proof_bytes,
        obligation_ledger_bytes: None,
    }
}

#[test]
fn artifact_admission_rejects_an_unsealed_proof_bundle() {
    let checked = checked_source();
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("enter"),
    )
    .expect("Terminal producer");
    let semantic_bytes = encode_module(&lowered.semantic_module).expect("canonical semantics");
    // A bare proof bundle still verifies in process but carries no subject
    // seal, so artifact admission must refuse it outright.
    let bare_proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("bare bundle");
    let profile = AdmissionProfile::default();
    terminal_verifier::verify_module(&lowered.semantic_module, &lowered.proof_bundle, &profile)
        .expect("the bare bundle still verifies in process");

    let result = lower_artifact(
        artifact_sections(&semantic_bytes, &bare_proof_bytes),
        &profile,
    );
    assert!(
        matches!(result, Err(ArtifactLoweringError::ProofDecode(_))),
        "artifact admission must reject an unsealed proof bundle"
    );
}

#[test]
fn artifact_admission_rejects_a_proof_section_sealed_for_another_subject() {
    let checked = checked_source();
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("enter"),
    )
    .expect("Terminal producer");
    let foreign = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("reset"),
    )
    .expect("foreign Terminal producer");
    let semantic_bytes = encode_module(&lowered.semantic_module).expect("canonical semantics");
    // The bundle verifies for both modules (compact obligation coordinates
    // coincide), but the seal names the foreign module's identity.
    let foreign_sealed_bytes =
        encode_proof_section(&foreign.semantic_module, &lowered.proof_bundle)
            .expect("foreign-sealed section");
    let profile = AdmissionProfile::default();
    terminal_verifier::verify_module(&lowered.semantic_module, &lowered.proof_bundle, &profile)
        .expect("the bundle itself verifies for this module");

    let result = lower_artifact(
        artifact_sections(&semantic_bytes, &foreign_sealed_bytes),
        &profile,
    );
    assert!(
        matches!(
            result,
            Err(ArtifactLoweringError::ProofDecode(
                ProofCodecError::ProofSubjectMismatch { .. }
            ))
        ),
        "artifact admission must reject a section sealed for another subject"
    );
}
