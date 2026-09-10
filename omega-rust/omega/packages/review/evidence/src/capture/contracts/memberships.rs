//! Public membership records currently name a declaration, not an application.
//! Qualified parameter types have their own exact indexed identity and do not
//! pass through this clause projection. Never collapse an indexed proof clause
//! to its family merely because those parameter identities are also retained.

use diagnostics::Diagnostic;
use typed_trees::{TypedTrees, domain::ProofMembershipFact};

pub(crate) fn require_declaration_membership(
    program: &TypedTrees,
    membership: &ProofMembershipFact,
) -> Result<(), Vec<Diagnostic>> {
    if !membership.domain_arguments.is_empty()
        || program.domain_definitions().iter().any(|domain| {
            domain.symbol == membership.domain_symbol
                && (!typed_trees::domain::index_parameters(program, domain).is_empty()
                    || (membership.semantic_domain.is_valid()
                        && membership.semantic_domain != domain.semantic_id))
        })
    {
        return Err(vec![Diagnostic::error(
            "indexed domain membership clauses require a canonical package-review application record",
        )]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_membership_arguments_cannot_project_as_a_family() {
        let mut program = TypedTrees::default();
        for index in [7, 9] {
            let argument =
                program
                    .type_reference_table
                    .insert(typed_trees::types::TypeReferenceNode::Named {
                        symbol: symbols::SymbolHandle::invalid(),
                        name: typed_trees::name::Identifier::generated(index.to_string()),
                    });
            let membership = ProofMembershipFact {
                domain_arguments: program
                    .type_reference_table
                    .insert_type_reference_handles([argument]),
                ..Default::default()
            };
            let error = require_declaration_membership(&program, &membership).unwrap_err();
            assert!(
                error[0]
                    .message
                    .contains("canonical package-review application record")
            );
        }
        assert!(require_declaration_membership(&program, &ProofMembershipFact::default()).is_ok());
    }
}
