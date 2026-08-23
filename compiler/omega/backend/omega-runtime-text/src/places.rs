use psi_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, NamePath, TableNamePath,
};

pub fn expression_place_eq(left: &Expression, right: &Expression) -> bool {
    match (left, right) {
        (Expression::Name(left), Expression::Name(right)) => name_path_eq(left, right),
        (Expression::Indexed(left), Expression::Indexed(right)) => {
            expression_place_eq(&left.collection, &right.collection) && left.index == right.index
        }
        (Expression::Member(left), Expression::Member(right)) => {
            left.member == right.member && expression_place_eq(&left.receiver, &right.receiver)
        }
        (Expression::Borrow(left), right) => expression_place_eq(&left.target, right),
        (left, Expression::Borrow(right)) => expression_place_eq(left, &right.target),
        _ => left == right,
    }
}

pub fn expression_place_eq_in_table(
    table: &ExpressionTable,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    expression_place_eq_across_tables(table, left, table, right)
}

pub fn expression_place_eq_across_tables(
    left_table: &ExpressionTable,
    left: ExpressionHandle,
    right_table: &ExpressionTable,
    right: ExpressionHandle,
) -> bool {
    match (left_table.expression(left), right_table.expression(right)) {
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            table_name_path_eq(left_table, left, right_table, right)
        }
        (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
            expression_place_eq_across_tables(
                left_table,
                left.collection,
                right_table,
                right.collection,
            ) && expression_eq_across_tables(left_table, left.index, right_table, right.index)
        }
        (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
            left.member == right.member
                && expression_place_eq_across_tables(
                    left_table,
                    left.receiver,
                    right_table,
                    right.receiver,
                )
        }
        (ExpressionNode::Borrow(left), _) => {
            expression_place_eq_across_tables(left_table, left.target, right_table, right)
        }
        (_, ExpressionNode::Borrow(right)) => {
            expression_place_eq_across_tables(left_table, left, right_table, right.target)
        }
        _ => expression_eq_across_tables(left_table, left, right_table, right),
    }
}

pub fn expression_place_eq_table_tree(
    table: &ExpressionTable,
    left: ExpressionHandle,
    right: &Expression,
) -> bool {
    match (table.expression(left), right) {
        (ExpressionNode::Name(left), Expression::Name(right)) => {
            table_name_path_tree_eq(table, left, right)
        }
        (ExpressionNode::Indexed(left), Expression::Indexed(right)) => {
            expression_place_eq_table_tree(table, left.collection, &right.collection)
                && expression_eq_table_tree(table, left.index, &right.index)
        }
        (ExpressionNode::Member(left), Expression::Member(right)) => {
            left.member == right.member
                && expression_place_eq_table_tree(table, left.receiver, &right.receiver)
        }
        (ExpressionNode::Borrow(left), _) => {
            expression_place_eq_table_tree(table, left.target, right)
        }
        (_, Expression::Borrow(right)) => {
            expression_place_eq_table_tree(table, left, &right.target)
        }
        _ => expression_eq_table_tree(table, left, right),
    }
}

fn name_path_eq(left: &NamePath, right: &NamePath) -> bool {
    if left.len() != right.len() {
        return false;
    }

    if left.head_symbol().is_valid() && right.head_symbol().is_valid() {
        return left.head_symbol() == right.head_symbol()
            && left
                .iter()
                .skip(1)
                .zip(right.iter().skip(1))
                .all(|(left, right)| left == right);
    }

    left.iter()
        .zip(right.iter())
        .all(|(left, right)| left == right)
}

fn table_name_path_eq(
    left_table: &ExpressionTable,
    left: &TableNamePath,
    right_table: &ExpressionTable,
    right: &TableNamePath,
) -> bool {
    let left_members = left_table.name_path_members(left.members);
    let right_members = right_table.name_path_members(right.members);
    if left_members.len() != right_members.len() {
        return false;
    }

    if left.head_symbol.is_valid() && right.head_symbol.is_valid() {
        return left.head_symbol == right.head_symbol
            && left_members
                .iter()
                .skip(1)
                .zip(right_members.iter().skip(1))
                .all(|(left, right)| left == right);
    }

    left_members
        .iter()
        .zip(right_members.iter())
        .all(|(left, right)| left == right)
}

