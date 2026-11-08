// Declaration-membership guard rejecting indexed domain membership clauses until a
// canonical package-review application record represents them; we never collapse an
// indexed proof clause to its family merely because parameter identities are also
// retained.

use diagnostics::Diagnostic;
use typed_trees::domain::ProofMembershipFact;

pub(crate) fn require_declaration_membership(
    program: &dyn ::validation::CheckingInput,
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
