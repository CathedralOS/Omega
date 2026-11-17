/*
Unit tests for the Omega syntax parser across declarations, contracts,
expressions, inline assembly, patterns, and type constraints.
*/

use crate::errors::parse_error::ParseError;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::State;
use syntax_trees::item::StateHandle;

fn parse_source(source: &str, parse_message: &str) -> SyntaxTrees {
    parse_source_verbose(source, "tokenize should succeed", parse_message)
}

fn parse_source_verbose(source: &str, tokenize_message: &str, parse_message: &str) -> SyntaxTrees {
    let tokens = Lexer::new(source).tokenize().expect(tokenize_message);
    crate::parse_syntax_trees(&tokens).expect(parse_message)
}

fn parse_source_unwrap(source: &str) -> SyntaxTrees {
    let tokens = Lexer::new(source).tokenize().unwrap();
    crate::parse_syntax_trees(&tokens).unwrap()
}

fn argument_path<'p>(parsed: &'p SyntaxTrees, path: &'p [Identifier]) -> Vec<&'p str> {
    path.iter().map(|member| member.as_str(parsed)).collect()
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
    ($name:ident all -> $variant:ident ($binding:ident): $ty:ident ;) => {
        fn $name(parsed: &syntax_trees::SyntaxTrees) -> Vec<&syntax_trees::item::$ty> {
            parsed
                .root_items()
                .filter_map(|item| match item {
                    syntax_trees::item::Item::$variant($binding) => Some($binding),
                    _ => None,
                })
                .collect()
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
root_item!(root_machines all -> Machine(machine): Machine;);
root_item!(root_domains all -> Domain(domain): DomainDefinition;);
root_item!(root_propositions all -> Proposition(proposition): PropositionDefinition;);
root_item!(root_traits all -> Trait(definition): TraitDefinition;);
root_item!(root_data_items all -> Data(definition): DataDefinition;);
root_item!(root_operators all -> Operator(operator): OperatorDefinition;);

fn entry_state_handle(parsed: &SyntaxTrees, machine: &syntax_trees::item::Machine) -> StateHandle {
    parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state")
}

fn entry_state<'p>(parsed: &'p SyntaxTrees, machine: &syntax_trees::item::Machine) -> &'p State {
    parsed.items.state(entry_state_handle(parsed, machine))
}

fn parse_source_error(source: &str, tokenize_message: &str, error_message: &str) -> ParseError {
    let tokens = Lexer::new(source).tokenize().expect(tokenize_message);
    crate::parse_syntax_trees(&tokens).expect_err(error_message)
}

fn parse_tokenized_error(source: &str, error_message: &str) -> ParseError {
    parse_source_error(source, "tokenize should succeed", error_message)
}

fn expect_error(error: &ParseError, needle: &str) {
    assert!(
        error.message.contains(needle),
        "unexpected diagnostic: {}",
        error.message
    );
}

pub(crate) mod bindings_and_conformances;
pub(crate) mod body_sequences;
pub(crate) mod constructors;
pub(crate) mod definitions_and_contracts;
pub(crate) mod domains_and_transitions;
pub(crate) mod expression_stack;
pub(crate) mod inline_assembly;
pub(crate) mod items_and_selections;
pub(crate) mod mathematical_definitions;
pub(crate) mod numbered_data;
pub(crate) mod properties_and_requirements;
pub(crate) mod source_parsing;
pub(crate) mod static_parameters;
pub(crate) mod type_constraints;
pub(crate) mod value_dispatch;
