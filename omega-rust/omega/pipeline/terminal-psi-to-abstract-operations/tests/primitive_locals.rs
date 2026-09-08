//! Valid primitive storage must reject until native referent realization exists.

use proof_admission::AdmissionProfile;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi::OperationKind;
use terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, LoweringError, lower_artifact_sections,
    lower_artifact_sections_for_native_realization, lower_artifact_sections_for_optimization,
};

#[test]
fn borrowed_primitive_local_rejects_at_every_native_entrance() {
    let source = r#"
        machine reset(value: &mut u64) -> u64 { value = 0; 7 }
        machine enter(value: &mut u64) {
            let mut scratch: u64 = 91;
            let returned: u64 = reset(&mut scratch);
            value = scratch;
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "enter")
        .expect("primitive local Terminal producer");
    let establishment = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::EstablishPrimitiveLocal { .. }
            )
        })
        .expect("real primitive local establishment")
        .id;
    let semantic_bytes = encode_module(&lowered.semantic_module).expect("canonical semantics");
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("canonical proof");
    let profile = AdmissionProfile::default();
    terminal_verifier::verify_module(&lowered.semantic_module, &lowered.proof_bundle, &profile)
        .expect("valid before native rejection");
    for result in [
        lower_artifact_sections(&semantic_bytes, &proof_bytes, &profile).map(|_| ()),
        lower_artifact_sections_for_optimization(&semantic_bytes, &proof_bytes, &profile)
            .map(|_| ()),
        lower_artifact_sections_for_native_realization(&semantic_bytes, &proof_bytes, &profile)
            .map(|_| ()),
    ] {
        assert!(
            matches!(result, Err(ArtifactLoweringError::Lowering(
            LoweringError::UnsupportedPrimitiveLocalEstablishment(operation)
        )) if operation == establishment),
            "native rejection: {result:?}"
        );
    }
}
