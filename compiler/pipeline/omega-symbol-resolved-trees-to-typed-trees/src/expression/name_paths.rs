use crate::name::lower_name;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(super) fn lower_name_path_members_into_table(
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    members: omega_core::arena::HandleSpan<resolved::name::DiagnosticName>,
) -> omega_core::arena::HandleSpan<typed::name::Identifier> {
    let mut lowered = omega_core::arena::HandleSpan::empty();

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
    head_symbol: omega_core::symbols::SymbolHandle,
    symbol: omega_core::symbols::SymbolHandle,
) -> omega_core::arena::HandleSpan<omega_core::symbols::SymbolHandle> {
    let mut lowered = omega_core::arena::HandleSpan::empty();

    for offset in 0..member_count {
        let member_symbol = if offset == 0 {
            head_symbol
        } else if offset + 1 == member_count {
            symbol
        } else {
            omega_core::symbols::SymbolHandle::invalid()
        };
        target.push_name_path_member_symbol(&mut lowered, member_symbol);
    }

    lowered
}
