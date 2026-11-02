//! Project authored refinement ceilings into their exact logical callable view.
//! The inherited declaration and runtime application stay unchanged. A bound's
//! supplying frames retain both operand custody and the source of its stronger
//! promises, so a refined view cannot strengthen an unrelated base invocation.

use super::AppliedTraitScope;
use crate::{
    TypedTrees,
    signature::StateSignature,
    trait_definition::{TraitRefinementClause, TraitRefinementReach},
};
use symbols::SymbolHandle;

impl<'program> AppliedTraitScope<'program> {
    fn refinement_prefixes<'scope>(
        &'scope self,
        program: &'scope TypedTrees,
    ) -> Vec<AppliedTraitScope<'scope>> {
        self.region_scopes
            .iter()
            .enumerate()
            .filter_map(|(position, frame)| {
                let declaration = program.trait_definition_by_symbol(frame.declaration)?;
                if declaration.refines.is_none() || declaration.refinement_clauses.is_empty() {
                    return None;
                }
                let mut prefix: AppliedTraitScope<'scope> = self.clone();
                prefix.declaration = declaration;
                let end = self
                    .substitution_scope_starts
                    .get(position + 1)
                    .copied()
                    .unwrap_or(self.substitutions.len());
                prefix.substitutions.truncate(end);
                prefix.substitution_scope_starts.truncate(position + 1);
                prefix.region_scopes.truncate(position + 1);
                Some(prefix)
            })
            .collect()
    }

    pub(crate) fn callable_contract_key(
        &self,
        program: &TypedTrees,
    ) -> Option<(SymbolHandle, crate::type_identity::NormalizedTypeIdentity)> {
        let prefixes = self.refinement_prefixes(program);
        let owner = prefixes
            .first()
            .map_or(self.declaration.symbol, |scope| scope.declaration.symbol);
        let identities = prefixes
            .iter()
            .map(|scope| scope.application_identity(program))
            .collect::<Option<Vec<_>>>()?;
        crate::type_identity::local_trait_contract_identity(
            program,
            self.application_identity(program)?,
            &identities,
        )
        .map(|identity| (owner, identity))
    }

    pub(crate) fn callable_refinement_clauses(
        &self,
        program: &TypedTrees,
        requirement: &str,
    ) -> Vec<TraitRefinementClause> {
        self.refinement_prefixes(program)
            .into_iter()
            .flat_map(|prefix| {
                prefix
                    .declaration
                    .refinement_clauses
                    .iter()
                    .filter(|clause| {
                        clause
                            .requirement
                            .as_ref()
                            .is_none_or(|name| name.as_str() == requirement)
                    })
                    .cloned()
            })
            .collect()
    }
}

impl TypedTrees {
    pub(crate) fn apply_callable_refinements(
        &mut self,
        signature: &mut StateSignature,
        clauses: &[TraitRefinementClause],
    ) -> Result<(), &'static str> {
        for clause in clauses {
            if !clause.signature.suspends_keyword_source_spans.is_empty() {
                signature.suspends &= clause.signature.suspends;
                signature
                    .suspends_keyword_source_spans
                    .extend_from_slice(&clause.signature.suspends_keyword_source_spans);
            }
            if !clause.signature.blocks_keyword_source_spans.is_empty() {
                signature.blocks &= clause.signature.blocks;
                signature
                    .blocks_keyword_source_spans
                    .extend_from_slice(&clause.signature.blocks_keyword_source_spans);
            }
            if let language_semantics::TerminationGuarantee::Terminates { premises } =
                &clause.signature.termination_guarantee
            {
                if premises.is_empty()
                    || matches!(
                        signature.termination_guarantee,
                        language_semantics::TerminationGuarantee::NoGuarantee
                    )
                {
                    signature.termination_guarantee =
                        clause.signature.termination_guarantee.clone();
                }
            }
            let row = match &clause.service_reach {
                TraitRefinementReach::Inherited => continue,
                TraitRefinementReach::Concrete(row) => *row,
                TraitRefinementReach::IndependentBounded(rows) => {
                    rows.iter()
                        .find(|row| row.requirement == signature.name)
                        .ok_or("refinement callable view lost its independent requirement row")?
                        .row
                }
            };
            if row == language_semantics::ServiceReachRowId::NULL {
                return Err("refinement callable view lost its authored reach row");
            }
            if signature.service_reach_is_installation_bound {
                signature.service_reach_row = row;
            } else {
                let current = signature.service_reach_row;
                let abstract_row = self.service_reach_rows.is_abstract(current)
                    || self.service_reach_rows.is_abstract(row);
                let services = self
                    .service_reach_rows
                    .services(current)
                    .iter()
                    .filter(|service| self.service_reach_rows.services(row).contains(service))
                    .copied()
                    .collect();
                let ceiling = self.service_reach_rows.intern(services);
                signature.service_reach_row = if abstract_row {
                    self.service_reach_rows.intern_abstract(ceiling)
                } else {
                    ceiling
                };
            }
            signature.service_reach_is_installation_bound = false;
        }
        Ok(())
    }
}
