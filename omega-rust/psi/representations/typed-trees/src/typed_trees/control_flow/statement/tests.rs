//! Typed statement tests.

use super::{StatementNode, StatementTable, TransitionTargetNode};
use crate::expression::{ExpressionNode, ExpressionTable};
use crate::name::Identifier;
use crate::types::{TypeReferenceNode, TypeReferenceTable};
use symbols::SymbolHandle;

#[test]
fn statement_table_appends_handle_native_payloads_directly() {
    let target_symbol = SymbolHandle::from_arena_index(11);
    let mut statements = StatementTable::new();
    let mut expressions = ExpressionTable::new();
    let argument = expressions.insert(crate::expression::ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(99),
    ));

    let mut arguments = arena::HandleSpan::empty();
    statements.push_expression_handle(&mut arguments, argument);

    let mut path = arena::HandleSpan::empty();
    statements.push_name_path_member(&mut path, Identifier::generated("next"));

    let target = statements.insert_transition_target(TransitionTargetNode::Named {
        static_machine_parameter: SymbolHandle::invalid(),
        path: super::TableNamePath {
            members: path,
            head_symbol: target_symbol,
            symbol: target_symbol,
        },
        arguments,
        evidence_arguments: Box::default(),
        source_span: Default::default(),
        authored_call_selection: None,
    });

    let mut state_statements = arena::HandleSpan::empty();
    let statement = statements.push_statement(
        &mut state_statements,
        StatementNode::Transition(super::TableTransition {
            target,
            continuation: super::TransitionTargetHandle::invalid(),
            guard: super::TransitionGuardNode::Always,
            proof_selectors: arena::HandleSpan::empty(),
            exit: Default::default(),
            source_span: Default::default(),
        }),
    );

    assert_eq!(state_statements.count(), 1);
    assert_eq!(statements.statement_count(), 1);
    assert_eq!(statements.transition_target_count(), 1);

    let StatementNode::Transition(transition) = statements.statement(statement) else {
        panic!("statement should be transition");
    };
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = statements.transition_target(transition.target)
    else {
        panic!("transition target should be named");
    };

    assert_eq!(path.symbol, target_symbol);
    assert_eq!(arguments.count(), 1);
    assert_eq!(statements.expression_handles(*arguments), &[argument]);
}

