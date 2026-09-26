use super::{named_type, operator_with_spelling};
use crate::operators::build_operator_facts;
use crate::tests::{
    Expression, Identifier, Machine, NamePath, State, StateParameter, SymbolHandle,
    TypeReferenceNode,
};
use language_core::operator_spelling::OperatorSpelling;
use typed_trees::expression::{ExpressionNode, TableIndexedExpression};

#[test]
fn narrows_index_operator_candidates_by_receiver_type() {
    let matching_operator_symbol = SymbolHandle::from_arena_index(120);
    let mismatched_operator_symbol = SymbolHandle::from_arena_index(121);
    let machine_symbol = SymbolHandle::from_arena_index(122);
    let state_symbol = SymbolHandle::from_arena_index(123);
    let items_symbol = SymbolHandle::from_arena_index(124);
    let type_parameter_symbol = SymbolHandle::from_arena_index(125);
    let index_symbol = SymbolHandle::from_arena_index(126);
    let mismatched_parameter_symbol = SymbolHandle::from_arena_index(127);

    let mut program = typed_trees::TypedTrees::default();
    let i32_type = named_type(&mut program, "i32");
    let usize_type = named_type(&mut program, "u64");
    let type_parameter = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: type_parameter_symbol,
            name: Identifier::generated("T"),
        });
    let slice_of_type_parameter = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice {
            element_type: type_parameter,
        });
    let reference_to_slice_of_type_parameter =
        program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: slice_of_type_parameter,
                access: language_core::ReferenceAccess::Shared,
                lifetime: None,
            });
    let slice_of_i32 = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice {
            element_type: i32_type,
        });
    let reference_to_slice_of_i32 =
        program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: slice_of_i32,
                access: language_core::ReferenceAccess::Shared,
                lifetime: None,
            });

    let mut matching_operator =
        operator_with_spelling(matching_operator_symbol, OperatorSpelling::Index);
    program.push_operator_type_parameter(
        &mut matching_operator,
        typed_trees::data::TypeParameter {
            symbol: type_parameter_symbol,
            name: Identifier::generated("T"),
            kind: typed_trees::data::TypeParameterKind::Type,
            bounds: typed_trees::data::DataProperties::default(),
        },
    );
    program.push_operator_parameter(
        &mut matching_operator,
        StateParameter {
            symbol: items_symbol,
            name: Identifier::generated("items"),
            type_reference: reference_to_slice_of_type_parameter,
            is_const: false,
            is_mutable: false,
            is_self: false,
            relevance: language_core::BindingRelevance::Relevant,
        },
    );
    program.push_operator_parameter(
        &mut matching_operator,
        StateParameter {
            symbol: index_symbol,
            name: Identifier::generated("index"),
            type_reference: usize_type,
            is_const: false,
            is_mutable: false,
            is_self: false,
            relevance: language_core::BindingRelevance::Relevant,
        },
    );
    program.push_operator(matching_operator);

    let mut mismatched_operator =
        operator_with_spelling(mismatched_operator_symbol, OperatorSpelling::Index);
    program.push_operator_parameter(
        &mut mismatched_operator,
        StateParameter {
            symbol: mismatched_parameter_symbol,
            name: Identifier::generated("value"),
            type_reference: i32_type,
            is_const: false,
            is_mutable: false,
            is_self: false,
            relevance: language_core::BindingRelevance::Relevant,
        },
    );
    program.push_operator(mismatched_operator);

    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Main"),
        ..Default::default()
    };
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::generated("entry"),
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: items_symbol,
            name: Identifier::generated("items"),
            type_reference: reference_to_slice_of_i32,
            is_const: false,
            is_mutable: false,
            is_self: false,
            relevance: language_core::BindingRelevance::Relevant,
        },
    );
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);

    let collection = program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::generated("items")],
            items_symbol,
            items_symbol,
        )));
    let index = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(0),
    ));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }));
    let origin = checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index: 0,
        role: checked_trees::CheckedValueStatementRole::Expression,
    };
    let mut value_roots = arena::Arena::default();
    value_roots.append(checked_trees::CheckedValueFact {
        expression: indexed,
        origin,
        ..Default::default()
    });

    let values = checked_trees::CheckedValueFacts::with_roots(value_roots);
    let facts = build_operator_facts(&program, &values);
    let indexed_use = facts
        .expression_use_in_origin(indexed, origin)
        .expect("indexed use");

    assert_eq!(
        indexed_use.status,
        checked_trees::CheckedOperatorResolutionStatus::Resolved
    );
    assert_eq!(
        indexed_use.selected_operator_symbol,
        matching_operator_symbol
    );
    assert_eq!(indexed_use.candidate_count, 1);
    let candidate = facts.candidates(indexed_use)[0];
    assert_eq!(
        candidate.receiver_type,
        reference_to_slice_of_type_parameter
    );
    assert_eq!(candidate.type_parameter_count, 1);
    assert_eq!(candidate.parameter_count, 2);
    assert_eq!(candidate.contract_count, 0);
    assert!(!candidate.is_boundary);
    assert_eq!(
        facts.candidate_symbols(indexed_use).collect::<Vec<_>>(),
        vec![matching_operator_symbol]
    );
}
