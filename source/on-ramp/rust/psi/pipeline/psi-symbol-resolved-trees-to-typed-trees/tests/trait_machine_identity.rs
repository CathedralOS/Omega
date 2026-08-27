use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees::data::{MachineParameterContract, TypeParameterKind};
use psi_typed_trees::types::TypeReferenceNode;

fn lower(source: &str) -> psi_typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn retains_trait_machine_identity_category_and_exact_requirement_symbol() {
    let typed = lower(
        r#"
        trait PrivateCallbackSlot<machine Requirement> {}
        boundary trait WindowProcedure {
            machine call(value: i32) -> i32;
        }
        data WndClassLayout {}
        WindowProcedureSlot: WndClassLayout satisfies PrivateCallbackSlot<WindowProcedure::call> {}
        "#,
    );

    let slot_trait = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "PrivateCallbackSlot")
        .expect("slot trait");
    let [parameter] = typed.trait_type_parameters(slot_trait) else {
        panic!("expected one slot parameter");
    };
    assert!(matches!(
        parameter.kind,
        TypeParameterKind::Machine {
            contract: MachineParameterContract::RequirementIdentity
        }
    ));

    let callback_trait = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "WindowProcedure")
        .expect("callback trait");
    let [callback_requirement] = typed.trait_machine_signatures(callback_trait) else {
        panic!("expected one callback requirement");
    };
    let conformance = typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "WindowProcedureSlot")
        })
        .expect("private slot conformance");
    let [argument] = typed
        .type_reference_table
        .type_reference_handles(conformance.arguments)
    else {
        panic!("expected one trait argument");
    };
    let TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(*argument)
    else {
        panic!("expected direct declaration identity");
    };
    assert_eq!(*symbol, callback_requirement.symbol);
    assert_eq!(
        typed.symbols.get(*symbol).kind,
        psi_symbols::SymbolKind::State
    );
}
