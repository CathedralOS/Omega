use super::{AppliedCallableOwner, AppliedCallableSignature, CallableApplication};
use crate::source_state::{TypedTrees, signature, trait_definition::AppliedTraitScope};
use symbols::SymbolHandle;

// The existing installation loop owns these transient requests for both a
// program preparation and a newly constructed call's exact application.
type CallableSignatureRequest = (
    SymbolHandle,
    SymbolHandle,
    crate::source_state::type_identity::NormalizedTypeIdentity,
    SymbolHandle,
    crate::source_state::trait_definition::TraitApplicationEnvironment,
    Vec<(
        crate::source_state::trait_definition::TraitRefinementClause,
        crate::source_state::trait_definition::TraitApplicationEnvironment,
    )>,
);

/// Signatures requested by a particular set of trait scopes, ready for substitution.
pub struct CallableSignatureRequests(Vec<CallableSignatureRequest>);

impl TypedTrees {
    pub fn applied_callable_signature(
        &self,
        application: &AppliedTraitScope<'_>,
        requirement: SymbolHandle,
    ) -> Option<&signature::StateSignature> {
        if let Some(closed) = self.closed_requirement_application(requirement)
            && self.requirement_application_matches_contract_scope(requirement, application)
        {
            return Some(closed.signature);
        }
        let (owner, identity) = application.callable_contract_key(self)?;
        self.applied_callable_signatures
            .iter()
            .find_map(|(_, prepared)| {
                let AppliedCallableOwner::Trait {
                    declaration,
                    application: prepared_identity,
                    ..
                } = &prepared.owner
                else {
                    return None;
                };
                if *declaration != owner
                    || *prepared_identity != identity
                    || prepared.signature.symbol != requirement
                {
                    return None;
                }
                Some(&prepared.signature)
            })
            // A Build may close a provider's entire public contract without
            // a source call ever requesting a separate applied signature.
            .or_else(|| self.closed_requirement_in_scope(application, requirement))
    }

    /// The already closed header of a requirement with no local arguments
    /// in this exact declaring trait application.
    pub fn closed_requirement_in_scope(
        &self,
        scope: &AppliedTraitScope<'_>,
        template: SymbolHandle,
    ) -> Option<&signature::StateSignature> {
        let mut matching = self.closed_requirement_applications().filter(|closed| {
            closed.template == template
                && closed.local_arguments.is_empty()
                && self
                    .requirement_application_matches_contract_scope(closed.signature.symbol, scope)
        });
        let header = matching.next()?.signature;
        matching.next().is_none().then_some(header)
    }

    /// Prepare the public signatures of the exact retained application
    /// before constructing its runtime actuals. The ordinary program pass
    /// uses the same request and installation operations; this grants no
    /// conformance proof or executable body.
    pub fn prepare_applied_callable_signatures_for_application(
        &mut self,
        lexical_owner: SymbolHandle,
        application: crate::source_state::types::TypeReferenceHandle,
    ) -> Result<(), &'static str> {
        if !self
            .type_reference_table
            .contains_type_reference(application)
            || !self.symbols.symbols().nodes().is_valid(lexical_owner)
        {
            return Err("applied callable lost its source application owner");
        }
        let scope = self
            .applied_requirement_scope(lexical_owner, application)
            .ok_or("applied callable lost its complete requirement application")?;
        let pending = self.applied_callable_signature_requests(vec![scope]);
        pending.apply(self)
    }

    pub fn applied_callable_signature_requests<'program>(
        &'program self,
        mut roots: Vec<AppliedTraitScope<'program>>,
    ) -> CallableSignatureRequests {
        let mut bound_roots = Vec::new();
        for root in &roots {
            root.visit(self, &mut |scope| {
                for bound in &scope.declaration.conformance_bounds {
                    if let Some(bound) = scope.bound_scope(self, bound.binder) {
                        bound_roots.push(bound);
                    }
                }
            });
        }
        roots.extend(bound_roots);
        let mut pending = Vec::new();
        for scope in roots {
            scope.visit(self, &mut |scope| {
                for signature in self.trait_machine_signatures(scope.declaration) {
                    let clauses = scope.callable_refinement_clauses(self, signature.name.as_str());
                    if signature.type_parameters.is_empty()
                        && scope.substitutions.is_empty()
                        && scope.declaration.lifetime_parameters.is_empty()
                        && clauses.is_empty()
                    {
                        continue;
                    }
                    let Some((owner, identity)) = scope.callable_contract_key(self) else {
                        continue;
                    };
                    if !clauses.is_empty() {
                        for closed in self.closed_requirement_applications().filter(|closed| {
                            closed.template == signature.symbol
                                && self.requirement_application_matches_scope(
                                    closed.signature.symbol,
                                    scope,
                                )
                                && !self.requirement_application_matches_contract_scope(
                                    closed.signature.symbol,
                                    scope,
                                )
                        }) {
                            pending.push((
                                scope.declaration.symbol,
                                owner,
                                identity,
                                closed.signature.symbol,
                                scope.environment(),
                                clauses.clone(),
                            ));
                        }
                    }
                    pending.push((
                        scope.declaration.symbol,
                        owner,
                        identity,
                        signature.symbol,
                        scope.environment(),
                        clauses,
                    ));
                }
            });
        }
        CallableSignatureRequests(pending)
    }
}

