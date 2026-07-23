use super::*;
use omega_core::operator_spelling::OperatorSpelling;
use omega_typed_trees::expression::{
    BinaryOperator, ExpressionNode, TableBinaryExpression, TableIndexedExpression,
    TableRangeExpression,
};
use omega_typed_trees::operator::{OperatorDefinition, resolve_spelling_for_operands};
use omega_typed_trees::types::TypeReferenceHandle;

#[test]
fn signature_requires_selects_domain_operator_without_flow_lookup() {
    let source = r#"
        data Quantity { value: i32; }

        domain Quantity::Additive {
            self.value >= 0;
            operator add(left: Quantity, right: Quantity) -> Quantity spelling +;
        }

        data Main {}

        machine Main::combine(&self, left: Quantity, right: Quantity)
        requires
            left in Quantity::Additive
        {
            let sum: Quantity = left + right;
        }

        machine Main::main(&mut self) {}
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let combine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::combine")
        .expect("combine machine");
    let combine_state = typed
        .machine_states(combine)
        .first()
        .expect("combine state");
    assert!(
        typed
            .machine_contracts(combine)
            .iter()
            .chain(typed.state_contracts(combine_state))
            .any(|contract| {
                typed
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .any(|fact| match fact {
                        omega_typed_trees::domain::ProofFact::Membership(membership) => {
                            typed.expression_table.display_name(membership.value) == "left"
                        }
                        _ => false,
                    })
            })
    );
    let checked = lower_typed_trees(typed).expect("signature selection should resolve +");

    assert!(checked.facts.operators.resolved_uses().any(|operator_use| {
        operator_use.spelling == OperatorSpelling::Add
            && checked
                .facts
                .operators
                .selected_candidate(operator_use)
                .is_some_and(|candidate| candidate.is_domain_owned())
    }));
}

#[test]
fn semantic_only_declared_type_selects_domain_operator() {
    let source = r#"
        domain i32::Degrees {
            operator add(left: i32, right: i32) -> i32 spelling +;
        }

        data Main {}

        machine Main::rotate(
            &self,
            value: i32 in Degrees & Wrapping,
            delta: i32 in Wrapping
        ) {
            let sum: i32 in Wrapping = value + delta;
        }

        machine Main::main(&mut self) {}
    "#;

    let checked = checked_program_from_source(source);
    assert!(has_selected_domain_add(&checked));
}

#[test]
fn explicit_mint_initializer_selects_domain_operator() {
    let source = r#"
        domain i32::Degrees {
            operator add(left: i32, right: i32) -> i32 spelling +;
        }

        data Main {}

        machine Main::rotate(&self) {
            let value: i32 in Wrapping = 1 as i32 in Degrees;
            let sum: i32 in Wrapping = value + 1;
        }

        machine Main::main(&mut self) {}
    "#;

    let checked = checked_program_from_source(source);
    assert!(has_selected_domain_add(&checked));
}

#[test]
fn flow_established_membership_does_not_select_domain_operator() {
    let source = r#"
        domain i32::Degrees {
            self >= 0;
            operator add(left: i32, right: i32) -> i32 spelling +;
        }

        data Main { value: i32 in Wrapping; }

        machine Main::mark(&mut self)
        ensures
            self.value in i32::Degrees
        {
            self.value = 0;
        }

        machine Main::main(&mut self) {
            self.mark();
            let sum: i32 in Wrapping = self.value + 1;
        }
    "#;

    let checked = checked_program_from_source(source);
    assert!(!has_selected_domain_add(&checked));
    assert!(
        checked
            .facts
            .operators
            .uses_with_status(omega_checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback)
            .any(|operator_use| operator_use.spelling == OperatorSpelling::Add)
    );
}

