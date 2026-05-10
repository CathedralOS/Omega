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
        (ExpressionNode::Mutable(left), _) => {
            expression_place_eq_across_tables(left_table, *left, right_table, right)
        }
        (_, ExpressionNode::Mutable(right)) => {
            expression_place_eq_across_tables(left_table, left, right_table, *right)
        }
        _ => expression_eq_across_tables(left_table, left, right_table, right),
    }
}

pub fn expression_name_with_suffix_eq_in_table(
    table: &ExpressionTable,
    base: ExpressionHandle,
    place: ExpressionHandle,
    suffix: &str,
) -> bool {
    let ExpressionNode::Name(base_path) = table.expression(base) else {
        return false;
    };
    let ExpressionNode::Name(place_path) = table.expression(place) else {
        return false;
    };

    table_name_path_with_suffix_eq(table, base_path, place_path, suffix)
}

pub fn expression_name_with_suffix_eq_tree(
    table: &ExpressionTable,
    base: ExpressionHandle,
    place: &Expression,
    suffix: &str,
) -> bool {
    let ExpressionNode::Name(base_path) = table.expression(base) else {
        return false;
    };
    let Expression::Name(place_path) = place else {
        return false;
    };

    table_name_path_with_tree_suffix_eq(table, base_path, place_path, suffix)
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

fn table_name_path_with_tree_suffix_eq(
    table: &ExpressionTable,
    base: &TableNamePath,
    place: &NamePath,
    suffix: &str,
) -> bool {
    let base_members = table.name_path_members(base.members);
    if place.len() != base_members.len().saturating_add(1) {
        return false;
    }

    if base.head_symbol.is_valid()
        && place.head_symbol().is_valid()
        && base.head_symbol != place.head_symbol()
    {
        return false;
    }

    base_members
        .iter()
        .zip(place.iter())
        .all(|(left, right)| left == right)
        && place.last().is_some_and(|member| member.as_str() == suffix)
}

fn table_name_path_with_suffix_eq(
    table: &ExpressionTable,
    base: &TableNamePath,
    place: &TableNamePath,
    suffix: &str,
) -> bool {
    let base_members = table.name_path_members(base.members);
    let place_members = table.name_path_members(place.members);
    if place_members.len() != base_members.len().saturating_add(1) {
        return false;
    }

    if base.head_symbol.is_valid()
        && place.head_symbol.is_valid()
        && base.head_symbol != place.head_symbol
    {
        return false;
    }

    base_members
        .iter()
        .zip(place_members.iter())
        .all(|(left, right)| left == right)
        && place_members
            .last()
            .is_some_and(|member| member.as_str() == suffix)
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
        (ExpressionNode::Mutable(left), ExpressionNode::Mutable(right)) => {
            expression_eq_across_tables(left_table, *left, right_table, *right)
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
