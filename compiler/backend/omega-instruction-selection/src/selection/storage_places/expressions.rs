use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, IndexedExpression,
    MemberExpression, NamePath, TableIndexedExpression, TableMemberExpression,
};
use omega_checked_trees::name::ProgramName;
use omega_core::symbols::SymbolHandle;

use super::nested_fields::FieldPathSegment;

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
            table.name_path_member_symbols(path.member_symbols),
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
            table.name_path_member_symbols(path.member_symbols),
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
    let (mut segments, head_symbol, _) = path.into_owned_segments();
    segments.push(FieldPathSegment::new(
        member.member.clone(),
        member.member_symbol,
    ));
    Some(StorageNamePath::owned(
        segments,
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
    let index = usize::try_from(*index).ok()?;
    let path = match table.expression(indexed.collection) {
        ExpressionNode::Name(path) => StorageNamePath::borrowed(
            table.name_path_members(path.members),
            table.name_path_member_symbols(path.member_symbols),
            path.head_symbol,
            path.symbol,
        ),
        ExpressionNode::Indexed(inner_indexed) => {
            indexed_expression_path_in_table(table, inner_indexed)?
        }
        _ => return None,
    };
    path.with_last_index(index)
}

pub(in crate::selection) enum StorageNamePath<'table> {
    Borrowed {
        members: &'table [ProgramName],
        member_symbols: &'table [SymbolHandle],
        head_symbol: SymbolHandle,
        final_symbol: SymbolHandle,
    },
    Owned {
        segments: Vec<FieldPathSegment>,
        head_symbol: SymbolHandle,
        final_symbol: SymbolHandle,
    },
}

impl<'table> StorageNamePath<'table> {
    fn borrowed(
        members: &'table [ProgramName],
        member_symbols: &'table [SymbolHandle],
        head_symbol: SymbolHandle,
        final_symbol: SymbolHandle,
    ) -> Self {
        Self::Borrowed {
            members,
            member_symbols,
            head_symbol,
            final_symbol,
        }
    }

    fn owned(
        segments: Vec<FieldPathSegment>,
        head_symbol: SymbolHandle,
        final_symbol: SymbolHandle,
    ) -> Self {
        Self::Owned {
            segments,
            head_symbol,
            final_symbol,
        }
    }

    pub(in crate::selection) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(in crate::selection) fn head_symbol(&self) -> SymbolHandle {
        match self {
            Self::Borrowed { head_symbol, .. } | Self::Owned { head_symbol, .. } => *head_symbol,
        }
    }

    pub(in crate::selection) fn len(&self) -> usize {
        match self {
            Self::Borrowed { members, .. } => members.len(),
            Self::Owned { segments, .. } => segments.len(),
        }
    }

    pub(in crate::selection) fn member(&self, index: usize) -> Option<&ProgramName> {
        match self {
            Self::Borrowed { members, .. } => members.get(index),
            Self::Owned { segments, .. } => segments.get(index).map(|segment| &segment.name),
        }
    }

    pub(in crate::selection) fn member_symbol(&self, index: usize) -> SymbolHandle {
        match self {
            Self::Borrowed {
                members,
                member_symbols,
                head_symbol,
                final_symbol,
                ..
            } => member_symbols.get(index).copied().unwrap_or_else(|| {
                if index == 0 {
                    *head_symbol
                } else if index + 1 == members.len() {
                    *final_symbol
                } else {
                    SymbolHandle::invalid()
                }
            }),
            Self::Owned { segments, .. } => segments
                .get(index)
                .map(|segment| segment.symbol)
                .unwrap_or_else(SymbolHandle::invalid),
        }
    }

    pub(in crate::selection) fn member_index(&self, index: usize) -> Option<usize> {
        match self {
            Self::Borrowed { .. } => None,
            Self::Owned { segments, .. } => segments.get(index).and_then(|segment| segment.index),
        }
    }

    pub(in crate::selection) fn suffix(&self, start: usize) -> StoragePathSuffix<'_, 'table> {
        match self {
            Self::Borrowed { .. } => StoragePathSuffix::Borrowed { path: self, start },
            Self::Owned { segments, .. } => StoragePathSuffix::Owned(
                segments
                    .get(start..)
                    .unwrap_or_else(|| segments.get(segments.len()..).unwrap_or(&[])),
            ),
        }
    }

    fn with_last_index(self, index: usize) -> Option<Self> {
        let (mut segments, head_symbol, final_symbol) = self.into_owned_segments();
        let last = segments.last_mut()?;
        last.index = Some(index);
        Some(Self::owned(segments, head_symbol, final_symbol))
    }

    fn into_owned_segments(self) -> (Vec<FieldPathSegment>, SymbolHandle, SymbolHandle) {
        match self {
            Self::Borrowed {
                members,
                member_symbols,
                head_symbol,
                final_symbol,
            } => {
                let segments = members
                    .iter()
                    .enumerate()
                    .map(|(index, member)| {
                        FieldPathSegment::new(
                            member.clone(),
                            borrowed_member_symbol(
                                members,
                                member_symbols,
                                head_symbol,
                                final_symbol,
                                index,
                            ),
                        )
                    })
                    .collect();
                (segments, head_symbol, final_symbol)
            }
            Self::Owned {
                segments,
                head_symbol,
                final_symbol,
            } => (segments, head_symbol, final_symbol),
        }
    }
}

fn borrowed_member_symbol(
    members: &[ProgramName],
    member_symbols: &[SymbolHandle],
    head_symbol: SymbolHandle,
    final_symbol: SymbolHandle,
    index: usize,
) -> SymbolHandle {
    member_symbols.get(index).copied().unwrap_or_else(|| {
        if index == 0 {
            head_symbol
        } else if index + 1 == members.len() {
            final_symbol
        } else {
            SymbolHandle::invalid()
        }
    })
}

#[derive(Clone, Copy)]
pub(in crate::selection) enum StoragePathSuffix<'path, 'table> {
    Borrowed {
        path: &'path StorageNamePath<'table>,
        start: usize,
    },
    Owned(&'path [FieldPathSegment]),
}

impl<'path, 'table> StoragePathSuffix<'path, 'table> {
    pub(in crate::selection) fn iter(self) -> StoragePathSuffixIter<'path, 'table> {
        match self {
            Self::Borrowed { path, start } => {
                StoragePathSuffixIter::Borrowed { path, index: start }
            }
            Self::Owned(segments) => StoragePathSuffixIter::Owned(segments.iter()),
        }
    }
}

pub(in crate::selection) enum StoragePathSuffixIter<'path, 'table> {
    Borrowed {
        path: &'path StorageNamePath<'table>,
        index: usize,
    },
    Owned(std::slice::Iter<'path, FieldPathSegment>),
}

impl<'path, 'table> Iterator for StoragePathSuffixIter<'path, 'table> {
    type Item = (&'path ProgramName, SymbolHandle, Option<usize>);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Borrowed { path, index } => {
                let member = path.member(*index)?;
                let symbol = path.member_symbol(*index);
                let field_index = path.member_index(*index);
                *index += 1;
                Some((member, symbol, field_index))
            }
            Self::Owned(segments) => segments
                .next()
                .map(|segment| (&segment.name, segment.symbol, segment.index)),
        }
    }
}
