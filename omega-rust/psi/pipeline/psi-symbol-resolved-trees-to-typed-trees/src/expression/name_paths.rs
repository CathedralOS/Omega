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
    let member_symbols = lower_name_path_member_symbols_into_table(source, target, path);

    typed::expression::TableNamePath {
        members,
        member_symbols,
        head_symbol: path.head_symbol,
        symbol: path.symbol,
    }
}

fn lower_name_path_member_symbols_into_table(
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    path: &resolved::expression::TableNamePath,
) -> psi_arena::HandleSpan<psi_symbols::SymbolHandle> {
    let mut lowered = psi_arena::HandleSpan::empty();

    for member_symbol in source.name_path_member_symbols(path.member_symbols) {
        let member_symbol = *member_symbol;
        target.push_name_path_member_symbol(&mut lowered, member_symbol);
    }

    lowered
}
