//! Direct owned projections contribute storage bounds under their exact owner.

use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn type_reference(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    // When true, a mutable receiver still contributes the field's declared
    // storage bounds: every store maintains them, so they hold at every
    // evaluation. When false, mutable places are opaque.
    declared_mutable_leaves: bool,
) -> Option<TypeReferenceHandle> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    let ExpressionNode::Name(receiver) = program.expression_table.expression(member.receiver)
    else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(receiver.members) else {
        return None;
    };
    // Which parameter carries the machine's data record, under either
    // spelling. The exemptions below are about the machine's OWN storage, not
    // about the keyword that used to name it.
    let parameters = program.state_parameters(state);
    let subject = program
        .data_binding_position(machine.attached_data_symbol, parameters)
        .map(|position| parameters[position].symbol);
    let is_subject = |parameter: &typed_trees::signature::StateParameter| {
        parameter.is_self || (parameter.symbol.is_valid() && Some(parameter.symbol) == subject)
    };
    let parameter = parameters.iter().find(|parameter| {
        let symbol_match = receiver.symbol.is_valid()
            && receiver.symbol == receiver.head_symbol
            && parameter.symbol == receiver.symbol;
        let self_match = parameter.is_self && name.is_self_receiver();
        (symbol_match || self_match)
            && parameter.name == *name
            // The machine's own field storage enforces its declared bounds on
            // every write, so `self.k` keeps them even through a `&mut self`
            // receiver — the read is of storage, not of a snapshot-only value.
            // A named subject is that same storage, so it keeps them too.
            && (declared_mutable_leaves || !parameter.is_mutable || is_subject(parameter))
            && !parameter.is_const
    })?;
    // The machine's own field storage enforces its declared bounds on every
    // write, so `self.k` keeps them even through a `&mut self` receiver. Peel
    // the receiver's self reference (the `Self` alias carries the field
    // parent), and resolve the record declaration through the machine's
    // attached data. An ordinary reference or generic receiver still needs
    // load or instantiated-field evidence; a named record contributes only
    // its stored field's own enforced bounds.
    let mut owner_type = parameter.type_reference;
    if is_subject(parameter) {
        while let TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } = program.type_reference_table.type_reference(owner_type)
        {
            owner_type = *referee;
        }
    }
    let TypeReferenceNode::Named { symbol: owner, .. } =
        program.type_reference_table.type_reference(owner_type)
    else {
        return None;
    };
    let declaration = if parameter.is_self {
        let attached = machine.attached_data.as_ref()?;
        program
            .data_definitions()
            .iter()
            .find(|definition| definition.name == *attached)?
    } else {
        program
            .data_definitions()
            .iter()
            .find(|declaration| owner.is_valid() && declaration.symbol == *owner)?
    };
    let selected = program.symbols.get(member.member_symbol);
    if !member.member_symbol.is_valid()
        || selected.kind != SymbolKind::Field
        || selected.parent != *owner
        || member.case_variant.is_some()
    {
        return None;
    }
    // A `self` member's resolved symbol is scoped to the `Self` receiver
    // spelling, not to the declaration's own field symbols — the exact-name
    // match inside the owning record is already unique.
    let member_symbol = if parameter.is_self {
        SymbolHandle::invalid()
    } else {
        member.member_symbol
    };
    crate::value_custody::places::exact_data_member_field(
        program,
        declaration,
        member_symbol,
        member.member.as_str(),
        None,
    )
    .map(|field| field.type_reference)
}
