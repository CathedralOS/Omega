//! Translate copied operand custody at the declaration-allocation boundary.
//! Syntax roots identify supplying implementations before symbols exist. Each
//! produced conformance records that exact input and its allocation coordinate;
//! no name, source span or matching type shape reconstructs the association.

use super::lowerer::Lowerer;
use diagnostics::Diagnostic;
use symbol_resolved_trees::types::{OperandLifetimeOwner, OperandLifetimeOwnerHandle};
use syntax_trees::item::ItemHandle;

pub(crate) fn retain(lowerer: &mut Lowerer, input: ItemHandle) -> OperandLifetimeOwnerHandle {
    if !input.is_valid() {
        return OperandLifetimeOwnerHandle::invalid();
    }
    if let Some((_, owner)) = lowerer
        .supplied_operand_owners
        .iter()
        .find(|(known, _)| *known == input)
    {
        return *owner;
    }
    let declaration = lowerer
        .conformance_inputs
        .iter()
        .find(|(known, _)| *known == input)
        .map(|(_, ordinal)| lowerer.symbol_resolved_trees.conformances[*ordinal].symbol)
        .unwrap_or_default();
    let owner = lowerer
        .symbol_resolved_trees
        .tables
        .types
        .operand_lifetime_owners
        .insert(OperandLifetimeOwner { declaration });
    lowerer.supplied_operand_owners.push((input, owner));
    owner
}

pub(crate) fn bind(lowerer: &mut Lowerer) -> Result<(), Vec<Diagnostic>> {
    for (input, owner) in &lowerer.supplied_operand_owners {
        let Some((_, ordinal)) = lowerer
            .conformance_inputs
            .iter()
            .find(|(known, _)| known == input)
        else {
            return Err(vec![Diagnostic::error(
                "supplied operand lost its exact declaration input",
            )]);
        };
        let declaration = lowerer.symbol_resolved_trees.conformances[*ordinal].symbol;
        if !declaration.is_valid() {
            return Err(vec![Diagnostic::error(
                "supplied operand has an unbound declaration owner",
            )]);
        }
        lowerer
            .symbol_resolved_trees
            .tables
            .types
            .operand_lifetime_owners
            .get_mut(*owner)
            .declaration = declaration;
    }
    Ok(())
}
