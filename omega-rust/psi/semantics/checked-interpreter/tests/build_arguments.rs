use checked_interpreter::{
    BuildMachineEvaluationFailure, BuildMachineEvaluationRequest, MeasuredBuildMachineEvaluation,
};
use checked_interpreter::{
    BuildTimeValue, InterpretOptions, evaluate_build_machine_arguments,
    evaluate_granted_build_machine_arguments,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn initial_reference_arguments_preserve_identity_through_helper_calls() {
    let source = "machine augment(value: &mut i32) { forward(value); }
        machine forward(value: &mut i32) { replace(value); }
        machine replace(value: &mut i32) { value = 7; }";
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolution");
    let typed = lower_symbol_resolved_trees(&resolved).expect("types");
    let checked = lower_typed_trees(typed.clone(), &CheckingRequest::settled())
        .expect("ordinary borrows check");
    drop(checked);
    let pure = evaluate_build_machine_arguments(
        &typed,
        BuildMachineEvaluationRequest::named("augment", vec![BuildTimeValue::Int(0)]),
    )
    .map(MeasuredBuildMachineEvaluation::into_value)
    .expect("pure build argument evaluation");
    let granted = evaluate_granted_build_machine_arguments(
        &typed,
        BuildMachineEvaluationRequest::named("augment", vec![BuildTimeValue::Int(0)]),
        InterpretOptions::default(),
    )
    .map(MeasuredBuildMachineEvaluation::into_value)
    .map_err(BuildMachineEvaluationFailure::into_diagnostic)
    .expect("granted build argument evaluation");
    assert_eq!(pure, vec![BuildTimeValue::Int(7)]);
    assert_eq!(granted, pure);
}