#[test]
fn binary_resolution_matches_the_complete_operand_tuple() {
    let i32_i32_symbol = SymbolHandle::from_arena_index(140);
    let i32_u64_symbol = SymbolHandle::from_arena_index(141);
    let machine_symbol = SymbolHandle::from_arena_index(142);
    let state_symbol = SymbolHandle::from_arena_index(143);
    let left_symbol = SymbolHandle::from_arena_index(144);
    let right_symbol = SymbolHandle::from_arena_index(145);

    let mut program = omega_typed_trees::TypedTrees::default();
    let i32_type = named_type(&mut program, "i32");
    let u64_type = named_type(&mut program, "u64");
    for (operator_symbol, right_type) in [(i32_i32_symbol, i32_type), (i32_u64_symbol, u64_type)] {
        let mut operator = operator_with_spelling(operator_symbol, OperatorSpelling::Add);
        for (symbol, name, type_reference) in [
            (left_symbol, "left", i32_type),
            (right_symbol, "right", right_type),
        ] {
            program.push_operator_parameter(
                &mut operator,
                StateParameter {
                    symbol,
                    name: Identifier::generated(name),
                    type_reference,
                    is_const: false,
                    is_mutable: false,
                    is_self: false,
                },
            );
        }
        program.push_operator(operator);
    }

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
    for (symbol, name, type_reference) in [
        (left_symbol, "left", i32_type),
        (right_symbol, "right", u64_type),
    ] {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol,
                name: Identifier::generated(name),
                type_reference,
                is_const: false,
                is_mutable: false,
                is_self: false,
            },
        );
    }
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);

    let left = program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::generated("left")],
            left_symbol,
            left_symbol,
        )));
    let right = program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::generated("right")],
            right_symbol,
            right_symbol,
        )));
    let binary = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator: BinaryOperator::Add,
            right,
        }));
    let origin = omega_checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index: 0,
        role: omega_checked_trees::CheckedValueStatementRole::Expression,
    };
    let mut value_roots = omega_core::arena::Arena::default();
    value_roots.append(omega_checked_trees::CheckedValueFact {
        expression: binary,
        origin,
    });

    let facts = build_operator_facts(
        &program,
        &omega_checked_trees::CheckedValueFacts::with_roots(value_roots),
    );
    let operator_use = facts
        .expression_use_in_origin(binary, origin)
        .expect("binary operator use");
    assert_eq!(operator_use.candidate_count, 1);
    assert_eq!(operator_use.selected_operator_symbol, i32_u64_symbol);
}

#[test]
fn complete_operand_matching_shares_generic_bindings_across_positions() {
    let generic_operator_symbol = SymbolHandle::from_arena_index(146);
    let heterogeneous_operator_symbol = SymbolHandle::from_arena_index(147);
    let type_parameter_symbol = SymbolHandle::from_arena_index(148);
    let left_symbol = SymbolHandle::from_arena_index(149);
    let right_symbol = SymbolHandle::from_arena_index(150);

    let mut program = omega_typed_trees::TypedTrees::default();
    let i32_type = named_type(&mut program, "i32");
    let u64_type = named_type(&mut program, "u64");
    let type_parameter = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: type_parameter_symbol,
            name: Identifier::generated("T"),
        });

    let mut generic_operator =
        operator_with_spelling(generic_operator_symbol, OperatorSpelling::Add);
    program.push_operator_type_parameter(
        &mut generic_operator,
        omega_typed_trees::data::TypeParameter {
            symbol: type_parameter_symbol,
            name: Identifier::generated("T"),
            kind: omega_typed_trees::data::TypeParameterKind::Type,
            bounds: omega_typed_trees::data::DataProperties::default(),
        },
    );
    for (symbol, name) in [(left_symbol, "left"), (right_symbol, "right")] {
        program.push_operator_parameter(
            &mut generic_operator,
            StateParameter {
                symbol,
                name: Identifier::generated(name),
                type_reference: type_parameter,
                is_const: false,
                is_mutable: false,
                is_self: false,
            },
        );
    }
    program.push_operator(generic_operator);

    let mut heterogeneous_operator =
        operator_with_spelling(heterogeneous_operator_symbol, OperatorSpelling::Add);
    for (symbol, name, type_reference) in [
        (left_symbol, "left", i32_type),
        (right_symbol, "right", u64_type),
    ] {
        program.push_operator_parameter(
            &mut heterogeneous_operator,
            StateParameter {
                symbol,
                name: Identifier::generated(name),
                type_reference,
                is_const: false,
                is_mutable: false,
                is_self: false,
            },
        );
    }
    program.push_operator(heterogeneous_operator);

    let candidates = resolve_spelling_for_operands(
        &program,
        OperatorSpelling::Add,
        &[Some(i32_type), Some(u64_type)],
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].operator.symbol, heterogeneous_operator_symbol);
}

