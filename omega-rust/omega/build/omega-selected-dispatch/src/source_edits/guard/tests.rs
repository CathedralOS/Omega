use super::*;
use psi_symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use psi_typed_trees::expression::{
    ExpressionNode, TableNamePath, TableUnaryExpression, UnaryOperator,
};
use psi_typed_trees::name::Identifier;
use psi_typed_trees::signature::StateParameter;
use psi_typed_trees::types::TypeReferenceNode;

#[test]
fn nested_expression_and_argument_span_mutations_are_detected() {
    let mut program = TypedTrees::default();
    let first = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let second = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let arguments = program.expression_table.insert_expression_handles([first]);
    let root = program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(arguments));
    let guard = GraphGuard::capture(&program, &[root], &[], &[], &[]).unwrap();
    guard.validate(&program).unwrap();
    *program.expression_table.expression_mut(first) = ExpressionNode::Boolean(true);
    assert!(guard.validate(&program).is_err());
    *program.expression_table.expression_mut(first) = ExpressionNode::Boolean(false);
    program
        .expression_table
        .set_expression_handle_at_offset(arguments, 0, second);
    assert!(guard.validate(&program).is_err());
}

#[test]
fn same_handle_operand_parameter_and_nested_type_mutations_are_detected() {
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::new();
    let symbol = symbols.insert_root(SymbolKind::Parameter, SymbolNameRef::Borrowed("operand"));
    program.symbols = symbols.finish();
    let primitive = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice {
            element_type: primitive,
        });
    let parameter = program.state_parameters.insert(StateParameter {
        symbol,
        name: Identifier::generated("operand"),
        type_reference: reference,
        ..StateParameter::default()
    });
    let root = program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            symbol,
            head_symbol: symbol,
            ..TableNamePath::default()
        }));
    let guard = GraphGuard::capture(&program, &[root], &[], &[], &[]).unwrap();
    program.type_reference_table.substitute_node(
        primitive,
        TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("u64"),
        },
    );
    assert!(guard.validate(&program).is_err());
    program
        .type_reference_table
        .substitute_node(primitive, TypeReferenceNode::Unit);
    program.state_parameters.get_mut(parameter).type_reference = primitive;
    assert!(guard.validate(&program).is_err());
}

#[test]
fn cyclic_graphs_are_finite_and_unrelated_nodes_do_not_invalidate_custody() {
    let mut program = TypedTrees::default();
    let root = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    *program.expression_table.expression_mut(root) = ExpressionNode::Unary(TableUnaryExpression {
        operator: UnaryOperator::LogicalNot,
        operand: root,
    });
    let guard = GraphGuard::capture(&program, &[root], &[], &[], &[]).unwrap();
    program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    guard.validate(&program).unwrap();
    assert_eq!(guard.expressions.len(), 1);
}

#[test]
fn stale_nonzero_handles_cannot_recover_equal_dummy_nodes() {
    let mut program = TypedTrees::default();
    let expression = program.expression_table.insert(ExpressionNode::default());
    let reference = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let expressions = GraphGuard::capture(&program, &[expression], &[], &[], &[]).unwrap();
    let types = GraphGuard::capture(&program, &[], &[reference], &[], &[]).unwrap();
    program.expression_table.clear();
    program.type_reference_table = psi_typed_trees::types::TypeReferenceTable::new();
    assert!(expressions.validate(&program).is_err());
    assert!(types.validate(&program).is_err());
}

#[test]
fn detached_original_static_arguments_keep_exact_const_binder_type_custody() {
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::new();
    let symbol = symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Borrowed("Count"));
    program.symbols = symbols.finish();
    let reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("u64"),
        });
    program
        .data_type_parameters
        .insert(psi_typed_trees::data::TypeParameter {
            symbol,
            name: Identifier::generated("Count"),
            kind: psi_typed_trees::data::TypeParameterKind::Const {
                type_reference: reference,
            },
            ..psi_typed_trees::data::TypeParameter::default()
        });
    let argument = StaticMachineArgument {
        path: Box::default(),
        application: None,
        const_literal: None,
        evidence_projection: None,
        symbol,
    };
    let guard = GraphGuard::capture(&program, &[], &[], &[], &[argument]).unwrap();
    guard.validate(&program).unwrap();
    program.type_reference_table.substitute_node(
        reference,
        TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("u32"),
        },
    );
    assert!(guard.validate(&program).is_err());
}
