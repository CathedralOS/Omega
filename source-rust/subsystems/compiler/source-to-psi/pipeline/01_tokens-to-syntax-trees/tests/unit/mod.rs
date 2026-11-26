/*
Unit tests for the Omega syntax parser across declarations, contracts,
expressions, inline assembly, patterns, and type constraints.
*/

use crate::errors::parse_error::ParseError;
use crate::frontend_fixture::Lexer;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::State;
use syntax_trees::item::StateHandle;

/// Parses source that must tokenize and parse, naming the source when it fails.
pub(crate) fn parse_source(source: &str) -> SyntaxTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|error| panic!("tokenize {source:?}: {error:?}"));
    crate::parse_syntax_trees(&tokens).unwrap_or_else(|error| panic!("parse {source:?}: {error:?}"))
}

/// Parses source that must tokenize and then be refused by the parser.
pub(crate) fn parse_error(source: &str) -> ParseError {
    let tokens = Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|error| panic!("tokenize {source:?}: {error:?}"));
    match crate::parse_syntax_trees(&tokens) {
        Ok(_) => panic!("{source:?} must not parse"),
        Err(error) => error,
    }
}

fn dispatch(
    parsed: &SyntaxTrees,
    statement: syntax_trees::statement::StatementHandle,
) -> &syntax_trees::statement::TableDispatch {
    let syntax_trees::statement::StatementNode::Dispatch(handle) =
        parsed.statements.statement(statement)
    else {
        panic!("expected one grouped source dispatch")
    };
    parsed.statements.dispatch(*handle)
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

fn expect_error(error: &ParseError, needle: &str) {
    assert!(
        error.message.contains(needle),
        "unexpected diagnostic: {}",
        error.message
    );
}

pub(crate) mod alias_properties;
pub(crate) mod bindings_and_conformances;
pub(crate) mod body_sequences;
pub(crate) mod constructors;
pub(crate) mod definitions_and_contracts;
pub(crate) mod domains_and_transitions;
pub(crate) mod expression_stack;
pub(crate) mod inline_assembly;
pub(crate) mod items_and_selections;
pub(crate) mod mathematical_definitions;
pub(crate) mod nested_data;
pub(crate) mod numbered_data;
pub(crate) mod properties_and_requirements;
pub(crate) mod source_parsing;
pub(crate) mod static_parameters;
pub(crate) mod subjectless_guards;
pub(crate) mod type_constraints;
pub(crate) mod unannotated_constants;
pub(crate) mod value_dispatch;
