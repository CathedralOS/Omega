use psi_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, IndexedExpression,
    MemberExpression, NamePath, TableCallExpression,
};
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;

pub(in crate::selection) fn normalized_storage_expression(
    expression: &Expression,
) -> Option<Expression> {
    match expression {
        Expression::Borrow(target) => normalized_storage_expression(&target.target),
        Expression::Indexed(indexed) => Some(Expression::Name(indexed_expression_path(indexed)?)),
        Expression::Member(member) => Some(Expression::Name(member_expression_path(member)?)),
        Expression::Call(call) => {
            normalized_storage_expression(slice_view_call_receiver_expression(call)?)
        }
        Expression::Name(_) => Some(expression.clone()),
        _ => None,
    }
}

fn member_expression_path(member: &MemberExpression) -> Option<NamePath> {
    let mut path = match &member.receiver {
        Expression::Name(path) => path.clone(),
        Expression::Indexed(indexed) => indexed_expression_path(indexed)?,
        Expression::Member(inner_member) => member_expression_path(inner_member)?,
        Expression::Call(call) => normalized_storage_expression(
            slice_view_call_receiver_expression(call)?,
        )
        .and_then(|normalized| {
            let Expression::Name(path) = normalized else {
                return None;
            };
            Some(path)
        })?,
        Expression::Borrow(target) => {
            normalized_storage_expression(&target.target).and_then(|normalized| {
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
    let mut path = match slice_view_collection_expression(&indexed.collection) {
        Expression::Name(path) => path.clone(),
        Expression::Indexed(inner_indexed) => indexed_expression_path(inner_indexed)?,
        _ => return None,
    };
    let last_segment = path.last()?.clone();
    path.replace_last_preserving_symbol(Identifier::generated(format!("{last_segment}[{index}]")))?;
    Some(path)
}

pub(in crate::selection) fn normalized_storage_name_path_in_table(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<StorageNamePath<'_>> {
    storage_path_len(table, expression).map(|_| StorageNamePath { table, expression })
}

pub(in crate::selection) struct StorageNamePath<'table> {
    table: &'table ExpressionTable,
    expression: ExpressionHandle,
}

impl<'table> StorageNamePath<'table> {
    pub(in crate::selection) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(in crate::selection) fn head_symbol(&self) -> SymbolHandle {
        storage_path_head_symbol(self.table, self.expression)
    }

    pub(in crate::selection) fn len(&self) -> usize {
        storage_path_len(self.table, self.expression).unwrap_or(0)
    }

    pub(in crate::selection) fn member(&self, index: usize) -> Option<&Identifier> {
        storage_path_member(self.table, self.expression, index)
    }

    pub(in crate::selection) fn member_symbol(&self, index: usize) -> SymbolHandle {
        storage_path_member_symbol(self.table, self.expression, index)
    }

    pub(in crate::selection) fn member_index(&self, index: usize) -> Option<usize> {
        storage_path_member_index(self.table, self.expression, index)
    }

    pub(in crate::selection) fn member_case_variant(&self, index: usize) -> Option<&Identifier> {
        storage_path_member_case_variant(self.table, self.expression, index)
    }

    pub(in crate::selection) fn suffix(&self, start: usize) -> StoragePathSuffix<'_, 'table> {
        StoragePathSuffix { path: self, start }
    }
}

fn storage_path_head_symbol(table: &ExpressionTable, expression: ExpressionHandle) -> SymbolHandle {
    match table.expression(expression) {
        ExpressionNode::Borrow(target) => storage_path_head_symbol(table, target.target),
        ExpressionNode::Indexed(indexed) => storage_path_head_symbol(table, indexed.collection),
        ExpressionNode::Member(member) => storage_path_head_symbol(table, member.receiver),
        ExpressionNode::Call(call) => slice_view_call_receiver_handle(call)
            .map(|receiver| storage_path_head_symbol(table, receiver))
            .unwrap_or_else(SymbolHandle::invalid),
        ExpressionNode::Name(path) => path.head_symbol,
        _ => SymbolHandle::invalid(),
    }
}

fn storage_path_len(table: &ExpressionTable, expression: ExpressionHandle) -> Option<usize> {
    match table.expression(expression) {
        ExpressionNode::Borrow(target) => storage_path_len(table, target.target),
        ExpressionNode::Indexed(indexed) => {
            let ExpressionNode::Integer(_) = table.expression(indexed.index) else {
                return None;
            };
            // A STACKED index (`cube[1][1]` -- an Indexed whose collection is
            // itself Indexed) cannot ride a name path: each member carries ONE
            // element index, and `member_index` returns the OUTERMOST one,
            // silently swallowing the inner -- the path would alias `cube[1]`.
            // Refuse, so callers fall to resolvers that bias the base per level
            // (`resolve_machine_owned_collection_with_const_prefix_in_table`).
            let mut collection = indexed.collection;
            while let ExpressionNode::Borrow(inner) = table.expression(collection) {
                collection = inner.target;
            }
            if matches!(table.expression(collection), ExpressionNode::Indexed(_)) {
                return None;
            }
            storage_path_len(table, indexed.collection)
        }
        ExpressionNode::Member(member) => storage_path_len(table, member.receiver)?.checked_add(1),
        ExpressionNode::Call(call) => {
            storage_path_len(table, slice_view_call_receiver_handle(call)?)
        }
        ExpressionNode::Name(path) => Some(table.name_path_members(path.members).len()),
        _ => None,
    }
}

fn storage_path_member(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    index: usize,
) -> Option<&Identifier> {
    match table.expression(expression) {
        ExpressionNode::Borrow(target) => storage_path_member(table, target.target, index),
        ExpressionNode::Indexed(indexed) => storage_path_member(table, indexed.collection, index),
        ExpressionNode::Call(call) => {
            storage_path_member(table, slice_view_call_receiver_handle(call)?, index)
        }
        ExpressionNode::Member(member) => {
            let receiver_len = storage_path_len(table, member.receiver)?;
            if index == receiver_len {
                Some(&member.member)
            } else {
                storage_path_member(table, member.receiver, index)
            }
        }
        ExpressionNode::Name(path) => table.name_path_members(path.members).get(index),
        _ => None,
    }
}

fn storage_path_member_symbol(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    index: usize,
) -> SymbolHandle {
    match table.expression(expression) {
        ExpressionNode::Borrow(target) => storage_path_member_symbol(table, target.target, index),
        ExpressionNode::Indexed(indexed) => {
            storage_path_member_symbol(table, indexed.collection, index)
        }
        ExpressionNode::Call(call) => slice_view_call_receiver_handle(call)
            .map(|receiver| storage_path_member_symbol(table, receiver, index))
            .unwrap_or_else(SymbolHandle::invalid),
        ExpressionNode::Member(member) => {
            let Some(receiver_len) = storage_path_len(table, member.receiver) else {
                return SymbolHandle::invalid();
            };
            if index == receiver_len {
                member.member_symbol
            } else {
                storage_path_member_symbol(table, member.receiver, index)
            }
        }
        ExpressionNode::Name(path) => borrowed_member_symbol(
            table.name_path_members(path.members),
            table.name_path_member_symbols(path.member_symbols),
            path.head_symbol,
            path.symbol,
            index,
        ),
        _ => SymbolHandle::invalid(),
    }
}

/// The case variant a destructure-bound payload member came from, if any. Walks
/// the Member chain like `storage_path_member_symbol` and returns the variant
/// recorded on the Member node at `index` (so the layout resolver can pick THAT
/// variant's field when several variants share a payload field name).
fn storage_path_member_case_variant(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    index: usize,
) -> Option<&Identifier> {
    match table.expression(expression) {
        ExpressionNode::Borrow(target) => {
            storage_path_member_case_variant(table, target.target, index)
        }
        ExpressionNode::Indexed(indexed) => {
            storage_path_member_case_variant(table, indexed.collection, index)
        }
        ExpressionNode::Call(call) => {
            storage_path_member_case_variant(table, slice_view_call_receiver_handle(call)?, index)
        }
        ExpressionNode::Member(member) => {
            let receiver_len = storage_path_len(table, member.receiver)?;
            if index == receiver_len {
                member.case_variant.as_ref()
            } else {
                storage_path_member_case_variant(table, member.receiver, index)
            }
        }
        _ => None,
    }
}

fn storage_path_member_index(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    index: usize,
) -> Option<usize> {
    match table.expression(expression) {
        ExpressionNode::Borrow(target) => storage_path_member_index(table, target.target, index),
        ExpressionNode::Indexed(indexed) => {
            let indexed_len = storage_path_len(table, indexed.collection)?;
            if index + 1 == indexed_len {
                let ExpressionNode::Integer(value) = table.expression(indexed.index) else {
                    return None;
                };
                usize::try_from(value.value_i64()?).ok()
            } else {
                storage_path_member_index(table, indexed.collection, index)
            }
        }
        ExpressionNode::Call(call) => {
            storage_path_member_index(table, slice_view_call_receiver_handle(call)?, index)
        }
        ExpressionNode::Member(member) => {
            let receiver_len = storage_path_len(table, member.receiver)?;
            if index < receiver_len {
                storage_path_member_index(table, member.receiver, index)
            } else {
                None
            }
        }
        ExpressionNode::Name(_) => None,
        _ => None,
    }
}

fn borrowed_member_symbol(
    members: &[Identifier],
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

fn slice_view_collection_expression<'expr>(expression: &'expr Expression) -> &'expr Expression {
    match expression {
        Expression::Call(call) => slice_view_call_receiver_expression(call).unwrap_or(expression),
        _ => expression,
    }
}

fn slice_view_call_receiver_expression(
    call: &psi_checked_trees::expression::CallExpression,
) -> Option<&Expression> {
    if !call.arguments.is_empty()
        || !matches!(
            call.target.as_str(),
            "as_slice" | "as_mut_slice" | "as_view" | "bytes"
        )
    {
        return None;
    }
    call.receiver.as_deref()
}

fn slice_view_call_receiver_handle(call: &TableCallExpression) -> Option<ExpressionHandle> {
    if !call.receiver.is_valid()
        || !call.arguments.is_empty()
        || !matches!(
            call.target.as_str(),
            "as_slice" | "as_mut_slice" | "as_view" | "bytes"
        )
    {
        return None;
    }
    Some(call.receiver)
}

#[derive(Clone, Copy)]
pub(in crate::selection) struct StoragePathSuffix<'path, 'table> {
    path: &'path StorageNamePath<'table>,
    start: usize,
}

impl<'path, 'table> StoragePathSuffix<'path, 'table> {
    pub(in crate::selection) fn iter(self) -> StoragePathSuffixIter<'path, 'table> {
        StoragePathSuffixIter {
            path: self.path,
            index: self.start,
        }
    }
}

pub(in crate::selection) struct StoragePathSuffixIter<'path, 'table> {
    path: &'path StorageNamePath<'table>,
    index: usize,
}

impl<'path, 'table> Iterator for StoragePathSuffixIter<'path, 'table> {
    type Item = (
        &'path Identifier,
        SymbolHandle,
        Option<usize>,
        Option<&'path Identifier>,
    );

    fn next(&mut self) -> Option<Self::Item> {
        let member = self.path.member(self.index)?;
        let symbol = self.path.member_symbol(self.index);
        let field_index = self.path.member_index(self.index);
        let case_variant = self.path.member_case_variant(self.index);
        self.index += 1;
        Some((member, symbol, field_index, case_variant))
    }
}
