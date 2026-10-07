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
    parse_source_verbose(source, "tokenize should succeed", parse_message)
}

/// `parse_source` with an explicit tokenize expectation for sites that pin
/// one.
fn parse_source_verbose(
    source: &str,
    tokenize_message: &str,
    parse_message: &str,
) -> syntax_trees::SyntaxTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect(tokenize_message);
    crate::parser::parse_syntax_trees(&tokens).expect(parse_message)
}

/// `unwrap`-flavored counterpart of `parse_source` for sites written with
/// plain unwraps.
fn parse_source_unwrap(source: &str) -> syntax_trees::SyntaxTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    crate::parser::parse_syntax_trees(&tokens).unwrap()
}

/// The first root `machine` item — the shared extraction the
/// single-machine tests use.
fn root_machine<'p>(
    parsed: &'p syntax_trees::SyntaxTrees,
    message: &str,
) -> &'p syntax_trees::item::Machine {
    parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect(message)
}

/// `unwrap`-flavored counterpart of `root_machine`.
fn root_machine_unwrap(parsed: &syntax_trees::SyntaxTrees) -> &syntax_trees::item::Machine {
    parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .unwrap()
}
