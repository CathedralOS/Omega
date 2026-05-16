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
    path.push_resolved(member.member.clone(), member.member_symbol);
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
    let last_segment = path.last()?.clone();
    path.replace_last_preserving_symbol(ProgramName::generated(format!(
        "{last_segment}[{index}]"
    )))?;
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
            path.symbol,
        )),
        _ => None,
    }
}

fn member_expression_path_in_table<'table>(
    table: &'table ExpressionTable,
    member: &TableMemberExpression,
) -> Option<StorageNamePath<'table>> {
    let path = match table.expression(member.receiver) {
        ExpressionNode::Name(path) => StorageNamePath::borrowed(
            table.name_path_members(path.members),
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
    let (mut members, mut member_symbols, head_symbol, _) = path.into_owned_parts();
    members.push(member.member.clone());
    member_symbols.push(member.member_symbol);
    Some(StorageNamePath::owned(
        members,
        member_symbols,
        head_symbol,
        member.member_symbol,
    ))
}

fn indexed_expression_path_in_table<'table>(
    table: &'table ExpressionTable,
    indexed: &TableIndexedExpression,
) -> Option<StorageNamePath<'table>> {
    let ExpressionNode::Integer(index) = table.expression(indexed.index) else {
        return None;
    };
    let path = match table.expression(indexed.collection) {
        ExpressionNode::Name(path) => StorageNamePath::borrowed(
            table.name_path_members(path.members),
            path.head_symbol,
            path.symbol,
        ),
        ExpressionNode::Indexed(inner_indexed) => {
            indexed_expression_path_in_table(table, inner_indexed)?
        }
        _ => return None,
    };
    let last_segment = path.members().last()?.clone();
    Some(
        path.replace_last_preserving_symbol(ProgramName::generated(format!(
            "{last_segment}[{index}]"
        )))?,
    )
}

pub(in crate::selection) enum StorageNamePath<'table> {
    Borrowed {
        members: &'table [ProgramName],
        head_symbol: SymbolHandle,
        final_symbol: SymbolHandle,
    },
    Owned {
        members: Vec<ProgramName>,
        member_symbols: Vec<SymbolHandle>,
        head_symbol: SymbolHandle,
        final_symbol: SymbolHandle,
    },
}

impl<'table> StorageNamePath<'table> {
    fn borrowed(
        members: &'table [ProgramName],
        head_symbol: SymbolHandle,
        final_symbol: SymbolHandle,
    ) -> Self {
        Self::Borrowed {
            members,
            head_symbol,
            final_symbol,
        }
    }

    fn owned(
        members: Vec<ProgramName>,
        member_symbols: Vec<SymbolHandle>,
        head_symbol: SymbolHandle,
        final_symbol: SymbolHandle,
    ) -> Self {
        Self::Owned {
            members,
            member_symbols,
            head_symbol,
            final_symbol,
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

    pub(in crate::selection) fn member_symbol(&self, index: usize) -> SymbolHandle {
        match self {
            Self::Borrowed {
                members,
                head_symbol,
                final_symbol,
                ..
            } => {
                if index == 0 {
                    *head_symbol
                } else if index + 1 == members.len() {
                    *final_symbol
                } else {
                    SymbolHandle::invalid()
                }
            }
            Self::Owned { member_symbols, .. } => member_symbols
                .get(index)
                .copied()
                .unwrap_or_else(SymbolHandle::invalid),
        }
    }

    fn replace_last_preserving_symbol(self, member: ProgramName) -> Option<Self> {
        let (mut members, member_symbols, head_symbol, final_symbol) = self.into_owned_parts();
        let last = members.last_mut()?;
        *last = member;
        Some(Self::owned(
            members,
            member_symbols,
            head_symbol,
            final_symbol,
        ))
    }

    fn into_owned_parts(
        self,
    ) -> (
        Vec<ProgramName>,
        Vec<SymbolHandle>,
        SymbolHandle,
        SymbolHandle,
    ) {
        match self {
            Self::Borrowed {
                members,
                head_symbol,
                final_symbol,
            } => {
                let mut member_symbols = vec![SymbolHandle::invalid(); members.len()];
                if let Some(root_symbol) = member_symbols.first_mut() {
                    *root_symbol = head_symbol;
                }
                if members.len() > 1
                    && let Some(last_symbol) = member_symbols.last_mut()
                {
                    *last_symbol = final_symbol;
                }
                (members.to_vec(), member_symbols, head_symbol, final_symbol)
            }
            Self::Owned {
                members,
                member_symbols,
                head_symbol,
                final_symbol,
            } => (members, member_symbols, head_symbol, final_symbol),
        }
    }
}
