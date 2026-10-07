//! Parser tests grouped by the grammar family they pin.

mod bindings_and_conformances;
mod body_sequences;
mod constructors;
mod definitions_and_contracts;
mod domains_and_transitions;
mod expression_stack;
mod inline_assembly;
mod items_and_selections;
mod mathematical_definitions;
mod numbered_data;
mod properties_and_requirements;
mod source_parsing;
mod static_parameters;
mod type_constraints;
mod value_dispatch;

/// Shared test entry: lex and parse `source`, expecting success at both
/// steps and naming the expectation when a site pins a specific parse.
fn parse_source(source: &str, parse_message: &str) -> syntax_trees::SyntaxTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    crate::parser::parse_syntax_trees(&tokens).expect(parse_message)
}
