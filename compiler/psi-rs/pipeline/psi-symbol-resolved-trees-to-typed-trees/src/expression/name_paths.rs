use crate::name::lower_name;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

pub(super) fn lower_name_path_members_into_table(
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    members: psi_arena::HandleSpan<resolved::name::DiagnosticName>,
) -> psi_arena::HandleSpan<typed::name::Identifier> {
    let mut lowered = psi_arena::HandleSpan::empty();

    for member in source.name_path_members(members) {
        target.push_name_path_member(&mut lowered, lower_name(member));
    }

    lowered
}

pub(super) fn lower_table_name_path_node_into_table(
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    path: &resolved::expression::TableNamePath,
) -> typed::expression::TableNamePath {
    let members = lower_name_path_members_into_table(source, target, path.members);
    let member_symbols = lower_name_path_member_symbols_into_table(
        target,
        path.members.count(),
        path.head_symbol,
        path.symbol,
    );

    typed::expression::TableNamePath {
        members,
        member_symbols,
        head_symbol: path.head_symbol,
        symbol: path.symbol,
    }
}

fn lower_name_path_member_symbols_into_table(
    target: &mut typed::expression::ExpressionTable,
    member_count: u32,
    head_symbol: psi_symbols::SymbolHandle,
    symbol: psi_symbols::SymbolHandle,
) -> psi_arena::HandleSpan<psi_symbols::SymbolHandle> {
    let mut lowered = psi_arena::HandleSpan::empty();

    for offset in 0..member_count {
        let member_symbol = if offset == 0 {
            head_symbol
        } else if offset + 1 == member_count {
            symbol
        } else {
            psi_symbols::SymbolHandle::invalid()
        };
        target.push_name_path_member_symbol(&mut lowered, member_symbol);
    }

    lowered
}
