//! Which binding a declaration reaches the machine's attached data through.
//!
//! Under the static-machine contract a declaration names the data it operates
//! on explicitly: `state step(main: &mut Main)` reads `main.i` where a
//! receiver form reads `self.i`. Both name the same storage, so a query that
//! tests only the receiver spelling answers "not machine storage" for migrated
//! source, and every analysis built on that answer stops without saying so.
//!
//! [`attached_storage_root`] names the one binding a declaration reaches that
//! storage through: the receiver when the declaration takes `self`, otherwise
//! the sole parameter declared as the attached data itself. A declaration
//! carrying two such parameters has no single root — the two are distinct
//! storage, and choosing one would silently analyze the wrong place — so the
//! query answers `None` rather than pick.
//!
//! [`exact_attached_storage_field`] is the root-neutral twin of
//! `exact_self_field`: the same declaration-exact field identity, reached
//! through whichever root the declaration actually binds.

use super::{exact_attached_field, exact_self_field, unwrapped_type_reference};
use language_core::SELF_RECEIVER;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataField;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceNode;

/// The one binding a declaration reaches the machine's attached data through.
///
/// `name` is how the declaration spells it, so a caller holding only rendered
/// place text can take a path apart; `symbol` is what a rendered root must
/// resolve to, so a same-spelled binding elsewhere is not this storage.
pub struct AttachedStorageRoot<'parameters> {
    pub name: &'parameters str,
    pub symbol: SymbolHandle,
}

/// The binding `parameters` reaches this machine's attached data through.
///
/// A receiver wins outright: `self` cannot be shadowed, so a declaration that
/// takes one reaches its storage through nothing else. Resolution installs a
/// receiver name under the machine's own symbol, which is what
/// `exact_self_field` matches, so that is the symbol reported here.
pub fn attached_storage_root<'parameters>(
    program: &TypedTrees,
    machine: &Machine,
    parameters: &'parameters [StateParameter],
) -> Option<AttachedStorageRoot<'parameters>> {
    if !machine.symbol.is_valid() || !machine.attached_data_symbol.is_valid() {
        return None;
    }
    if parameters.iter().any(|parameter| parameter.is_self) {
        return Some(AttachedStorageRoot {
            name: SELF_RECEIVER,
            symbol: machine.symbol,
        });
    }
    let mut attached = parameters
        .iter()
        .filter(|parameter| parameter_declares_attached_data(program, machine, parameter));
    let parameter = attached.next()?;
    if attached.next().is_some() || !parameter.symbol.is_valid() {
        return None;
    }
    Some(AttachedStorageRoot {
        name: parameter.name.as_str(),
        symbol: parameter.symbol,
    })
}

/// Whether `parameter` is declared as the machine's exact attached data,
/// looking through the reference and constraint shells a `&mut Main` binding
/// carries. Another declaration of the same shape is not the same data.
fn parameter_declares_attached_data(
    program: &TypedTrees,
    machine: &Machine,
    parameter: &StateParameter,
) -> bool {
    let Some(declared) = unwrapped_type_reference(program, parameter.type_reference) else {
        return false;
    };
    matches!(
        program.type_reference_table.type_reference(declared),
        TypeReferenceNode::Named { symbol, .. } if *symbol == machine.attached_data_symbol
    )
}

/// Resolve a direct field of this machine's attached data, reached through
/// whichever root `parameters` binds: `self.field` or `main.field`.
///
/// The parameter form resolves the root by symbol, not by spelling, so a local
/// that happens to share the parameter's name is not this storage.
pub fn exact_attached_storage_field<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<&'program DataField> {
    if let Some(field) = exact_self_field(program, machine, expression) {
        return Some(field);
    }
    let root = attached_storage_root(program, machine, parameters)?;
    if root.name == SELF_RECEIVER {
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
    if receiver.symbol != root.symbol
        || receiver.head_symbol != root.symbol
        || !matches!(program.expression_table.name_path_members(receiver.members),
            [only] if only.as_str() == root.name)
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
