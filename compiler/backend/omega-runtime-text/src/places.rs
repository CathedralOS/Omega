use omega_typed_program::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, NamePath, TableNamePath,
};

pub fn expression_place_eq(left: &Expression, right: &Expression) -> bool {
    match (left, right) {
        (Expression::Name(left), Expression::Name(right)) => name_path_eq(left, right),
        (Expression::Indexed(left), Expression::Indexed(right)) => {
            expression_place_eq(&left.collection, &right.collection) && left.index == right.index
        }
        (Expression::Mutable(left), right) => expression_place_eq(left, right),
        (left, Expression::Mutable(right)) => expression_place_eq(left, right),
        _ => left == right,
    }
}

pub fn expression_place_eq_in_table(
    table: &ExpressionTable,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    match (table.expression(left), table.expression(right)) {
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            table_name_path_eq(table, left, right)
        }
        (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
            expression_place_eq_in_table(table, left.collection, right.collection)
                && expression_eq_in_table(table, left.index, right.index)
        }
        (ExpressionNode::Mutable(left), _) => expression_place_eq_in_table(table, *left, right),
        (_, ExpressionNode::Mutable(right)) => expression_place_eq_in_table(table, left, *right),
        _ => expression_eq_in_table(table, left, right),
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
    table: &ExpressionTable,
    left: &TableNamePath,
    right: &TableNamePath,
) -> bool {
    let left_members = table.name_path_members(left.members);
    let right_members = table.name_path_members(right.members);
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

fn expression_eq_in_table(
    table: &ExpressionTable,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    if left == right {
        return true;
    }

    match (table.expression(left), table.expression(right)) {
        (ExpressionNode::ArrayLiteral(left), ExpressionNode::ArrayLiteral(right)) => {
            let left = table.expression_handles(*left);
            let right = table.expression_handles(*right);
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| expression_eq_in_table(table, *left, *right))
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && expression_eq_in_table(table, left.left, right.left)
                && expression_eq_in_table(table, left.right, right.right)
        }
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
        (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
            expression_eq_in_table(table, left.collection, right.collection)
                && expression_eq_in_table(table, left.index, right.index)
        }
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
        (ExpressionNode::Mutable(left), ExpressionNode::Mutable(right)) => {
            expression_eq_in_table(table, *left, *right)
        }
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            table_name_path_eq(table, left, right)
        }
        (ExpressionNode::StructLiteral(left), ExpressionNode::StructLiteral(right)) => {
            let left_fields = table.struct_fields(left.fields);
            let right_fields = table.struct_fields(right.fields);
            left.type_name == right.type_name
                && left_fields.len() == right_fields.len()
                && left_fields
                    .iter()
                    .zip(right_fields.iter())
                    .all(|(left, right)| {
                        left.name == right.name
                            && expression_eq_in_table(table, left.value, right.value)
                    })
        }
        (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
        _ => false,
    }
}
