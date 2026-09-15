//! Generic substitution tests for type multiplicity.
use crate::checks::type_multiplicity;
use language_semantics::Multiplicity;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn linear_generic_bound_classifies_the_parameter_type() {
    let source = r#"
        data Main {}
        machine Main::identity<T [linear]>(value: T) -> T {
            value
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::identity")
        .expect("generic identity machine");
    let state = typed
        .machine_states(machine)
        .first()
        .expect("generic identity state");
    let parameter = typed
        .state_parameters(state)
        .iter()
        .find(|parameter| !parameter.is_self)
        .expect("linear generic value parameter");
    assert_eq!(
        type_multiplicity(&typed, parameter.type_reference),
        Multiplicity::Linear
    );
}