fn checked_program_from_source(source: &str) -> omega_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("checked lowering")
}

fn has_selected_domain_add(checked: &omega_checked_trees::CheckedTrees) -> bool {
    checked.facts.operators.resolved_uses().any(|operator_use| {
        operator_use.spelling == OperatorSpelling::Add
            && checked
                .facts
                .operators
                .selected_candidate(operator_use)
                .is_some_and(|candidate| candidate.is_domain_owned())
    })
}

#[test]
fn records_indexed_expression_operator_spelling_resolution() {
    let index_operator_symbol = SymbolHandle::from_arena_index(80);
    let range_operator_symbol = SymbolHandle::from_arena_index(81);

    let mut program = omega_typed_trees::TypedTrees::default();
    let index_operator = operator_with_placeholder_operands(
        &mut program,
        index_operator_symbol,
        OperatorSpelling::Index,
    );
    program.push_operator(index_operator);
    let range_operator = operator_with_placeholder_operands(
        &mut program,
        range_operator_symbol,
        OperatorSpelling::Range,
    );
    program.push_operator(range_operator);

    let collection = program.expression_table.insert(ExpressionNode::Integer(
        omega_core::literals::IntegerLiteral::from_value(0),
    ));
    let index = program.expression_table.insert(ExpressionNode::Integer(
        omega_core::literals::IntegerLiteral::from_value(0),
    ));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }));

    let range_start = program.expression_table.insert(ExpressionNode::Integer(
        omega_core::literals::IntegerLiteral::from_value(0),
    ));
    let range_end = program.expression_table.insert(ExpressionNode::Integer(
        omega_core::literals::IntegerLiteral::from_value(1),
    ));
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
    let first_operator =
        operator_with_placeholder_operands(&mut program, first_candidate, OperatorSpelling::Index);
    program.push_operator(first_operator);
    let second_operator =
        operator_with_placeholder_operands(&mut program, second_candidate, OperatorSpelling::Index);
    program.push_operator(second_operator);

    let collection = program.expression_table.insert(ExpressionNode::Integer(
        omega_core::literals::IntegerLiteral::from_value(0),
    ));
    let index = program.expression_table.insert(ExpressionNode::Integer(
        omega_core::literals::IntegerLiteral::from_value(0),
    ));
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
    let domain_operator = operator_with_placeholder_operands(
        &mut program,
        domain_operator_symbol,
        OperatorSpelling::Index,
    );
    program.push_domain_operator(&mut domain, domain_operator);
    program.push_domain_definition(domain);

    let collection = program.expression_table.insert(ExpressionNode::Integer(
        omega_core::literals::IntegerLiteral::from_value(0),
    ));
    let index = program.expression_table.insert(ExpressionNode::Integer(
        omega_core::literals::IntegerLiteral::from_value(0),
    ));
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

    assert_eq!(
        indexed_use.status,
        omega_checked_trees::CheckedOperatorResolutionStatus::DomainPending
    );
    assert!(!indexed_use.selected_operator_symbol.is_valid());
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].operator_symbol, domain_operator_symbol);
    assert_eq!(candidates[0].domain_symbol, domain_symbol);
    assert!(candidates[0].is_domain_owned());
}