#[test]
fn deep_copy_owns_nested_statement_payloads() {
    let local_symbol = SymbolHandle::from_arena_index(21);
    let target_symbol = SymbolHandle::from_arena_index(22);
    let mut source_statements = StatementTable::new();
    let mut source_expressions = ExpressionTable::new();
    let mut source_types = TypeReferenceTable::new();

    let initial = source_expressions.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(7),
    ));
    let guard = source_expressions.insert(ExpressionNode::Boolean(true));
    let local_type = source_types.insert(TypeReferenceNode::Named {
        symbol: SymbolHandle::invalid(),
        name: Identifier::generated("i32"),
    });
    let mut source_span = arena::HandleSpan::empty();
    source_statements.push_statement(
        &mut source_span,
        StatementNode::LocalData(super::TableLocalData {
            symbol: local_symbol,
            name: Identifier::generated("value"),
            type_reference: local_type,
            initial_value: initial,
            is_mutable: true,
            type_is_inferred: true,
            relevance: language_core::BindingRelevance::Relevant,
        }),
    );

    let mut members = arena::HandleSpan::empty();
    source_statements.push_name_path_member(&mut members, Identifier::generated("next"));
    let mut arguments = arena::HandleSpan::empty();
    source_statements.push_expression_handle(&mut arguments, initial);
    let target = source_statements.insert_transition_target(TransitionTargetNode::Named {
        static_machine_parameter: SymbolHandle::invalid(),
        path: super::TableNamePath {
            members,
            head_symbol: target_symbol,
            symbol: target_symbol,
        },
        arguments,
        evidence_arguments: Box::default(),
        source_span: Default::default(),
        authored_call_selection: None,
    });
    source_statements.push_statement(
        &mut source_span,
        StatementNode::Transition(super::TableTransition {
            target,
            continuation: super::TransitionTargetHandle::invalid(),
            guard: super::TransitionGuardNode::When(guard),
            proof_selectors: arena::HandleSpan::empty(),
            exit: Default::default(),
            source_span: Default::default(),
        }),
    );
    let mut expression_members = arena::HandleSpan::empty();
    source_expressions
        .push_name_path_member(&mut expression_members, Identifier::generated("value"));
    let mut expression_member_symbols = arena::HandleSpan::empty();
    source_expressions.push_name_path_member_symbol(&mut expression_member_symbols, local_symbol);
    let local_reference =
        source_expressions.insert(ExpressionNode::Name(crate::expression::TableNamePath {
            members: expression_members,
            member_symbols: expression_member_symbols,
            head_symbol: local_symbol,
            symbol: local_symbol,
        }));
    source_statements.push_statement(&mut source_span, StatementNode::Expression(local_reference));
    source_statements.push_statement(
        &mut source_span,
        StatementNode::Call(super::TableCall {
            receiver_root_symbol: local_symbol,
            receiver_symbol: target_symbol,
            ..Default::default()
        }),
    );
    source_statements.push_statement(
        &mut source_span,
        StatementNode::RootBinding(super::RootBinding {
            receiver: local_reference,
            slot: Box::from([
                Identifier::generated("Target"),
                Identifier::generated("ProgramEntry"),
            ]),
            implementation: Box::from([
                Identifier::generated("Product"),
                Identifier::generated("start"),
            ]),
            implementation_operand: crate::expression::ExpressionHandle::invalid(),
            source_span: Default::default(),
        }),
    );

    let mut copied_statements = StatementTable::new();
    let mut copied_expressions = ExpressionTable::new();
    let mut copied_types = TypeReferenceTable::new();
    let copied_span = copied_statements.copy_statement_nodes_deep_from(
        &source_statements,
        &source_expressions,
        &mut copied_expressions,
        &source_types,
        &mut copied_types,
        source_span,
    );

    // Mutating the source tables after the copy cannot alter the clone.
    *source_expressions.expression_mut(initial) =
        ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(99));
    source_types.substitute_node(local_type, TypeReferenceNode::Unit);
    let remapped_local = SymbolHandle::from_arena_index(31);
    let remapped_target = SymbolHandle::from_arena_index(32);
    copied_statements.remap_symbols_in(
        copied_span,
        &mut copied_expressions,
        &mut copied_types,
        &[
            (local_symbol, remapped_local),
            (target_symbol, remapped_target),
        ],
    );

    let copied = copied_statements.statements(copied_span);
    let StatementNode::RootBinding(binding) = &copied[4] else {
        panic!("fifth copied statement should retain its declaration");
    };
    assert_eq!(
        binding
            .slot
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["Target", "ProgramEntry"]
    );
    assert_eq!(
        binding
            .implementation
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["Product", "start"]
    );
    let ExpressionNode::Name(receiver) = copied_expressions.expression(binding.receiver) else {
        panic!("root binding receiver identity");
    };
    assert_eq!(receiver.symbol, remapped_local);
    let StatementNode::Call(call) = &copied[3] else {
        panic!("fourth copied statement should retain its receiver identities");
    };
    assert_eq!(call.receiver_root_symbol, remapped_local);
    assert_eq!(call.receiver_symbol, remapped_target);
    let StatementNode::LocalData(local) = &copied[0] else {
        panic!("first copied statement should be local data");
    };
    assert_eq!(local.symbol, remapped_local);
    assert_eq!(copied_types.display_name(local.type_reference), "i32");
    assert!(local.type_is_inferred);
    assert_eq!(
        copied_expressions.expression(local.initial_value),
        &ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(7))
    );

    let StatementNode::Transition(transition) = &copied[1] else {
        panic!("second copied statement should be transition");
    };
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = copied_statements.transition_target(transition.target)
    else {
        panic!("copied transition target should be named");
    };
    assert_eq!(path.head_symbol, remapped_target);
    assert_eq!(path.symbol, remapped_target);
    assert_eq!(
        copied_statements.name_path_members(path.members)[0].as_str(),
        "next"
    );
    let copied_argument = copied_statements.expression_handles(*arguments)[0];
    assert_eq!(
        copied_expressions.expression(copied_argument),
        &ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(7))
    );
    let super::TransitionGuardNode::When(copied_guard) = transition.guard else {
        panic!("copied transition guard should be conditional");
    };
    assert!(matches!(
        copied_expressions.expression(copied_guard),
        ExpressionNode::Boolean(true)
    ));

    let StatementNode::Expression(reference) = copied[2] else {
        panic!("third copied statement should be an expression");
    };
    let ExpressionNode::Name(path) = copied_expressions.expression(reference) else {
        panic!("copied expression should be a name path");
    };
    assert_eq!(path.head_symbol, remapped_local);
    assert_eq!(path.symbol, remapped_local);
    assert_eq!(
        copied_expressions.name_path_member_symbols(path.member_symbols),
        &[remapped_local]
    );
}
