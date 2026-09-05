use crate::name::lower_name;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(super) fn lower_name_path_members_into_table(
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    members: arena::HandleSpan<resolved::name::DiagnosticName>,
) -> arena::HandleSpan<typed::name::Identifier> {
    let mut lowered = arena::HandleSpan::empty();

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
) -> arena::HandleSpan<symbols::SymbolHandle> {
    let mut lowered = arena::HandleSpan::empty();

    for member_symbol in source.name_path_member_symbols(path.member_symbols) {
        let member_symbol = *member_symbol;
        target.push_name_path_member_symbol(&mut lowered, member_symbol);
    }

    lowered
}
