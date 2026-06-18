use super::*;
use omega_core::operator_spelling::OperatorSpelling;
use omega_typed_trees::expression::{ExpressionNode, TableIndexedExpression, TableRangeExpression};
use omega_typed_trees::operator::OperatorDefinition;
use omega_typed_trees::types::TypeReferenceHandle;

#[test]
fn records_indexed_expression_operator_spelling_resolution() {
    let index_operator_symbol = SymbolHandle::from_arena_index(80);
    let range_operator_symbol = SymbolHandle::from_arena_index(81);

    let mut program = omega_typed_trees::TypedTrees::default();
    program.push_operator(operator_with_spelling(
        index_operator_symbol,
        OperatorSpelling::Index,
    ));
    program.push_operator(operator_with_spelling(
        range_operator_symbol,
        OperatorSpelling::Range,
    ));

    let collection = program.expression_table.insert(ExpressionNode::Integer(0));
    let index = program.expression_table.insert(ExpressionNode::Integer(0));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }));

    let range_start = program.expression_table.insert(ExpressionNode::Integer(0));
    let range_end = program.expression_table.insert(ExpressionNode::Integer(1));
    let range = program
        .expression_table
        .insert(ExpressionNode::Range(TableRangeExpression {
            start: range_start,
            end: range_end,
            end_inclusive: false,
        }));
    let ranged = program
        .expression_table
        .insert(ExpressionNode::Indexed(TableIndexedExpression {
            collection,
            index: range,
        }));

    let values = checked_values_for([indexed, ranged]);

    let facts = build_operator_facts(&program, &values);
    let indexed_use = facts.expression_use(indexed).expect("indexed use");
    let ranged_use = facts.expression_use(ranged).expect("ranged use");

    assert_eq!(indexed_use.spelling, OperatorSpelling::Index);
    assert_eq!(indexed_use.selected_operator_symbol, index_operator_symbol);
    assert_eq!(
        facts.candidate_symbols(indexed_use).collect::<Vec<_>>(),
        vec![index_operator_symbol]
    );
    assert!(!facts.candidates(indexed_use)[0].is_domain_owned());
    assert_eq!(
        indexed_use.status,
        omega_checked_trees::CheckedOperatorResolutionStatus::Resolved
    );
    assert_eq!(ranged_use.spelling, OperatorSpelling::Range);
    assert_eq!(ranged_use.selected_operator_symbol, range_operator_symbol);
    assert_eq!(
        facts.candidate_symbols(ranged_use).collect::<Vec<_>>(),
        vec![range_operator_symbol]
    );
    assert_eq!(
        ranged_use.status,
        omega_checked_trees::CheckedOperatorResolutionStatus::Resolved
    );
}

#[test]
fn records_ambiguous_operator_spelling_status() {
    let first_candidate = SymbolHandle::from_arena_index(90);
    let second_candidate = SymbolHandle::from_arena_index(91);

    let mut program = omega_typed_trees::TypedTrees::default();
    program.push_operator(operator_with_spelling(
        first_candidate,
        OperatorSpelling::Index,
    ));
    program.push_operator(operator_with_spelling(
        second_candidate,
        OperatorSpelling::Index,
    ));

    let collection = program.expression_table.insert(ExpressionNode::Integer(0));
    let index = program.expression_table.insert(ExpressionNode::Integer(0));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }));

    let values = checked_values_for([indexed]);
    let facts = build_operator_facts(&program, &values);
    let indexed_use = facts.expression_use(indexed).expect("indexed use");

    assert_eq!(indexed_use.spelling, OperatorSpelling::Index);
    assert_eq!(
        indexed_use.status,
        omega_checked_trees::CheckedOperatorResolutionStatus::Ambiguous
    );
    assert_eq!(indexed_use.candidate_count, 2);
    assert_eq!(
        facts.candidate_symbols(indexed_use).collect::<Vec<_>>(),
        vec![first_candidate, second_candidate]
    );
    assert!(!indexed_use.selected_operator_symbol.is_valid());
}

