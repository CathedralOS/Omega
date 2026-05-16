use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, IndexedExpression,
    MemberExpression, NamePath, TableIndexedExpression, TableMemberExpression,
};
use omega_checked_trees::name::ProgramName;

pub(in crate::selection) fn normalized_storage_expression(
    expression: &Expression,
) -> Option<Expression> {
    match expression {
        Expression::Mutable(target) => normalized_storage_expression(target),
        Expression::Indexed(indexed) => Some(Expression::Name(indexed_expression_path(indexed)?)),
        Expression::Member(member) => Some(Expression::Name(member_expression_path(member)?)),
        Expression::Name(_) => Some(expression.clone()),
        _ => None,
    }
}

fn member_expression_path(member: &MemberExpression) -> Option<NamePath> {
    let mut path = match &member.receiver {
        Expression::Name(path) => path.clone(),
        Expression::Indexed(indexed) => indexed_expression_path(indexed)?,
        Expression::Member(inner_member) => member_expression_path(inner_member)?,
        Expression::Mutable(target) => {
            normalized_storage_expression(target).and_then(|normalized| {
                let Expression::Name(path) = normalized else {
                    return None;
                };
                Some(path)
            })?
        }
        _ => return None,
    };
    path.push(member.member.clone());
    Some(path)
}

pub(in crate::selection) fn indexed_expression_path(
    indexed: &IndexedExpression,
) -> Option<NamePath> {
    let Expression::Integer(index) = &indexed.index else {
        return None;
    };
    let mut path = match &indexed.collection {
        Expression::Name(path) => path.clone(),
        Expression::Indexed(inner_indexed) => indexed_expression_path(inner_indexed)?,
        _ => return None,
    };
    let last_segment = path.last_mut()?;
    *last_segment = ProgramName::generated(format!("{last_segment}[{index}]"));
    Some(path)
}

pub(in crate::selection) fn normalized_storage_name_path_in_table(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<NamePath> {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => normalized_storage_name_path_in_table(table, *target),
        ExpressionNode::Indexed(indexed) => indexed_expression_path_in_table(table, indexed),
        ExpressionNode::Member(member) => member_expression_path_in_table(table, member),
        ExpressionNode::Name(path) => Some(NamePath::resolved(
            table.name_path_members(path.members).to_vec(),
            path.head_symbol,
            path.symbol,
        )),
        _ => None,
    }
}

fn member_expression_path_in_table(
    table: &ExpressionTable,
    member: &TableMemberExpression,
) -> Option<NamePath> {
    let mut path = match table.expression(member.receiver) {
        ExpressionNode::Name(path) => NamePath::resolved(
            table.name_path_members(path.members).to_vec(),
            path.head_symbol,
            path.symbol,
        ),
        ExpressionNode::Indexed(indexed) => indexed_expression_path_in_table(table, indexed)?,
        ExpressionNode::Member(inner_member) => {
            member_expression_path_in_table(table, inner_member)?
        }
        ExpressionNode::Mutable(target) => normalized_storage_name_path_in_table(table, *target)?,
        _ => return None,
    };
    path.push(member.member.clone());
    Some(path)
}

fn indexed_expression_path_in_table(
    table: &ExpressionTable,
    indexed: &TableIndexedExpression,
) -> Option<NamePath> {
    let ExpressionNode::Integer(index) = table.expression(indexed.index) else {
        return None;
    };
    let mut path = match table.expression(indexed.collection) {
        ExpressionNode::Name(path) => NamePath::resolved(
            table.name_path_members(path.members).to_vec(),
            path.head_symbol,
            path.symbol,
        ),
        ExpressionNode::Indexed(inner_indexed) => {
            indexed_expression_path_in_table(table, inner_indexed)?
        }
        _ => return None,
    };
    let last_segment = path.last_mut()?;
    *last_segment = ProgramName::generated(format!("{last_segment}[{index}]"));
    Some(path)
}
