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

macro_rules! root_item {
    ($name:ident -> $variant:ident ($binding:ident): $ty:ident ;) => {
        fn $name<'p>(
            parsed: &'p syntax_trees::SyntaxTrees,
            message: &str,
        ) -> &'p syntax_trees::item::$ty {
            parsed
                .root_items()
                .find_map(|item| match item {
                    syntax_trees::item::Item::$variant($binding) => Some($binding),
                    _ => None,
                })
                .expect(message)
        }
    };
    ($name:ident unwrap -> $variant:ident ($binding:ident): $ty:ident ;) => {
        fn $name(parsed: &syntax_trees::SyntaxTrees) -> &syntax_trees::item::$ty {
            parsed
                .root_items()
                .find_map(|item| match item {
                    syntax_trees::item::Item::$variant($binding) => Some($binding),
                    _ => None,
                })
                .unwrap()
        }
    };
}

root_item!(root_machine -> Machine(machine): Machine;);
root_item!(root_machine_unwrap unwrap -> Machine(machine): Machine;);
root_item!(root_trait -> Trait(definition): TraitDefinition;);
root_item!(root_data -> Data(definition): DataDefinition;);
root_item!(root_domain -> Domain(definition): DomainDefinition;);
root_item!(root_conformance -> Conformance(item): ConformanceItem;);
root_item!(root_const -> Const(definition): ConstDefinition;);
root_item!(root_operator -> Operator(definition): OperatorDefinition;);

/// The handle of `machine`'s first state — the entry state of the
/// single-state machines these tests parse.
fn entry_state_handle(
    parsed: &syntax_trees::SyntaxTrees,
    machine: &syntax_trees::item::Machine,
) -> syntax_trees::item::StateHandle {
    parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state")
}

/// `machine`'s first state — `entry_state_handle` resolved through the
/// item arena.
fn entry_state<'p>(
    parsed: &'p syntax_trees::SyntaxTrees,
    machine: &syntax_trees::item::Machine,
) -> &'p syntax_trees::item::State {
    parsed.items.state(entry_state_handle(parsed, machine))
}

/// The expect_err counterpart of `parse_source_verbose`: lex, then return
/// the parse error the site asserts on.
fn parse_source_error(
    source: &str,
    tokenize_message: &str,
    error_message: &str,
) -> crate::diagnostics::parse_error::ParseError {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect(tokenize_message);
    crate::parser::parse_syntax_trees(&tokens).expect_err(error_message)
}

/// Assert `error`'s message names the expected diagnostic needle.
fn expect_error(error: &crate::diagnostics::parse_error::ParseError, needle: &str) {
    assert!(
        error.message.contains(needle),
        "unexpected diagnostic: {}",
        error.message
    );
}
