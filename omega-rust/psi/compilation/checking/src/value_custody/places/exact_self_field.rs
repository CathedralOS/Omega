//! Declaration-local storage identity for a direct attached-data self field.

use super::exact_data_member_field;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataField;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;

/// Resolve a direct `self.field` under this machine's exact attached declaration.
/// Another receiver of the same type is not the same storage. An absent field
/// symbol may resolve uniquely within that declaration, but a conflicting
/// retained symbol cannot be replaced by a same-spelled field.
pub fn exact_self_field<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
) -> Option<&'program DataField> {
    if !machine.symbol.is_valid() || !machine.attached_data_symbol.is_valid() {
        return None;
    }
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    if member.case_variant.is_some() {
        return None;
    }
    let ExpressionNode::Name(receiver) = program.expression_table.expression(member.receiver)
    else {
        return None;
    };
    if receiver.symbol != machine.symbol
        || receiver.head_symbol != machine.symbol
        || !matches!(program.expression_table.name_path_members(receiver.members),
            [only] if only.is_self_receiver())
    {
        return None;
    }
    exact_attached_field(
        program,
        machine,
        member.member_symbol,
        member.member.as_str(),
    )
}

/// Resolve a field in the exact carrier declaration, without machine aliases.
/// The caller establishes that the expression denotes this receiver's storage.
pub fn exact_attached_field<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    field_symbol: SymbolHandle,
    field_name: &str,
) -> Option<&'program DataField> {
    if !machine.symbol.is_valid() || !machine.attached_data_symbol.is_valid() {
        return None;
    }
    let mut owners = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == machine.attached_data_symbol);
    let owner = owners.next()?;
    if owners.next().is_some() {
        return None;
    }
    exact_data_member_field(program, owner, field_symbol, field_name, None)
}