#[test]
fn records_domain_owned_operator_candidates() {
    let domain_symbol = SymbolHandle::from_arena_index(100);
    let domain_operator_symbol = SymbolHandle::from_arena_index(101);

    let mut program = omega_typed_trees::TypedTrees::default();
    let mut domain = omega_typed_trees::domain::DomainDefinition {
        symbol: domain_symbol,
        ..Default::default()
    };
    program.push_domain_operator(
        &mut domain,
        operator_with_spelling(domain_operator_symbol, OperatorSpelling::Index),
    );
    program.push_domain_definition(domain);

    let collection = program.expression_table.insert(ExpressionNode::Integer(0));
    let index = program.expression_table.insert(ExpressionNode::Integer(0));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }));

    let values = checked_values_for([indexed]);
    let facts = build_operator_facts(&program, &values);
    let indexed_use = facts.expression_use(indexed).expect("indexed use");
    let candidates = facts.candidates(indexed_use);

    assert_eq!(indexed_use.selected_operator_symbol, domain_operator_symbol);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].operator_symbol, domain_operator_symbol);
    assert_eq!(candidates[0].domain_symbol, domain_symbol);
    assert!(candidates[0].is_domain_owned());
}

#[test]
fn records_operator_contract_span_for_proof_bridge() {
    let operator_symbol = SymbolHandle::from_arena_index(105);

    let mut program = omega_typed_trees::TypedTrees::default();
    let mut operator = operator_with_spelling(operator_symbol, OperatorSpelling::Index);
    program.push_operator_contract(
        &mut operator,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: HandleSpan::empty(),
            token_count: 1,
        },
    );
    let operator_contracts = operator.contracts;
    program.push_operator(operator);

    let collection = program.expression_table.insert(ExpressionNode::Integer(0));
    let index = program.expression_table.insert(ExpressionNode::Integer(0));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }));

    let values = checked_values_for([indexed]);
    let facts = build_operator_facts(&program, &values);
    let indexed_use = facts.expression_use(indexed).expect("indexed use");
    let candidate = facts.candidates(indexed_use)[0];

    assert_eq!(indexed_use.selected_operator_symbol, operator_symbol);
    assert_eq!(candidate.contracts, operator_contracts);
    assert_eq!(candidate.contract_count, 1);
    let contract_uses = facts.resolved_contract_uses().collect::<Vec<_>>();
    assert_eq!(contract_uses.len(), 1);
    assert_eq!(contract_uses[0].operator_symbol(), operator_symbol);
    assert_eq!(contract_uses[0].contracts(), operator_contracts);
}