#[test]
fn records_operator_contract_span_for_proof_bridge() {
    let operator_symbol = SymbolHandle::from_arena_index(105);

    let mut program = omega_typed_trees::TypedTrees::default();
    let mut operator =
        operator_with_placeholder_operands(&mut program, operator_symbol, OperatorSpelling::Index);
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

    let collection = program.expression_table.insert(ExpressionNode::Integer(
        omega_core::literals::IntegerLiteral::from_value(0),
    ));
    let index = program.expression_table.insert(ExpressionNode::Integer(
        omega_core::literals::IntegerLiteral::from_value(0),
    ));
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
    let operator =
        operator_with_placeholder_operands(&mut program, operator_symbol, OperatorSpelling::Index);
    program.push_operator(operator);

    let collection = program.expression_table.insert(ExpressionNode::Integer(
        omega_core::literals::IntegerLiteral::from_value(0),
    ));
    let index = program.expression_table.insert(ExpressionNode::Integer(
        omega_core::literals::IntegerLiteral::from_value(0),
    ));
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
                is_mutable: false,
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
    let index = program.expression_table.insert(ExpressionNode::Integer(
        omega_core::literals::IntegerLiteral::from_value(0),
    ));
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
fn narrows_index_operator_candidates_by_complete_operand_tuple() {
    let matching_operator_symbol = SymbolHandle::from_arena_index(130);
    let mismatched_operator_symbol = SymbolHandle::from_arena_index(131);
    let wrong_index_operator_symbol = SymbolHandle::from_arena_index(137);
    let machine_symbol = SymbolHandle::from_arena_index(132);
    let state_symbol = SymbolHandle::from_arena_index(133);
    let local_symbol = SymbolHandle::from_arena_index(134);
    let index_symbol = SymbolHandle::from_arena_index(138);
    let matching_parameter_symbol = SymbolHandle::from_arena_index(135);
    let mismatched_parameter_symbol = SymbolHandle::from_arena_index(136);

    let mut program = omega_typed_trees::TypedTrees::default();
    let i32_type = named_type(&mut program, "i32");
    let usize_type = named_type(&mut program, "u64");
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
    program.push_operator_parameter(
        &mut matching_operator,
        StateParameter {
            symbol: SymbolHandle::invalid(),
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
    program.push_operator_parameter(
        &mut mismatched_operator,
        StateParameter {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("index"),
            type_reference: usize_type,
            is_const: false,
            is_mutable: false,
            is_self: false,
        },
    );
    program.push_operator(mismatched_operator);

    let mut wrong_index_operator =
        operator_with_spelling(wrong_index_operator_symbol, OperatorSpelling::Index);
    for (name, type_reference) in [("items", reference_to_slice_of_i32), ("index", i32_type)] {
        program.push_operator_parameter(
            &mut wrong_index_operator,
            StateParameter {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated(name),
                type_reference,
                is_const: false,
                is_mutable: false,
                is_self: false,
            },
        );
    }
    program.push_operator(wrong_index_operator);

    let collection = program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::generated("items")],
            local_symbol,
            local_symbol,
        )));
    let index = program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::generated("index")],
            index_symbol,
            index_symbol,
        )));
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
            is_mutable: false,
        }),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::LocalData(omega_typed_trees::statement::TableLocalData {
            symbol: index_symbol,
            name: Identifier::generated("index"),
            type_reference: usize_type,
            initial_value: omega_typed_trees::expression::ExpressionHandle::invalid(),
            is_mutable: false,
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
        statement_index: 2,
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
    assert_eq!(candidate.parameter_count, 2);
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

fn operator_with_placeholder_operands(
    program: &mut omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
    spelling: OperatorSpelling,
) -> OperatorDefinition {
    let mut operator = operator_with_spelling(symbol, spelling);
    let operand_count = if spelling == OperatorSpelling::Range {
        3
    } else {
        2
    };
    for index in 0..operand_count {
        program.push_operator_parameter(
            &mut operator,
            StateParameter {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated(format!("operand_{index}")),
                type_reference: TypeReferenceHandle::invalid(),
                is_const: false,
                is_mutable: false,
                is_self: false,
            },
        );
    }
    operator
}
