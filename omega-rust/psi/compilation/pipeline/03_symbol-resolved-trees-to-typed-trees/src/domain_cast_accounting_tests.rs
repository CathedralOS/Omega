//! Casts retain their exact selected domain and read declaration-owned values.

use super::*;
use crate::typing_queries::{declared_callable_value_type, declared_place_type};
use std::sync::Arc;

fn source(text: &str) -> (symbol_resolved_trees::SymbolResolvedTrees, TypedTrees) {
    let mut sources = source::SourceMap::default();
    let id = sources
        .add("cast_contexts/main.omg".into(), text.to_owned())
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap();
    let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
    request.sources = Some(Arc::new(sources));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
    let typed = crate::lower_symbol_resolved_trees(&resolved).unwrap();
    (resolved, typed)
}

fn machine(typed: &TypedTrees, name: &str) -> SymbolHandle {
    typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap()
        .symbol
}

#[test]
fn machine_and_requirement_contracts_retain_each_domain_entry() {
    let (_, typed) = source(
        r#"
domain i64::Marked;
machine relay(value: i64 in Marked) -> i64 in Marked
requires (value as -Marked) != 0
ensures (result as -Marked) == (value as -Marked)
{ value as =Marked }
trait Service {
    machine relay(value: i64 in Marked) -> i64 in Marked
    requires (value as -Marked) != 0
    ensures (result as -Marked) == (value as -Marked);
}
boundary trait Raw {
    machine relay(value: i64 in Marked) -> i64 in Marked
    requires (value as -Marked) != 0
    ensures (result as -Marked) == (value as -Marked)
    reaches none crashes none;
}
"#,
    );
    let domain = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str().ends_with("Marked"))
        .unwrap();
    let casts = typed
        .expression_table
        .expression_entries()
        .filter_map(|(handle, expression)| match expression {
            ExpressionNode::Cast(cast) if cast.form.entry().is_some() => Some((handle, cast)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        casts.len() >= 10,
        "every source contract and the kept body entry are present"
    );
    for (handle, cast) in casts {
        assert!(cast.result_type.is_valid(), "{cast:?}");
        assert_eq!(cast.semantic_domain_symbol, domain.symbol);
        assert_eq!(cast.semantic_domain_id, domain.semantic_id);
        let selected = typed
            .expression_table
            .authored_selection_occurrences(handle)
            .filter_map(|occurrence| typed.authored_declaration_selections().get(occurrence))
            .filter_map(|selection| {
                selection
                    .target()
                    .as_resolved()
                    .map(|target| target.selected_symbol())
            })
            .collect::<Vec<_>>();
        assert_eq!(selected, [domain.symbol]);
    }
    assert!(
        typed.deferred_refusals.is_empty(),
        "{:?}",
        typed.deferred_refusals
    );
}

#[test]
fn kept_and_removed_entries_retain_their_selected_domain_arguments() {
    let (_, typed) = source(
        r#"
domain<Index> i64::Marked<Index>;
machine keep(value: i64 in Marked<u32>) -> i64 in Marked<u32> { value as =Marked<u32> }
machine remove(value: i64 in Marked<u32>) -> i64 { value as -Marked<u32> }
"#,
    );
    let mut count = 0;
    for (_, expression) in typed.expression_table.expression_entries() {
        let ExpressionNode::Cast(cast) = expression else {
            continue;
        };
        assert!(cast.result_type.is_valid());
        assert!(cast.semantic_domain_symbol.is_valid());
        let arguments = typed
            .type_reference_table
            .type_reference_handles(cast.semantic_domain_arguments);
        assert_eq!(arguments.len(), 1);
        assert_eq!(
            typed.primitive_type_reference(arguments[0]),
            Some(typed_trees::types::PrimitiveType::Integer(
                numerics::integer_carrier::FixedInteger::unsigned(32)
            ))
        );
        count += 1;
    }
    assert_eq!(count, 2);
}

#[test]
fn reserved_results_are_exact_callable_values_and_never_storage() {
    let (_, mut typed) = source(
        r#"
domain i64::Marked;
machine first(value: i64 in Marked) -> i64 in Marked
ensures (result as -Marked) == (value as -Marked)
{ value }
machine second(value: i64 in Marked) -> i64 in Marked { value }
machine shadow(result: i64 in Marked) -> i64 { result as -Marked }
"#,
    );
    let first = machine(&typed, "first");
    let first_state = typed.machine_states(typed.machine_by_symbol(first).unwrap())[0].clone();
    let second = machine(&typed, "second");
    let second_state = typed.machine_states(typed.machine_by_symbol(second).unwrap())[0].symbol;
    let (expression, path) = typed
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| match node {
            ExpressionNode::Name(path) if path.reserved_result_of == first_state.symbol => {
                Some((handle, *path))
            }
            _ => None,
        })
        .unwrap();
    for owner in [first, first_state.symbol] {
        assert_eq!(
            declared_callable_value_type(&typed, owner, expression).unwrap(),
            first_state.return_type
        );
    }
    assert!(declared_place_type(&typed, &first_state, expression).is_err());
    assert!(declared_callable_value_type(&typed, second, expression).is_err());
    assert!(declared_callable_value_type(&typed, SymbolHandle::invalid(), expression).is_err());
    let foreign = typed.expression_table.insert(ExpressionNode::Name(
        typed_trees::expression::TableNamePath {
            reserved_result_of: second_state,
            ..path
        },
    ));
    assert!(declared_callable_value_type(&typed, first, foreign).is_err());

    let shadow = machine(&typed, "shadow");
    let state = &typed.machine_states(typed.machine_by_symbol(shadow).unwrap())[0];
    let formal = &typed.state_parameters(state)[0];
    let ordinary = typed
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| match node {
            ExpressionNode::Name(path) if path.resolved_symbol() == Some(formal.symbol) => {
                assert!(!path.reserved_result_of.is_valid());
                Some(handle)
            }
            _ => None,
        })
        .unwrap();
    assert_ne!(formal.type_reference, state.return_type);
    assert_eq!(
        declared_callable_value_type(&typed, shadow, ordinary).unwrap(),
        formal.type_reference
    );
    assert_eq!(
        declared_place_type(&typed, state, ordinary).unwrap(),
        formal.type_reference
    );
}

