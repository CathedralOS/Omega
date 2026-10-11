//! Prepare and rejoin exact logical trait applications for both named maps and
//! machine satisfaction edges. Consumers borrow these signatures after freezing;
//! they neither mutate declarations nor infer an authored fixed type argument.
//! The cache key retains regions even though carrier preparation substitutes
//! Type operands only. Complete region refinement must read the original
//! signature with its borrowed application scopes: flattening a mixed captured
//! and fresh lifetime telescope can recapture same-spelled operands.

use super::{AppliedCallableOwner, AppliedCallableSignature, CallableApplication};
use crate::{TypedTrees, signature, trait_definition::AppliedTraitScope};
use symbols::SymbolHandle;

impl TypedTrees {
    /// Retained rows must keep both the handle and the key it names. Unused
    /// query keys may grow independently after cloning; they are not storage
    /// obligations. Compare only keys referenced by the retained signatures.
    pub(crate) fn retains_trait_application_identities(&self, base: &Self) -> bool {
        base.applied_callable_signatures
            .iter()
            .all(|(_, prepared)| {
                let AppliedCallableOwner::Trait { application, .. } = prepared.owner else {
                    return true;
                };
                let Some(key) = base.type_identity_text(application) else {
                    return false;
                };
                self.type_identities
                    .with(|table| table.text(application) == Some(key.as_str()))
            })
    }

    /// Applied logical signature for one exact declaring scope. Its complete
    /// trait application remains separate from requirement authority.
    pub fn applied_callable_signature(
        &self,
        application: &AppliedTraitScope<'_>,
        requirement: SymbolHandle,
    ) -> Option<&signature::StateSignature> {
        let identity = application.application_identity(self)?;
        self.applied_callable_signatures
            .iter()
            .find_map(|(_, prepared)| {
                let AppliedCallableOwner::Trait {
                    declaration,
                    application: prepared_identity,
                } = &prepared.owner
                else {
                    return None;
                };
                if *declaration != application.declaration.symbol
                    || *prepared_identity != identity
                    || prepared.signature.symbol != requirement
                {
                    return None;
                }
                Some(&prepared.signature)
            })
    }

    /// Prepare reachable declaring scopes once, including every inherited path.
    /// An application owns logical contracts, not executable realizations;
    /// constructing this row confers no conformance or proof authority.
    pub fn prepare_applied_callable_signatures(&mut self) -> Result<(), &'static str> {
        let roots = self.applied_trait_roots();
        let mut pending = Vec::new();
        for scope in roots {
            scope.visit(self, &mut |scope| {
                for signature in self.trait_machine_signatures(scope.declaration) {
                    // A completely monomorphic declaration needs no view.
                    // Outer arguments still substitute contracts when the
                    // requirement has no local binders, including phantom ones.
                    if signature.type_parameters.is_empty()
                        && scope.substitutions.is_empty()
                        && scope.declaration.lifetime_parameters.is_empty()
                    {
                        continue;
                    }
                    let Some(identity) = scope.application_identity(self) else {
                        continue;
                    };
                    pending.push((
                        scope.declaration.symbol,
                        identity,
                        signature.symbol,
                        scope.substitutions.clone(),
                        scope.substitution_scope_starts.clone(),
                        scope.argument_owner,
                    ));
                }
            });
        }
        for (trait_symbol, identity, requirement, substitutions, starts, argument_owner) in pending
        {
            if self
                .applied_callable_signatures.iter().any(|(_, prepared)| {
                    prepared.signature.symbol == requirement
                        && matches!(prepared.owner, AppliedCallableOwner::Trait { application, .. } if application == identity)
                })
            {
                continue;
            }
            let application =
                self.materialize_trait_application(&substitutions, &starts, argument_owner)?;
            let arguments = self.trait_application_arguments(trait_symbol, &application)?;
            // Ancestor substitutions resolve the final operands, but are not
            // the declaring signature's telescope. Reusing that scratch scope
            // would capture unrelated binders while copying local contracts.
            let Some(applied) = CallableApplication::default().nominal(
                self,
                trait_symbol,
                requirement,
                &arguments,
            )?
            else {
                continue;
            };
            self.applied_callable_signatures
                .insert(AppliedCallableSignature {
                    owner: AppliedCallableOwner::Trait {
                        declaration: trait_symbol,
                        application: identity,
                    },
                    signature: applied,
                });
        }
        Ok(())
    }

    /// Materialize each simultaneous telescope in its outer scopes. This shared
    /// derivation serves symbolic checking views and closed executable instances;
    /// it does not infer missing operands from a provider or runtime carrier.
    pub fn materialize_trait_application_arguments(
        &mut self,
        trait_symbol: SymbolHandle,
        substitutions: &[(SymbolHandle, crate::types::TypeReferenceHandle)],
        scope_starts: &[usize],
        argument_owner: SymbolHandle,
    ) -> Result<Vec<crate::types::TypeReferenceHandle>, &'static str> {
        let application =
            self.materialize_trait_application(substitutions, scope_starts, argument_owner)?;
        self.trait_application_arguments(trait_symbol, &application)
    }

    fn materialize_trait_application(
        &mut self,
        substitutions: &[(SymbolHandle, crate::types::TypeReferenceHandle)],
        scope_starts: &[usize],
        argument_owner: SymbolHandle,
    ) -> Result<CallableApplication, &'static str> {
        let mut application = CallableApplication::default();
        for (ordinal, start) in scope_starts.iter().copied().enumerate() {
            let end = scope_starts
                .get(ordinal + 1)
                .copied()
                .unwrap_or(substitutions.len());
            let group = substitutions[start..end]
                .iter()
                .map(|(parameter, argument)| {
                    let applied = application.type_reference(self, *argument)?;
                    // Only copied supplying operands acquire this custody.
                    // Original declaration nodes remain lexical, and compound
                    // inherited arguments may contain differently owned children.
                    if ordinal == 0 && applied.is_valid() {
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
    ) -> Result<Vec<crate::types::TypeReferenceHandle>, &'static str> {
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
