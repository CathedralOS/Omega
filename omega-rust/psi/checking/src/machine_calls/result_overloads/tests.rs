//! Result overload tests.

use super::resolve_named_result_overloads;
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableCall, TableLocalData};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

fn constrained_policy(
    program: &mut TypedTrees,
    base: TypeReferenceHandle,
    policy: ArithmeticDomain,
) -> TypeReferenceHandle {
    let constraints = program
        .type_reference_table
        .insert_constraints([TypeConstraintNode::ArithmeticDomain(policy)]);
    program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: base,
            constraints,
        })
}

fn overload(
    program: &mut TypedTrees,
    machine_symbol: u32,
    entry_symbol: u32,
    parameter_type: TypeReferenceHandle,
    return_type: TypeReferenceHandle,
) -> Machine {
    let mut state = State {
        symbol: SymbolHandle::from_arena_index(entry_symbol),
        name: Identifier::generated("convert"),
        return_type,
        ..State::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: SymbolHandle::from_arena_index(entry_symbol + 100),
            name: Identifier::generated("value"),
            type_reference: parameter_type,
            ..StateParameter::default()
        },
    );
    let mut machine = Machine {
        symbol: SymbolHandle::from_arena_index(machine_symbol),
        name: Identifier::generated("I32::convert"),
        attached_data: Some(Identifier::generated("I32")),
        ..Machine::default()
    };
    program.push_machine_state(&mut machine, state);
    machine
}

fn expression_call(
    program: &mut TypedTrees,
    target_symbol: SymbolHandle,
    argument: ExpressionHandle,
) -> ExpressionHandle {
    let arguments = program
        .expression_table
        .insert_expression_handles([argument]);
    program
        .expression_table
        .insert(ExpressionNode::Call(TableCallExpression {
            receiver: ExpressionHandle::invalid(),
            target_symbol,
            target: Identifier::generated("convert"),
            static_machine_parameter: symbols::SymbolHandle::invalid(),
            static_requirement_dispatch: None,
            machine_arguments: Box::default(),
            quotient_operation: None,
            private_layout_operation: None,
            arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
        }))
}

#[test]
fn expected_result_selects_qualified_overload_and_no_expected_selects_empty() {
    let mut program = TypedTrees::default();
    let bool_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("bool"),
        });
    let i32_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("i32"),
        });
    let saturating_type = constrained_policy(&mut program, i32_type, ArithmeticDomain::Saturating);
    let unqualified = overload(&mut program, 10, 11, bool_type, i32_type);
    let saturating = overload(&mut program, 20, 21, bool_type, saturating_type);
    program.push_machine(unqualified);
    program.push_machine(saturating);

    let argument = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let qualified_call =
        expression_call(&mut program, SymbolHandle::from_arena_index(11), argument);
    let statement_arguments = program
        .statement_table
        .insert_expression_handles([argument]);
    let mut caller_state = State {
        symbol: SymbolHandle::from_arena_index(31),
        name: Identifier::generated("entry"),
        ..State::default()
    };
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::LocalData(TableLocalData {
            symbol: SymbolHandle::from_arena_index(32),
            name: Identifier::generated("converted"),
            type_reference: saturating_type,
            initial_value: qualified_call,
            is_mutable: false,
            type_is_inferred: false,
            relevance: language_core::BindingRelevance::Relevant,
        }),
    );
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::Call(TableCall {
            target_symbol: SymbolHandle::from_arena_index(21),
            target: Identifier::generated("convert"),
            arguments: statement_arguments,
            ..TableCall::default()
        }),
    );
    let caller_statements = caller_state.statement_nodes;
    let mut caller = Machine {
        symbol: SymbolHandle::from_arena_index(30),
        name: Identifier::generated("Main::run"),
        attached_data: Some(Identifier::generated("Main")),
        ..Machine::default()
    };
    program.push_machine_state(&mut caller, caller_state);
    program.push_machine(caller);

    resolve_named_result_overloads(&mut program).expect("overloads resolve");

    let ExpressionNode::Call(call) = program.expression_table.expression(qualified_call) else {
        panic!("expected call expression");
    };
    assert_eq!(call.target_symbol, SymbolHandle::from_arena_index(21));
    let StatementNode::Call(call) = &program.statement_table.statements(caller_statements)[1]
    else {
        panic!("expected statement call");
    };
    assert_eq!(call.target_symbol, SymbolHandle::from_arena_index(11));
}

