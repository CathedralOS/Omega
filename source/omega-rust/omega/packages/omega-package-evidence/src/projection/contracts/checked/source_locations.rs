use crate::evidence::PackageReviewSourceLocationRole;
use crate::evidence::package::ProjectedNestedSourceLocation;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

fn proof_fact_handle_at(
    facts: psi_arena::HandleSpan<psi_typed_trees::domain::ProofFact>,
    offset: u32,
) -> psi_arena::Handle<psi_typed_trees::domain::ProofFact> {
    psi_arena::Handle::from_parts(
        facts
            .start()
            .arena_index()
            .checked_add(offset)
            .expect("proof fact handle index overflow"),
        facts.start().generation(),
    )
}

pub(crate) fn project_required_proof_fact_source_locations(
    compilation: &CheckedCompilation,
    facts: psi_arena::HandleSpan<psi_typed_trees::domain::ProofFact>,
    subject: &str,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let mut locations = Vec::with_capacity(facts.len());
    for offset in 0..facts.count() {
        let source_span = compilation
            .proof_fact_source_span(proof_fact_handle_at(facts, offset))
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "{subject} fact has no exact authored source custody"
                ))]
            })?;
        locations.push(ProjectedNestedSourceLocation {
            source_span,
            role: PackageReviewSourceLocationRole::ProofFact,
        });
    }
    Ok(locations)
}

pub(crate) fn project_contract_source_locations(
    compilation: &CheckedCompilation,
    contracts: &[psi_typed_trees::signature::SignatureContract],
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let mut locations = Vec::new();
    for contract in contracts {
        if let Some(source_span) = contract.keyword_source_span {
            locations.push(ProjectedNestedSourceLocation {
                source_span,
                role: PackageReviewSourceLocationRole::ContractClause,
            });
        }
        for offset in 0..contract.facts.count() {
            let fact = proof_fact_handle_at(contract.facts, offset);
            match compilation.proof_fact_source_span(fact) {
                Some(source_span) => locations.push(ProjectedNestedSourceLocation {
                    source_span,
                    role: PackageReviewSourceLocationRole::ProofFact,
                }),
                None if contract.keyword_source_span.is_some() => {
                    return Err(vec![Diagnostic::error(
                        "authored package-review contract fact has no exact source custody",
                    )]);
                }
                None => {}
            }
        }
    }
    Ok(locations)
}
