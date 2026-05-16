use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, IndexedExpression,
    MemberExpression, NamePath, TableIndexedExpression, TableMemberExpression,
};
use omega_checked_trees::name::ProgramName;
use omega_core::symbols::SymbolHandle;

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
) -> Option<StorageNamePath<'_>> {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => normalized_storage_name_path_in_table(table, *target),
        ExpressionNode::Indexed(indexed) => indexed_expression_path_in_table(table, indexed),
        ExpressionNode::Member(member) => member_expression_path_in_table(table, member),
        ExpressionNode::Name(path) => Some(StorageNamePath::borrowed(
            table.name_path_members(path.members),
            path.head_symbol,
        )),
        _ => None,
    }
}

fn member_expression_path_in_table<'table>(
    table: &'table ExpressionTable,
    member: &TableMemberExpression,
) -> Option<StorageNamePath<'table>> {
    let path = match table.expression(member.receiver) {
        ExpressionNode::Name(path) => {
            StorageNamePath::borrowed(table.name_path_members(path.members), path.head_symbol)
        }
        ExpressionNode::Indexed(indexed) => indexed_expression_path_in_table(table, indexed)?,
        ExpressionNode::Member(inner_member) => {
            member_expression_path_in_table(table, inner_member)?
        }
        ExpressionNode::Mutable(target) => normalized_storage_name_path_in_table(table, *target)?,
        _ => return None,
    };
    let (mut members, head_symbol) = path.into_owned_parts();
    members.push(member.member.clone());
    Some(StorageNamePath::owned(members, head_symbol))
}

fn indexed_expression_path_in_table<'table>(
    table: &'table ExpressionTable,
    indexed: &TableIndexedExpression,
) -> Option<StorageNamePath<'table>> {
    let ExpressionNode::Integer(index) = table.expression(indexed.index) else {
        return None;
    };
    let path = match table.expression(indexed.collection) {
        ExpressionNode::Name(path) => {
            StorageNamePath::borrowed(table.name_path_members(path.members), path.head_symbol)
        }
        ExpressionNode::Indexed(inner_indexed) => {
            indexed_expression_path_in_table(table, inner_indexed)?
        }
        _ => return None,
    };
    let (mut members, head_symbol) = path.into_owned_parts();
    let last_segment = members.last_mut()?;
    *last_segment = ProgramName::generated(format!("{last_segment}[{index}]"));
    Some(StorageNamePath::owned(members, head_symbol))
}

pub(in crate::selection) enum StorageNamePath<'table> {
    Borrowed {
        members: &'table [ProgramName],
        head_symbol: SymbolHandle,
    },
    Owned {
        members: Vec<ProgramName>,
        head_symbol: SymbolHandle,
    },
}

impl<'table> StorageNamePath<'table> {
    fn borrowed(members: &'table [ProgramName], head_symbol: SymbolHandle) -> Self {
        Self::Borrowed {
            members,
            head_symbol,
        }
    }

    fn owned(members: Vec<ProgramName>, head_symbol: SymbolHandle) -> Self {
        Self::Owned {
            members,
            head_symbol,
        }
    }

    pub(in crate::selection) fn is_empty(&self) -> bool {
        self.members().is_empty()
    }

    pub(in crate::selection) fn head_symbol(&self) -> SymbolHandle {
        match self {
            Self::Borrowed { head_symbol, .. } | Self::Owned { head_symbol, .. } => *head_symbol,
        }
    }

    pub(in crate::selection) fn members(&self) -> &[ProgramName] {
        match self {
            Self::Borrowed { members, .. } => members,
            Self::Owned { members, .. } => members,
        }
    }

    pub(in crate::selection) fn first(&self) -> Option<&ProgramName> {
        self.members().first()
    }

    fn into_owned_parts(self) -> (Vec<ProgramName>, SymbolHandle) {
        match self {
            Self::Borrowed {
                members,
                head_symbol,
            } => (members.to_vec(), head_symbol),
            Self::Owned {
                members,
                head_symbol,
            } => (members, head_symbol),
        }
    }
}
