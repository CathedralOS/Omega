//! Declaration-local storage identity for a direct attached-data self field.

use super::exact_data_member_field;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::{DataField, DataMember};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;

#[cfg(test)]
mod tests;

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
            [only] if only.as_str() == "self")
    {
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
    let field = exact_data_member_field(
        program,
        owner,
        SymbolHandle::invalid(),
        member.member.as_str(),
        None,
    )?;
    if !member.member_symbol.is_valid() || member.member_symbol == field.symbol {
        return Some(field);
    }

    // Resolution installs attached fields as the first Field children of an
    // attached machine, in the attached declaration's field order. Those
    // inherited symbols are distinct from the data's storage declarations.
    // Rejoin that exact slot rather than treating every same-named machine
    // member (or another receiver's field) as the storage declaration.
    let ordinal = program
        .data_members(owner)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
        })
        .position(|candidate| candidate.symbol == field.symbol)?;
    let inherited = program
        .symbols
        .child_handles(machine.symbol)?
        .filter(|symbol| program.symbols.get(*symbol).kind == SymbolKind::Field)
        .nth(ordinal)?;
    (member.member_symbol == inherited
        && program.symbols.get(inherited).parent == machine.symbol
        && program.symbols.name(inherited) == field.name.as_str())
    .then_some(field)
}