#[test]
fn a_shared_cast_cannot_choose_between_two_callable_owners() {
    let (resolved, mut typed) = source(
        r#"
domain i64::Marked;
machine first(value: i64 in Marked) -> i64 { value as -Marked }
machine second(value: i64 in Marked) -> i64 { value as -Marked }
"#,
    );
    let first = machine(&typed, "first");
    let second = machine(&typed, "second");
    let first_state = typed.machine_states(typed.machine_by_symbol(first).unwrap())[0].clone();
    let second_state = typed.machine_by_symbol(second).unwrap().states.start();
    typed.machine_states.get_mut(second_state).statement_nodes = first_state.statement_nodes;
    let refused = account(&resolved, &mut typed, 0, &[]).unwrap_err();
    assert!(
        refused
            .message
            .contains("more than one exact lexical owner"),
        "{refused:?}"
    );
}

#[test]
fn a_constant_and_its_retained_initializer_share_one_source_cast_owner() {
    let (_, typed) = source(
        r#"
domain i64::Marked;
machine plain(value: i64 in Marked) -> i64 { value as -Marked }
const VALUE: i64 = plain(7 as +Marked);
"#,
    );
    let constant = typed
        .const_declarations()
        .iter()
        .find(|row| typed.symbols.name(row.symbol) == "VALUE")
        .unwrap();
    assert!(constant.evaluated_value.is_none());
    assert!(typed.tables.machines.is_valid(constant.evaluation_entry));
    let entry = typed.tables.machines.get(constant.evaluation_entry).symbol;
    let owners = typed_trees::references::collect_program_operand_owners(&typed);
    let shared = typed
        .expression_table
        .expression_entries()
        .filter_map(|(handle, node)| matches!(node, ExpressionNode::Cast(_)).then_some(handle))
        .any(|handle| {
            let owners = owners.expression_owners(handle).collect::<Vec<_>>();
            owners.contains(&constant.symbol) && owners.contains(&entry)
        });
    assert!(
        shared,
        "the actual retained initializer carries the shared source expression"
    );
    assert!(
        typed.deferred_refusals.is_empty(),
        "{:?}",
        typed.deferred_refusals
    );
}