#[test]
fn records_operator_uses_per_semantic_origin() {
    let operator_symbol = SymbolHandle::from_arena_index(110);
    let first_state = SymbolHandle::from_arena_index(111);
    let second_state = SymbolHandle::from_arena_index(112);

    let mut program = omega_typed_trees::TypedTrees::default();
    program.push_operator(operator_with_spelling(
        operator_symbol,
        OperatorSpelling::Index,
    ));

    let collection = program.expression_table.insert(ExpressionNode::Integer(0));
    let index = program.expression_table.insert(ExpressionNode::Integer(0));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }));
    let first_origin = omega_checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol: SymbolHandle::from_arena_index(113),
        state_symbol: first_state,
        statement_index: 0,
        role: omega_checked_trees::CheckedValueStatementRole::Expression,
    };
    let second_origin = omega_checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol: SymbolHandle::from_arena_index(113),
        state_symbol: second_state,
        statement_index: 0,
        role: omega_checked_trees::CheckedValueStatementRole::Expression,
    };
    let mut value_roots = omega_core::arena::Arena::with_capacity(2);
    value_roots.append(omega_checked_trees::CheckedValueFact {
        expression: indexed,
        origin: first_origin,
    });
    value_roots.append(omega_checked_trees::CheckedValueFact {
        expression: indexed,
        origin: second_origin,
    });
    let values = omega_checked_trees::CheckedValueFacts::with_roots(value_roots);

    let facts = build_operator_facts(&program, &values);

    assert_eq!(facts.uses.len(), 2);
    assert!(
        facts
            .expression_use_in_origin(indexed, first_origin)
            .is_some()
    );
    assert!(
        facts
            .expression_use_in_origin(indexed, second_origin)
            .is_some()
    );
}

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

    let mut program = omega_typed_trees::TypedTrees::default();
    let i32_type = named_type(&mut program, "i32");
    let usize_type = named_type(&mut program, "usize");
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
                is_mutable: false,
                is_relaxed: false,
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
                is_mutable: false,
                is_relaxed: false,
                lifetime: None,
            });

    let mut matching_operator =
        operator_with_spelling(matching_operator_symbol, OperatorSpelling::Index);
    program.push_operator_type_parameter(
        &mut matching_operator,
        omega_typed_trees::data::TypeParameter {
            symbol: type_parameter_symbol,
            name: Identifier::generated("T"),
            kind: omega_typed_trees::data::TypeParameterKind::Type,
            bounds: omega_typed_trees::data::DataProperties::default(),
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
    let index = program.expression_table.insert(ExpressionNode::Integer(0));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }));
    let origin = omega_checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index: 0,
        role: omega_checked_trees::CheckedValueStatementRole::Expression,
    };
    let mut value_roots = omega_core::arena::Arena::default();
    value_roots.append(omega_checked_trees::CheckedValueFact {
        expression: indexed,
        origin,
    });

    let values = omega_checked_trees::CheckedValueFacts::with_roots(value_roots);
    let facts = build_operator_facts(&program, &values);
    let indexed_use = facts
        .expression_use_in_origin(indexed, origin)
        .expect("indexed use");

    assert_eq!(
        indexed_use.status,
        omega_checked_trees::CheckedOperatorResolutionStatus::Resolved
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

#[test]
fn narrows_index_operator_candidates_by_local_receiver_type() {
    let matching_operator_symbol = SymbolHandle::from_arena_index(130);
    let mismatched_operator_symbol = SymbolHandle::from_arena_index(131);
    let machine_symbol = SymbolHandle::from_arena_index(132);
    let state_symbol = SymbolHandle::from_arena_index(133);
    let local_symbol = SymbolHandle::from_arena_index(134);
    let matching_parameter_symbol = SymbolHandle::from_arena_index(135);
    let mismatched_parameter_symbol = SymbolHandle::from_arena_index(136);

    let mut program = omega_typed_trees::TypedTrees::default();
    let i32_type = named_type(&mut program, "i32");
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
                is_mutable: false,
                is_relaxed: false,
                lifetime: None,
            });

    let mut matching_operator =
        operator_with_spelling(matching_operator_symbol, OperatorSpelling::Index);
    program.push_operator_parameter(
        &mut matching_operator,
        StateParameter {
            symbol: matching_parameter_symbol,
            name: Identifier::generated("items"),
            type_reference: reference_to_slice_of_i32,
            is_const: false,
            is_mutable: false,
            is_self: false,
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
        },
    );
    program.push_operator(mismatched_operator);

    let collection = program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::generated("items")],
            local_symbol,
            local_symbol,
        )));
    let index = program.expression_table.insert(ExpressionNode::Integer(0));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }));

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
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::LocalData(omega_typed_trees::statement::TableLocalData {
            symbol: local_symbol,
            name: Identifier::generated("items"),
            type_reference: reference_to_slice_of_i32,
            initial_value: omega_typed_trees::expression::ExpressionHandle::invalid(),
        }),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(indexed),
    );
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);

    let origin = omega_checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index: 1,
        role: omega_checked_trees::CheckedValueStatementRole::Expression,
    };
    let mut value_roots = omega_core::arena::Arena::default();
    value_roots.append(omega_checked_trees::CheckedValueFact {
        expression: indexed,
        origin,
    });

    let values = omega_checked_trees::CheckedValueFacts::with_roots(value_roots);
    let facts = build_operator_facts(&program, &values);
    let indexed_use = facts
        .expression_use_in_origin(indexed, origin)
        .expect("indexed use");

    assert_eq!(
        indexed_use.status,
        omega_checked_trees::CheckedOperatorResolutionStatus::Resolved
    );
    assert_eq!(
        indexed_use.selected_operator_symbol,
        matching_operator_symbol
    );
    let candidate = facts.candidates(indexed_use)[0];
    assert_eq!(candidate.receiver_type, reference_to_slice_of_i32);
    assert_eq!(candidate.parameter_count, 1);
    assert_eq!(
        facts.candidate_symbols(indexed_use).collect::<Vec<_>>(),
        vec![matching_operator_symbol]
    );
}

fn checked_values_for(
    expressions: impl IntoIterator<Item = omega_typed_trees::expression::ExpressionHandle>,
) -> omega_checked_trees::CheckedValueFacts {
    let mut value_roots = omega_core::arena::Arena::default();
    for expression in expressions {
        value_roots.append(omega_checked_trees::CheckedValueFact {
            expression,
            origin: Default::default(),
        });
    }
    omega_checked_trees::CheckedValueFacts::with_roots(value_roots)
}

fn named_type(program: &mut omega_typed_trees::TypedTrees, name: &str) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated(name),
        })
}

fn operator_with_spelling(symbol: SymbolHandle, spelling: OperatorSpelling) -> OperatorDefinition {
    OperatorDefinition {
        is_boundary: false,
        symbol,
        name: HandleSpan::empty(),
        type_parameters: HandleSpan::empty(),
        parameters: HandleSpan::empty(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: HandleSpan::empty(),
        spelling: Some(spelling),
        token_count: 0,
    }
}
