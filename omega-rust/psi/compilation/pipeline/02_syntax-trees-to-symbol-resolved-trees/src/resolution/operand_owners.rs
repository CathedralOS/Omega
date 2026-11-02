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

/// The allocation bridge is exact input custody, prepared before name walks.
/// It carries no inferred owner or checked admission.
pub(crate) fn bindings(
    lowerer: &Lowerer,
) -> Result<Vec<(OperandLifetimeOwnerHandle, usize)>, Vec<Diagnostic>> {
    lowerer
        .supplied_operand_owners
        .iter()
        .map(|(input, owner)| {
            let (_, ordinal) = lowerer
                .conformance_inputs
                .iter()
                .find(|(known, _)| known == input)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "supplied operand lost its exact declaration input",
                    )]
                })?;
            Ok((*owner, *ordinal))
        })
        .collect()
}

/// Declaration allocators already own these symbols. Publish that existing
/// input-to-declaration bridge before resolving any copied value expression.
pub(crate) fn bind_program(
    program: &mut symbol_resolved_trees::SymbolResolvedTrees,
    bindings: &[(OperandLifetimeOwnerHandle, usize)],
) -> Result<(), Vec<Diagnostic>> {
    for &(owner, ordinal) in bindings {
        let declaration = program.conformances[ordinal].symbol;
        if !declaration.is_valid() {
            return Err(vec![Diagnostic::error(
                "supplied operand has an unbound declaration owner",
            )]);
        }
        program
            .tables
            .types
            .operand_lifetime_owners
            .get_mut(owner)
            .declaration = declaration;
    }
    Ok(())
}
