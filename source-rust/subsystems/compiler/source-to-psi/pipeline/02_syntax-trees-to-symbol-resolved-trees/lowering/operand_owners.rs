/*
    Translate copied operand custody at the declaration-allocation boundary.
    Syntax roots identify supplying declarations before symbols exist. Each
    produced trait or conformance records that input and its allocation coordinate;
    no name, source span or matching type shape reconstructs the association.
*/

use crate::resolution::lowering::lowerer::Lowerer;
use crate::source_resolved::types::{OperandLifetimeOwner, OperandLifetimeOwnerHandle};
use diagnostics::Diagnostic;
use syntax_trees::item::ItemHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub(crate) enum DeclarationInput {
    Conformance(usize),
    Trait(usize),
}

impl DeclarationInput {
    fn symbol(
        self,
        program: &crate::source_resolved::SymbolResolvedTrees,
    ) -> symbols::SymbolHandle {
        match self {
            Self::Conformance(ordinal) => program.conformances[ordinal].symbol,
            Self::Trait(ordinal) => program.traits[ordinal].symbol,
        }
    }
}

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
        .declaration_inputs
        .iter()
        .find(|(known, _)| *known == input)
        .map(|(_, declaration)| declaration.symbol(&lowerer.symbol_resolved_trees))
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

/*
    The allocation bridge is exact input custody, prepared before name walks.
    It carries no inferred owner or checked admission.
*/
pub(crate) fn bindings(
    lowerer: &Lowerer,
) -> Result<Vec<(OperandLifetimeOwnerHandle, DeclarationInput)>, Vec<Diagnostic>> {
    lowerer
        .supplied_operand_owners
        .iter()
        .map(|(input, owner)| {
            let (_, ordinal) = lowerer
                .declaration_inputs
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

/*
    Declaration allocators already own these symbols. Publish that existing
    input-to-declaration bridge before resolving any copied value expression.
*/
pub(crate) fn bind_program(
    program: &mut crate::source_resolved::SymbolResolvedTrees,
    bindings: &[(OperandLifetimeOwnerHandle, DeclarationInput)],
) -> Result<(), Vec<Diagnostic>> {
    for &(owner, ordinal) in bindings {
        let declaration = ordinal.symbol(program);
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