#[test]
fn missing_exact_result_dispatch_set_rejects_without_rebinding() {
    let mut program = TypedTrees::default();
    let bool_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("bool"),
        });
    let i32_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("i32"),
        });
    let saturating_type = constrained_policy(&mut program, i32_type, ArithmeticDomain::Saturating);
    let wrapping_type = constrained_policy(&mut program, i32_type, ArithmeticDomain::Wrapping);
    let unqualified = overload(&mut program, 40, 41, bool_type, i32_type);
    let saturating = overload(&mut program, 50, 51, bool_type, saturating_type);
    program.push_machine(unqualified);
    program.push_machine(saturating);
    let argument = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let call = expression_call(&mut program, SymbolHandle::from_arena_index(41), argument);
    let mut caller_state = State {
        symbol: SymbolHandle::from_arena_index(61),
        name: Identifier::generated("entry"),
        ..State::default()
    };
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::LocalData(TableLocalData {
            symbol: SymbolHandle::from_arena_index(62),
            name: Identifier::generated("converted"),
            type_reference: wrapping_type,
            initial_value: call,
            is_mutable: false,
            type_is_inferred: false,
            relevance: language_core::BindingRelevance::Relevant,
        }),
    );
    let mut caller = Machine {
        symbol: SymbolHandle::from_arena_index(60),
        name: Identifier::generated("Main::run"),
        ..Machine::default()
    };
    program.push_machine_state(&mut caller, caller_state);
    program.push_machine(caller);

    let diagnostics = resolve_named_result_overloads(&mut program)
        .expect_err("Wrapping has no declared candidate");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("result dispatch set `arithmetic:Wrapping`")
    }));
}

fn module_free_overload(
    program: &mut TypedTrees,
    machine_symbol: u32,
    entry_symbol: u32,
    parameter_type: TypeReferenceHandle,
    return_type: TypeReferenceHandle,
) -> Machine {
    let mut overload = overload(
        program,
        machine_symbol,
        entry_symbol,
        parameter_type,
        return_type,
    );
    overload.name = Identifier::generated("convert");
    overload.attached_data = None;
    overload
}

#[test]
fn unbound_receiverless_call_into_duplicate_name_family_resolves_by_result_dispatch() {
    let mut program = TypedTrees::default();
    let bool_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("bool"),
        });
    let i32_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("i32"),
        });
    let saturating_type = constrained_policy(&mut program, i32_type, ArithmeticDomain::Saturating);
    // Two same-named top-level machines form one result-overload family:
    // identical path and parameter signature, distinct result dispatch sets.
    let unqualified = module_free_overload(&mut program, 10, 11, bool_type, i32_type);
    let saturating = module_free_overload(&mut program, 20, 21, bool_type, saturating_type);
    program.push_machine(unqualified);
    program.push_machine(saturating);

    let argument = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    // The ambiguous authored name binds no provisional symbol upstream.
    let qualified_call = expression_call(&mut program, SymbolHandle::invalid(), argument);
    let ordinary_call = expression_call(&mut program, SymbolHandle::invalid(), argument);
    let mut caller_state = State {
        symbol: SymbolHandle::from_arena_index(31),
        name: Identifier::generated("entry"),
        ..State::default()
    };
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::LocalData(TableLocalData {
            symbol: SymbolHandle::from_arena_index(32),
            name: Identifier::generated("qualified"),
            type_reference: saturating_type,
            initial_value: qualified_call,
            is_mutable: false,
            type_is_inferred: false,
            relevance: language_core::BindingRelevance::Relevant,
        }),
    );
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::LocalData(TableLocalData {
            symbol: SymbolHandle::from_arena_index(33),
            name: Identifier::generated("ordinary"),
            type_reference: i32_type,
            initial_value: ordinary_call,
            is_mutable: false,
            type_is_inferred: false,
            relevance: language_core::BindingRelevance::Relevant,
        }),
    );
    let mut caller = Machine {
        symbol: SymbolHandle::from_arena_index(30),
        name: Identifier::generated("Main::run"),
        ..Machine::default()
    };
    program.push_machine_state(&mut caller, caller_state);
    program.push_machine(caller);

    resolve_named_result_overloads(&mut program).expect("overload family resolves");

    let ExpressionNode::Call(call) = program.expression_table.expression(qualified_call) else {
        panic!("expected call expression");
    };
    assert_eq!(call.target_symbol, SymbolHandle::from_arena_index(21));
    let ExpressionNode::Call(call) = program.expression_table.expression(ordinary_call) else {
        panic!("expected call expression");
    };
    assert_eq!(call.target_symbol, SymbolHandle::from_arena_index(11));
}
