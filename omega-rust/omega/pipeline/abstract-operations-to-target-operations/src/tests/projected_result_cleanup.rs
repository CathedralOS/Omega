//! Generalized continuation segments do not widen sole-return root disposal.

use super::*;

#[test]
fn sole_projected_result_return_cannot_omit_an_unrelated_affine_parameter() {
    let source = "data Token { value: u64; }
        data Pair { left: Token; right: Token; }
        data Root {} data Sink {}
        machine Root::forward(value: Pair) -> Pair { value }
        machine Sink::take(value: Token) {}
        machine Root::enter(value: Pair) {
            Sink::take(Root::forward(value).right);
        }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let terminal = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").unwrap();
    let semantic = terminal_codec::encode_module(&terminal.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&terminal.proof_bundle).unwrap();
    let mut plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    let target = NativeTarget::linux_x64();
    assert!(lower_to_target_operations(&plan, target).is_ok());
    let entry = plan
        .functions
        .iter_mut()
        .find(|function| function.machine == plan.entry)
        .unwrap();
    let mut unrelated = entry.structural_parameters[0].clone();
    unrelated.place = PlaceId::new(u64::from(u32::MAX) - 1).unwrap();
    unrelated.position = 1;
    entry.structural_parameters.push(unrelated);
    assert!(matches!(
        lower_to_target_operations(&plan, target),
        Err(LoweringError::UnsupportedOperationInUnitFunction(_))
    ));
}