fn table_name_path_tree_eq(
    table: &ExpressionTable,
    left: &TableNamePath,
    right: &NamePath,
) -> bool {
    let left_members = table.name_path_members(left.members);
    if left_members.len() != right.len() {
        return false;
    }

    if left.head_symbol.is_valid() && right.head_symbol().is_valid() {
        return left.head_symbol == right.head_symbol()
            && left_members
                .iter()
                .skip(1)
                .zip(right.iter().skip(1))
                .all(|(left, right)| left == right);
    }

    left_members
        .iter()
        .zip(right.iter())
        .all(|(left, right)| left == right)
}

fn expression_eq_across_tables(
    left_table: &ExpressionTable,
    left: ExpressionHandle,
    right_table: &ExpressionTable,
    right: ExpressionHandle,
) -> bool {
    match (left_table.expression(left), right_table.expression(right)) {
        (ExpressionNode::ArrayLiteral(left), ExpressionNode::ArrayLiteral(right)) => {
            let left = left_table.expression_handles(*left);
            let right = right_table.expression_handles(*right);
            left.len() == right.len()
                && left.iter().zip(right.iter()).all(|(left, right)| {
                    expression_eq_across_tables(left_table, *left, right_table, *right)
                })
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && expression_eq_across_tables(left_table, left.left, right_table, right.left)
                && expression_eq_across_tables(left_table, left.right, right_table, right.right)
        }
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
        (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
            expression_eq_across_tables(left_table, left.collection, right_table, right.collection)
                && expression_eq_across_tables(left_table, left.index, right_table, right.index)
        }
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
        (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
            left.member == right.member
                && expression_eq_across_tables(
                    left_table,
                    left.receiver,
                    right_table,
                    right.receiver,
                )
        }
        (ExpressionNode::Borrow(left), ExpressionNode::Borrow(right)) => {
            left.access == right.access
                && expression_eq_across_tables(left_table, left.target, right_table, right.target)
        }
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            table_name_path_eq(left_table, left, right_table, right)
        }
        (ExpressionNode::StructLiteral(left), ExpressionNode::StructLiteral(right)) => {
            let left_fields = left_table.struct_fields(left.fields);
            let right_fields = right_table.struct_fields(right.fields);
            left.type_name == right.type_name
                && left_fields.len() == right_fields.len()
                && left_fields
                    .iter()
                    .zip(right_fields.iter())
                    .all(|(left, right)| {
                        left.name == right.name
                            && expression_eq_across_tables(
                                left_table,
                                left.value,
                                right_table,
                                right.value,
                            )
                    })
        }
        (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
        _ => false,
    }
}

fn expression_eq_table_tree(
    table: &ExpressionTable,
    left: ExpressionHandle,
    right: &Expression,
) -> bool {
    match (table.expression(left), right) {
        (ExpressionNode::ArrayLiteral(left), Expression::ArrayLiteral(right)) => {
            let left = table.expression_handles(*left);
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| expression_eq_table_tree(table, *left, right))
        }
        (ExpressionNode::Binary(left), Expression::Binary(right)) => {
            left.operator == right.operator
                && expression_eq_table_tree(table, left.left, &right.left)
                && expression_eq_table_tree(table, left.right, &right.right)
        }
        (ExpressionNode::Boolean(left), Expression::Boolean(right)) => left == right,
        (ExpressionNode::Float(left), Expression::Float(right)) => left == right,
        (ExpressionNode::Indexed(left), Expression::Indexed(right)) => {
            expression_eq_table_tree(table, left.collection, &right.collection)
                && expression_eq_table_tree(table, left.index, &right.index)
        }
        (ExpressionNode::Integer(left), Expression::Integer(right)) => left == right,
        (ExpressionNode::Member(left), Expression::Member(right)) => {
            left.member == right.member
                && expression_eq_table_tree(table, left.receiver, &right.receiver)
        }
        (ExpressionNode::Borrow(left), Expression::Borrow(right)) => {
            left.access == right.access
                && expression_eq_table_tree(table, left.target, &right.target)
        }
        (ExpressionNode::Name(left), Expression::Name(right)) => {
            table_name_path_tree_eq(table, left, right)
        }
        (ExpressionNode::StructLiteral(left), Expression::StructLiteral(right)) => {
            let left_fields = table.struct_fields(left.fields);
            left.type_name == right.type_name
                && left_fields.len() == right.fields.len()
                && left_fields
                    .iter()
                    .zip(right.fields.iter())
                    .all(|(left, right)| {
                        left.name == right.name
                            && expression_eq_table_tree(table, left.value, &right.value)
                    })
        }
        (ExpressionNode::String(left), Expression::String(right)) => left == right,
        _ => false,
    }
}
