use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees::data::{MachineParameterContract, TypeParameterKind};
use psi_symbol_resolved_trees::types::TypeReference;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn resolves_trait_requirement_argument_as_exact_state_identity() {
    let tokens = Lexer::new(
        r#"
        trait PrivateCallbackSlot<machine Requirement> {}
        boundary trait WindowProcedure {
            machine call(value: i32) -> i32;
        }
        data WndClassLayout {}
        WindowProcedureSlot: WndClassLayout satisfies PrivateCallbackSlot<WindowProcedure::call> {}
        "#,
    )
    .tokenize()
    .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let program = lower_syntax_trees(&syntax).expect("resolve");

    let slot_trait = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "PrivateCallbackSlot")
        .expect("slot trait");
    let [parameter] = program.trait_type_parameters(slot_trait) else {
        panic!("expected one slot parameter");
    };
    assert!(matches!(
        parameter.kind,
        TypeParameterKind::Machine {
            contract: MachineParameterContract::DeclarationIdentity
        }
    ));

    let callback_trait = program
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "WindowProcedure")
        .expect("callback trait");
    let [callback_requirement] = program.trait_machine_signatures(callback_trait.machines) else {
        panic!("expected one callback requirement");
    };
    let conformance = program
        .conformances
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "WindowProcedureSlot")
        })
        .expect("private slot conformance");
    let [argument] = program.child_type_references(conformance.arguments) else {
        panic!("expected one trait argument");
    };
    let TypeReference::Named { symbol, .. } = argument else {
        panic!("expected direct declaration identity");
    };
    assert_eq!(*symbol, callback_requirement.symbol);
}
