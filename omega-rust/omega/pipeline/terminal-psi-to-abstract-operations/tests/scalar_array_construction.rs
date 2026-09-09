//! Verified scalar-array construction retains an explicit native support boundary.

use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_psi::OperationKind;
use terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, LoweringError, lower_artifact_sections,
    lower_artifact_sections_for_native_realization, lower_artifact_sections_for_optimization,
};

#[test]
fn verified_scalar_array_constructors_reject_before_losing_empty_or_material_payloads() {
    for (carrier, initializer) in [("[u8; 2]", "[7, 9]"), ("[u8; 0]", "[]")] {
        let source = format!(
            "data Sizes {{}} const Sizes::ROWS: [{carrier}; 1] = [{initializer}];
             machine selected() -> {carrier} {{ Sizes::ROWS[0] }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .expect("tokenize array source");
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse array source");
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve exact array selection");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type array source");
        let checked =
            typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check array source");
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "selected")
            .expect("selected array reaches Terminal");
        let semantic = encode_module(&lowered.semantic_module).expect("encode array semantics");
        let proof = encode_proof_bundle(&lowered.proof_bundle).expect("encode array proof");
        let module = decode_module(&semantic).expect("decode array semantics");
        let decoded_proof = decode_proof_bundle(&proof).expect("decode array proof");
        let profile = AdmissionProfile::default();
        terminal_verifier::verify_module(&module, &decoded_proof, &profile)
            .expect("array construction verifies independently before native rejection");
        let constructor = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find(|operation| matches!(operation.kind, OperationKind::EstablishScalarArray { .. }))
            .expect("array constructor retains its scalar payload")
            .id;
        for result in [
            lower_artifact_sections(&semantic, &proof, &profile).map(|_| ()),
            lower_artifact_sections_for_optimization(&semantic, &proof, &profile).map(|_| ()),
            lower_artifact_sections_for_native_realization(&semantic, &proof, &profile).map(|_| ()),
        ] {
            assert!(
                matches!(result, Err(ArtifactLoweringError::Lowering(
                LoweringError::UnsupportedScalarArray(operation)
            )) if operation == constructor),
                "{result:?}"
            );
        }
    }
}
