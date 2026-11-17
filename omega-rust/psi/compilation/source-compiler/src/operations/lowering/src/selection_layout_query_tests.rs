//! Core layout queries retain their data operand without inventing a callee.

use super::*;
use language_core::data_layout_queries::DataLayoutQueryKind;
use symbols::SymbolNameRef;
use crate::source_symbols::SymbolTableBuilder;
use crate::source_state::expression::{DataLayoutQueryRequest, StaticMachineArgument, TableCallExpression};
use crate::source_state::machine::Machine;
use crate::source_state::name::Identifier;
use crate::source_state::state::State;
use crate::source_state::statement::StatementNode;
use crate::source_state::types::TypeReferenceNode;

#[test]
fn layout_queries_keep_the_operand_and_refuse_stale_or_missing_nominal_calls() {
    for kind in [DataLayoutQueryKind::Size, DataLayoutQueryKind::Alignment] {
        let mut builder = SymbolTableBuilder::default();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Static("entry")),
                (SymbolKind::Data, SymbolNameRef::Static("Cell")),
            ],
        );
        let owner = children.handle_at(0);
        let operand = children.handle_at(1);
        let state = builder
            .insert_children(owner, [(SymbolKind::State, SymbolNameRef::Static("start"))])
            .handle_at(0);
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let reference = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: operand,
            name: Identifier::generated("Cell"),
        });
        let query = DataLayoutQueryRequest {
            kind,
            operand: StaticMachineArgument {
                symbol: operand,
                type_reference: reference,
                ..Default::default()
            },
            nonempty_view: Default::default(),
        };
        let expression = typed
            .expression_table
            .insert(ExpressionNode::Call(TableCallExpression {
                layout_query: Some(query),
                ..Default::default()
            }));
        let statements = typed
            .statement_table
            .insert_statements([StatementNode::Expression(expression)]);
        let states = typed.machine_states.insert_many([State {
            symbol: state,
            statement_nodes: statements,
            ..Default::default()
        }]);
        typed.push_machine(Machine {
            symbol: owner,
            states,
            ..Default::default()
        });
        let admitted = DeclarationSelection::from_symbols([owner]);
        let operators = CompletedOperators::default();
        let capture = |typed: &TypedTrees| super::capture(typed, &admitted, &operators, &[]);
        let captured = capture(&typed).expect("a typed core query has no nominal callee");
        assert!(
            captured
                .dependencies
                .iter()
                .any(|row| row.target == operand)
        );
        assert!(
            captured
                .dependencies
                .iter()
                .all(|row| row.target.is_valid())
        );

        // A present coordinate remains visible even on an intrinsic query.
        let ExpressionNode::Call(call) = typed.expression_table.expression_mut(expression) else {
            unreachable!()
        };
        call.target_symbol = SymbolHandle::from_parts(u32::MAX, 1);
        let refused = capture(&typed).unwrap_err();
        assert!(
            refused
                .to_string()
                .contains("missing or stale declaration coordinate"),
            "{refused}"
        );

        // Removing the intrinsic request cannot turn an unresolved ordinary
        // call into a successful declaration-free query.
        let ExpressionNode::Call(call) = typed.expression_table.expression_mut(expression) else {
            unreachable!()
        };
        call.target_symbol = SymbolHandle::invalid();
        let request = call.layout_query.take().unwrap();
        let refused = capture(&typed).unwrap_err();
        assert!(
            refused
                .to_string()
                .contains("missing or stale declaration coordinate"),
            "{refused}"
        );

        let ExpressionNode::Call(call) = typed.expression_table.expression_mut(expression) else {
            unreachable!()
        };
        call.layout_query = Some(request);
        call.layout_query.as_mut().unwrap().operand.symbol = SymbolHandle::from_parts(u32::MAX, 1);
        let refused = capture(&typed).unwrap_err();
        assert!(
            refused
                .to_string()
                .contains("missing or stale declaration coordinate"),
            "{refused}"
        );
    }
}
