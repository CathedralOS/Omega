use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees::item::{Item, MachineParameterContract, TypeParameterKind};
use psi_tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn trait_machine_parameter_is_a_declaration_identity_without_a_where_contract() {
    let tokens = Lexer::new("trait PrivateCallbackSlot<machine Requirement> {}")
        .tokenize()
        .expect("tokenize trait machine identity");
    let syntax = parse_syntax_trees(&tokens).expect("parse trait machine identity");
    let Item::Trait(trait_definition) = syntax.root_items().next().expect("trait root") else {
        panic!("expected trait root");
    };
    let [parameter] = syntax
        .items
        .type_parameters(trait_definition.type_parameters)
    else {
        panic!("expected one trait parameter");
    };

    assert!(matches!(
        parameter.kind,
        TypeParameterKind::Machine {
            contract: Some(MachineParameterContract::RequirementIdentity)
        }
    ));
}
