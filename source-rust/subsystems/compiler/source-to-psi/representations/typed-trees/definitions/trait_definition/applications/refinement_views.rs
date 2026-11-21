use super::AppliedTraitScope;
use crate::source_state::{
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

    pub fn callable_contract_key(
        &self,
        program: &TypedTrees,
    ) -> Option<(
        SymbolHandle,
        crate::source_state::type_identity::NormalizedTypeIdentity,
    )> {
        let prefixes = self.refinement_prefixes(program);
        let owner = prefixes
            .first()
            .map_or(self.declaration.symbol, |scope| scope.declaration.symbol);
        let identities = prefixes
            .iter()
            .map(|scope| scope.application_identity(program))
            .collect::<Option<Vec<_>>>()?;
        crate::source_state::type_identity::local_trait_contract_identity(
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
    ) -> Vec<(TraitRefinementClause, super::TraitApplicationEnvironment)> {
        self.refinement_prefixes(program)
            .into_iter()
            .flat_map(|prefix| {
                let environment = prefix.environment();
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
                    .map(|clause| (clause, environment.clone()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

impl TypedTrees {
    /// Subset checks use the base requirement in the refining trait's scope,
    /// so a renamed binder and an inherited fixed parent keep their meanings.
    pub fn check_refined_service_ceilings(&self) -> Result<(), String> {
        for declaration in self
            .traits()
            .iter()
            .filter(|declaration| !declaration.refinement_clauses.is_empty())
        {
            let scope = AppliedTraitScope::abstract_scope(declaration);
            let base = scope.parent_scope(
                self,
                declaration
                    .refines
                    .as_ref()
                    .ok_or("refinement lost its base")?,
            )?;
            for clause in &declaration.refinement_clauses {
                let TraitRefinementReach::Concrete(narrowed) = clause.service_reach else {
                    continue;
                };
                let narrowed = self.refinement_service_keys(&scope, narrowed)?;
                for requirement in self
                    .trait_machine_signatures(base.declaration)
                    .iter()
                    .filter(|requirement| {
                        clause
                            .requirement
                            .as_ref()
                            .is_none_or(|name| name == &requirement.name)
                    })
                {
                    let wider =
                        self.refinement_service_keys(&base, requirement.service_reach_row)?;
                    if !narrowed.is_subset(&wider) {
                        return Err(format!(
                            "refinement clause adds a service outside the declared ceiling of `{}::{}`",
                            base.declaration.name, requirement.name
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn refinement_service_keys(
        &self,
        scope: &AppliedTraitScope<'_>,
        row: semantic_vocabulary::ServiceReachRowId,
    ) -> Result<std::collections::BTreeSet<String>, &'static str> {
        let mut services = std::collections::BTreeSet::new();
        for &id in self.service_reach_rows.services(row) {
            let member = self
                .service_reach_scope(id)
                .ok_or("refined service lacks its complete declared application")?
                .preceded_by(scope);
            member.try_visit(self, &mut |parent| {
                if parent.declaration.is_boundary {
                    let identity = parent
                        .portable_application_identity(self)
                        .and_then(|identity| self.type_identity_text(identity))
                        .ok_or("refined service has no checked application identity")?;
                    services.insert(identity);
                }
                Ok(())
            })?;
        }
        Ok(services)
    }

    fn applied_refinement_row(
        &mut self,
        environment: &super::TraitApplicationEnvironment,
        row: semantic_vocabulary::ServiceReachRowId,
        from_base: bool,
    ) -> Result<semantic_vocabulary::ServiceReachRowId, &'static str> {
        let scope = environment
            .scope(self)
            .ok_or("refinement lost its applied trait")?;
        let scope = if from_base {
            scope.parent_scope(
                self,
                scope
                    .declaration
                    .refines
                    .as_ref()
                    .ok_or("refinement lost its base trait")?,
            )?
        } else {
            scope
        };
        let mut members = Vec::new();
        for &id in self.service_reach_rows.services(row) {
            if self
                .service_reaches
                .definition(id)
                .is_some_and(|row| row.application.is_none())
            {
                members.push((id, None));
                continue;
            }
            let applied = self
                .service_reach_scope(id)
                .ok_or("refinement lost an applied service")?
                .preceded_by(&scope);
            applied.try_visit(self, &mut |parent| {
                if parent.declaration.is_boundary {
                    members.push((id, Some(parent.environment())));
                }
                Ok(())
            })?;
        }
        let mut ids = Vec::new();
        for (original, applied) in members {
            let id = match applied {
                Some(environment) => self.intern_service_reach_environment(&environment)?,
                None => self.service_reaches.canonical_id(original),
            };
            self.service_reaches.extend_closure(id, &mut ids);
        }
        let bound = self.service_reach_rows.intern(ids);
        Ok(if self.service_reach_rows.is_abstract(row) {
            self.service_reach_rows.intern_abstract(bound)
        } else {
            bound
        })
    }

    pub(crate) fn apply_callable_refinements(
        &mut self,
        signature: &mut StateSignature,
        clauses: &[(TraitRefinementClause, super::TraitApplicationEnvironment)],
    ) -> Result<(), &'static str> {
        for (clause, environment) in clauses {
            if !clause.signature.contracts.is_empty() {
                self.apply_refinement_postconditions(signature, &clause.signature, environment)?;
            }
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
            if let source_declarations::TerminationGuarantee::Terminates { premises } =
                &clause.signature.termination_guarantee
                && (premises.is_empty()
                    || matches!(
                        signature.termination_guarantee,
                        source_declarations::TerminationGuarantee::NoGuarantee
                    ))
            {
                signature.termination_guarantee = clause.signature.termination_guarantee.clone();
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
            if row == semantic_vocabulary::ServiceReachRowId::NULL {
                return Err("refinement callable view lost its authored reach row");
            }
            let row = self.applied_refinement_row(
                environment, row, matches!(clause.service_reach, TraitRefinementReach::IndependentBounded(_)),
            )?;
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
