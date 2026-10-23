//! Declaration-local storage identity for a direct attached-data self field.

use super::exact_data_member_field;
use symbols::SymbolHandle;
use typed_trees::data::DataField;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;

/// Resolve a direct `self.field` under this machine's exact attached declaration.
/// Another receiver of the same type is not the same storage. An absent field
/// symbol may resolve uniquely within that declaration, but a conflicting
/// retained symbol cannot be replaced by a same-spelled field.
/// A one-member `receiver.field` place resolved against an exact receiver
/// symbol: the receiver path must name `receiver_symbol` spelled as exactly
/// one member satisfying `member_matches` — `self` for the machine's own
/// storage, the storage root's name for a parameter-carried root.
pub fn single_member_receiver_field<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
    receiver_symbol: SymbolHandle,
    member_matches: impl Fn(&typed_trees::name::Identifier) -> bool,
) -> Option<&'program DataField> {
    let member = program.expression_table.member_node(expression)?;
    if member.case_variant.is_some() {
        return None;
    }
    let Some(receiver) = program.expression_table.name_path_node(member.receiver) else {
        return None;
    };
    if receiver.symbol != receiver_symbol
        || receiver.head_symbol != receiver_symbol
        || !matches!(program.expression_table.name_path_members(receiver.members),
            [only] if member_matches(only))
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

pub fn exact_self_field<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
) -> Option<&'program DataField> {
    if !machine.symbol.is_valid() || !machine.attached_data_symbol.is_valid() {
        return None;
    }
    single_member_receiver_field(
        program,
        machine,
        expression,
        machine.symbol,
        typed_trees::name::Identifier::is_self_receiver,
    )
}

/// Resolve a field in the exact carrier declaration, without machine aliases.
/// The caller establishes that the expression denotes this receiver's storage.
pub fn exact_attached_field<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    field_symbol: SymbolHandle,
    field_name: &str,
) -> Option<&'program DataField> {
    if !machine.symbol.is_valid() || !machine.attached_data_symbol.is_valid() {
        return None;
    }
    let owner = super::unique_data_definition_by_symbol(program, machine.attached_data_symbol)?;
    exact_data_member_field(program, owner, field_symbol, field_name, None)
}
