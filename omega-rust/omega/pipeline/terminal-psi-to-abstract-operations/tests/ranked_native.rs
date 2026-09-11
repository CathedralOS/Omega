#[path = "ranked_native/legacy_fixture.rs"]
mod legacy_fixture;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, lower_artifact_sections, lower_artifact_sections_for_native_realization,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const COUNTDOWN_SOURCE: &str = r#"
    data Token { value: i32; }
    data Root {}

    machine Root::countdown(token: Token, remaining: u32)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> countdown(token, remaining - 1)
            _ -> done(token)
        }
        state done(token: Token) {}
    }
"#;

fn artifact(source: &str) -> (Vec<u8>, Vec<u8>, terminal_psi::TerminalModule) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize fixture");
    let syntax = parse_syntax_trees(&tokens).expect("parse fixture");
    let resolved = lower_syntax_trees(&syntax).expect("resolve fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type fixture");
    let checked = lower_typed_trees(typed).expect("check fixture");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::countdown")
        .expect("lower ranked fixture");
    let semantic =
        terminal_codec::encode_module(&lowered.semantic_module).expect("encode ranked semantics");
    let proof =
        terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("encode ranked proof");
    (semantic, proof, lowered.semantic_module)
}

#[test]
fn unsigned_countdown_tag_is_explicitly_rejected_at_native_entrance() {
    let module = legacy_fixture::legacy_countdown();
    let proof = terminal_codec::encode_proof_bundle(&terminal_psi::ProofBundle::default()).unwrap();
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    assert!(matches!(
        lower_artifact_sections_for_native_realization(&semantic, &proof, &profile),
        Err(ArtifactLoweringError::UnsupportedUnsignedCountdownNativeCustody)
    ));
    assert!(lower_artifact_sections(&semantic, &proof, &profile).is_err());
}

#[test]
fn deleting_countdown_metadata_cannot_convert_a_cycle_to_ordinary_custody() {
    let (_, proof, mut module) = artifact(COUNTDOWN_SOURCE);
    for machine in &mut module.machines {
        machine.ranked_scc = None;
    }
    let semantic = terminal_codec::encode_module(&module).unwrap();
    assert!(
        lower_artifact_sections_for_native_realization(
            &semantic,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err()
    );
}
