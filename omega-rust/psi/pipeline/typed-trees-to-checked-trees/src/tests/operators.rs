//! Fixtures shared by the operator tests: the indexed selection fixture,
//! checked programs from source and operator spellings.

mod applications;
mod crash_routes;
mod crashes;
mod destinations;
mod float_policies_and_spelling_resolution;
mod index_operator_candidate_narrowing;
mod indexed_and_domain_operator_selection;
mod invocations;
mod trait_operator_bindings;

use crate::operators::build_operator_facts;
use crate::tests::front_end::typed_program;
use crate::tests::{
    HandleSpan, Identifier, StateParameter, StatementNode, SymbolHandle, TypeReferenceNode,
};
use language_core::operator_spelling::OperatorSpelling;
use typed_trees::operator::OperatorDefinition;
use typed_trees::types::TypeReferenceHandle;

fn indexed_selection_fixture(
    source: &str,
) -> (typed_trees::TypedTrees, checked_trees::CheckedOperatorFacts) {
    let program = typed_program(source);
    let mut roots = arena::Arena::default();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                roots.append(checked_trees::CheckedValueFact {
                    expression: local.initial_value,
                    origin: checked_trees::CheckedValueOrigin::StateStatement {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        statement_index,
                        role: checked_trees::CheckedValueStatementRole::Expression,
                    },
                    ..Default::default()
                });
            }
        }
    }
    let facts = build_operator_facts(
        &program,
        &checked_trees::CheckedValueFacts::with_roots(roots),
    );
    (program, facts)
}

fn has_selected_domain_add(checked: &checked_trees::CheckedTrees) -> bool {
    checked.facts.operators.resolved_uses().any(|operator_use| {
        operator_use.spelling == OperatorSpelling::Add
            && checked
                .facts
                .operators
                .selected_candidate(operator_use)
                .is_some_and(|candidate| candidate.is_domain_owned())
    })
}

fn checked_values_for(
    expressions: impl IntoIterator<Item = typed_trees::expression::ExpressionHandle>,
) -> checked_trees::CheckedValueFacts {
    let mut value_roots = arena::Arena::default();
    for expression in expressions {
        value_roots.append(checked_trees::CheckedValueFact {
            expression,
            // These synthetic operands are statement roots. NestedExpression
            // denotes a child already traversed from such an enclosing root.
            origin: checked_trees::CheckedValueOrigin::StateStatement {
                machine_symbol: SymbolHandle::from_arena_index(1),
                state_symbol: SymbolHandle::from_arena_index(2),
                statement_index: 0,
                role: checked_trees::CheckedValueStatementRole::Expression,
            },
            ..Default::default()
        });
    }
    checked_trees::CheckedValueFacts::with_roots(value_roots)
}

fn named_type(program: &mut typed_trees::TypedTrees, name: &str) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated(name),
        })
}

fn operator_with_spelling(symbol: SymbolHandle, spelling: OperatorSpelling) -> OperatorDefinition {
    OperatorDefinition {
        is_public: false,
        is_boundary: false,
        symbol,
        name: HandleSpan::empty(),
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        parameters: HandleSpan::empty(),
        return_type: typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: HandleSpan::empty(),
        spelling: Some(spelling),
        token_count: 0,
        home_domain: SymbolHandle::invalid(),
    }
}

fn operator_with_placeholder_operands(
    program: &mut typed_trees::TypedTrees,
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
                relevance: language_core::BindingRelevance::Relevant,
            },
        );
    }
    operator
}