impl CallableSignatureRequests {
    pub fn apply(self, program: &mut TypedTrees) -> Result<(), &'static str> {
        let pending = self.0;
        let row_key =
            |requirement: SymbolHandle,
             declaration: SymbolHandle,
             identity: crate::source_state::type_identity::NormalizedTypeIdentity| {
                (
                    (requirement.arena_index(), requirement.generation()),
                    (declaration.arena_index(), declaration.generation()),
                    identity,
                )
            };
        let index_rows = |program: &TypedTrees| {
            let mut rows = program
                .applied_callable_signatures
                .iter()
                .filter_map(|(_, prepared)| match prepared.owner {
                    AppliedCallableOwner::Trait {
                        declaration,
                        application,
                        ..
                    } => Some(row_key(prepared.signature.symbol, declaration, application)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            rows.sort_unstable();
            (rows, program.applied_callable_signatures.revision())
        };
        let (mut prepared_rows, mut indexed_revision) = index_rows(program);
        for (trait_symbol, owner, identity, requirement, environment, clauses) in pending {
            if program.applied_callable_signatures.revision() != indexed_revision {
                (prepared_rows, indexed_revision) = index_rows(program);
            }
            if prepared_rows
                .binary_search(&row_key(requirement, owner, identity))
                .is_ok()
            {
                continue;
            }
            let mut applied = if let Some(closed) =
                program.closed_requirement_application(requirement)
            {
                closed.signature.clone()
            } else {
                let application = program.materialize_trait_application(&environment)?;
                let arguments = program.trait_application_arguments(trait_symbol, &application)?;
                let Some(applied) = CallableApplication {
                    trait_region_application: Some((
                        environment.clone(),
                        environment.frame_count(),
                    )),
                    ..CallableApplication::default()
                }
                .nominal(program, trait_symbol, requirement, &arguments)?
                else {
                    continue;
                };
                applied
            };
            program.apply_callable_refinements(&mut applied, &clauses)?;
            let scope = environment.retain(program)?;
            let inserted = row_key(applied.symbol, owner, identity);
            let before_insert = program.applied_callable_signatures.revision();
            program
                .applied_callable_signatures
                .insert(AppliedCallableSignature {
                    owner: AppliedCallableOwner::Trait {
                        declaration: owner,
                        application: identity,
                        scope,
                    },
                    signature: applied,
                });
            let after_insert = program.applied_callable_signatures.revision();
            if before_insert == indexed_revision && after_insert == before_insert + 1 {
                if let Err(position) = prepared_rows.binary_search(&inserted) {
                    prepared_rows.insert(position, inserted);
                }
                indexed_revision = after_insert;
            } else {
                (prepared_rows, indexed_revision) = index_rows(program);
            }
        }
        Ok(())
    }
}

impl TypedTrees {
    pub fn materialize_trait_application_arguments(
        &mut self,
        environment: &crate::source_state::trait_definition::TraitApplicationEnvironment,
    ) -> Result<Vec<crate::source_state::types::TypeReferenceHandle>, &'static str> {
        let trait_symbol = environment
            .scope(self)
            .ok_or("applied callable lost its complete source environment")?
            .declaration
            .symbol;
        let application = self.materialize_trait_application(environment)?;
        self.trait_application_arguments(trait_symbol, &application)
    }

    pub(crate) fn materialize_trait_application(
        &mut self,
        environment: &crate::source_state::trait_definition::TraitApplicationEnvironment,
    ) -> Result<CallableApplication, &'static str> {
        let scope = environment
            .scope(self)
            .ok_or("applied callable lost its complete source environment")?;
        let substitutions = scope.substitutions;
        let scope_starts = scope.substitution_scope_starts;
        let argument_owner = scope.argument_owner;
        let mut application = CallableApplication {
            trait_region_application: Some((environment.clone(), 0)),
            ..CallableApplication::default()
        };
        for (ordinal, start) in scope_starts.iter().copied().enumerate() {
            if let Some((_, preceding_frames)) = &mut application.trait_region_application {
                *preceding_frames = ordinal;
            }
            let end = scope_starts
                .get(ordinal + 1)
                .copied()
                .unwrap_or(substitutions.len());
            let group = substitutions[start..end]
                .iter()
                .map(|(parameter, argument)| {
                    let applied = application.type_reference(self, *argument)?;
                    if ordinal == 0
                        && applied.is_valid()
                        && !self
                            .type_reference_table
                            .operand_lifetime_owner(applied)
                            .is_valid()
                    {
                        self.type_reference_table
                            .retain_operand_lifetime_owner(applied, argument_owner)?;
                    }
                    Ok((*parameter, applied))
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            application.types.extend(group);
        }
        Ok(application)
    }

    fn trait_application_arguments(
        &self,
        trait_symbol: SymbolHandle,
        application: &CallableApplication,
    ) -> Result<Vec<crate::source_state::types::TypeReferenceHandle>, &'static str> {
        let parameters = self
            .trait_definition_by_symbol(trait_symbol)
            .map(|declaration| {
                self.trait_type_parameters(declaration)
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .collect::<Vec<_>>()
            })
            .ok_or("applied callable lost its declaring trait")?;
        let arguments = parameters
            .into_iter()
            .map(|symbol| {
                application
                    .types
                    .iter()
                    .rev()
                    .find(|(parameter, _)| *parameter == symbol)
                    .map(|(_, argument)| *argument)
                    .ok_or("applied callable lost a trait argument")
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(arguments)
    }
}
