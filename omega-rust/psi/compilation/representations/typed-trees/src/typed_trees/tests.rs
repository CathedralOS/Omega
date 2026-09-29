//! Typed trees carrier tests.

use crate::state::State;
use crate::statement::{StatementNode, TableLocalData};
use crate::{
    TypedTreeRoots, TypedTrees, data, domain, machine, name::Identifier, operator,
    trait_definition, types, wire,
};
use arena::HandleSpan;
use language_core::BindingRelevance;
use symbols::SymbolHandle;

#[test]
fn typed_tree_roots_constructor_keeps_top_level_roots_explicit() {
    let data_definitions = HandleSpan::<data::DataDefinition>::default();
    let domain_definitions = HandleSpan::<domain::DomainDefinition>::default();
    let machines = HandleSpan::<machine::Machine>::default();
    let operators = HandleSpan::<operator::OperatorDefinition>::default();
    let traits = HandleSpan::<trait_definition::TraitDefinition>::default();

    let roots = TypedTreeRoots::with_roots(
        data_definitions,
        domain_definitions,
        machines,
        operators,
        traits,
    );

    assert_eq!(roots.data_definitions, data_definitions);
    assert_eq!(roots.domain_definitions, domain_definitions);
    assert_eq!(roots.machines, machines);
    assert_eq!(roots.operators, operators);
    assert_eq!(roots.traits, traits);
}

#[test]
fn signature_invokes_use_their_dedicated_arena() {
    let mut trees = TypedTrees::default();
    let mut signature_invokes = HandleSpan::empty();

    trees.signature_invokes.append_to_span(
        &mut signature_invokes,
        crate::signature::AuthoredInvocation {
            name: Identifier::generated("Console"),
            source_span: source::SourceSpan::default(),
            target: crate::signature::AuthoredInvocationTarget::Service(
                symbols::SymbolHandle::from_arena_index(7),
            ),
        },
    );

    assert_eq!(
        trees.signature_invokes.span_or_empty(signature_invokes)[0].as_str(),
        "Console"
    );
}

#[test]
fn erased_fields_do_not_contribute_to_wire_scalar_body_size() {
    let mut trees = TypedTrees::default();
    let unsupported = trees
        .type_reference_table
        .insert(types::TypeReferenceNode::Named {
            symbol: Default::default(),
            name: Identifier::generated("Unsupported"),
        });
    let members = trees.append_wire_members(vec![wire::WireMember::Field(wire::WireField {
        number: 127,
        name: Identifier::generated("proof"),
        relevance: BindingRelevance::Erased,
        type_reference: unsupported,
    })]);
    let schema = wire::WireSchema {
        name: Identifier::generated("Message"),
        members,
        ..wire::WireSchema::default()
    };

    assert_eq!(trees.wire_schema_scalar_body_worst_case(&schema), Some(0));
}

#[test]
fn state_local_declaration_answers_the_one_top_level_declaration() {
    let mut trees = TypedTrees::default();
    let first = SymbolHandle::from_arena_index(1);
    let second = SymbolHandle::from_arena_index(2);
    let repeated = SymbolHandle::from_arena_index(3);
    let absent = SymbolHandle::from_arena_index(4);
    let mut state = State::default();
    for symbol in [first, second, repeated, repeated] {
        trees.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::LocalData(TableLocalData {
                symbol,
                ..Default::default()
            }),
        );
    }

    let (ordinal, local) = trees
        .state_local_declaration(&state, second)
        .expect("second local is declared once");
    assert_eq!(ordinal, 1);
    assert_eq!(local.symbol, second);
    assert_eq!(
        trees
            .state_local_declaration(&state, first)
            .map(|(ordinal, _)| ordinal),
        Some(0)
    );
    assert!(trees.state_local_declaration(&state, absent).is_none());
    assert!(
        trees.state_local_declaration(&state, repeated).is_none(),
        "two declarations of one symbol are refused, not resolved to the first"
    );
}
