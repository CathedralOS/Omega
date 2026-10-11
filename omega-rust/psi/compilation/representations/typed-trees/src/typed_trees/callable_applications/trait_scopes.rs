//! Prepare and rejoin exact logical trait applications for both named maps and
//! machine satisfaction edges. Consumers borrow these signatures after freezing;
//! they neither mutate declarations nor infer an authored fixed type argument.

use super::{AppliedCallableOwner, AppliedCallableSignature, CallableApplication};
use crate::{TypedTrees, signature, trait_definition::AppliedTraitScope};
use symbols::SymbolHandle;

impl TypedTrees {
    /// Applied logical signature for one exact declaring scope. Its complete
    /// trait application remains separate from requirement authority.
    pub fn applied_callable_signature(
        &self,
        application: &AppliedTraitScope<'_>,
        requirement: SymbolHandle,
    ) -> Option<&signature::StateSignature> {
        self.applied_callable_signatures
            .iter()
            .find_map(|(_, prepared)| {
                let AppliedCallableOwner::Trait {
                    declaration,
                    arguments,
                } = &prepared.owner
                else {
                    return None;
                };
                if *declaration != application.declaration.symbol
                    || prepared.signature.symbol != requirement
                {
                    return None;
                }
                let arguments = self.type_reference_table.type_reference_handles(*arguments);
                let prepared_scope =
                    AppliedTraitScope::new(self, application.declaration, arguments)?;
                application
                    .same_application(self, &prepared_scope)
                    .then_some(&prepared.signature)
            })
    }

    /// Prepare reachable declaring scopes once, including every inherited path.
    /// An application owns logical contracts, not executable realizations;
    /// constructing this row confers no conformance or proof authority.
    pub fn prepare_applied_callable_signatures(&mut self) -> Result<(), &'static str> {
        let roots = self
            .machines()
            .iter()
            .flat_map(|machine| self.machine_trait_conformances(machine))
            .map(|edge| (edge.symbol, edge.arguments))
            .chain(
                self.conformances()
                    .iter()
                    .map(|edge| (edge.trait_symbol, edge.arguments)),
            )
            .chain(
                self.traits()
                    .iter()
                    .filter(|declaration| declaration.type_parameters.is_empty())
                    .map(|declaration| (declaration.symbol, arena::HandleSpan::empty())),
            )
            .collect::<Vec<_>>();
        let mut pending = Vec::new();
        for (trait_symbol, arguments) in roots {
            let Some(declaration) = self.trait_definition_by_symbol(trait_symbol) else {
                continue;
            };
            let Some(scope) = AppliedTraitScope::new(
                self,
                declaration,
                self.type_reference_table.type_reference_handles(arguments),
            ) else {
                continue;
            };
            scope.visit(self, &mut |scope| {
                for signature in self.trait_machine_signatures(scope.declaration) {
                    // A signature without local binders needs no cloned
                    // telescope. Existing scoped judgments already apply its
                    // ordinary parameters, result and contracts read-only.
                    if signature.type_parameters.is_empty() {
                        continue;
                    }
                    pending.push((
                        scope.declaration.symbol,
                        signature.symbol,
                        scope.substitutions.clone(),
                        scope.substitution_scope_starts.clone(),
                    ));
                }
            });
        }
        for (trait_symbol, requirement, substitutions, starts) in pending {
            let Some(declaration) = self.trait_definition_by_symbol(trait_symbol) else {
                continue;
            };
            let scope = AppliedTraitScope {
                declaration,
                substitutions: substitutions.clone(),
                substitution_scope_starts: starts.clone(),
            };
            if self
                .applied_callable_signature(&scope, requirement)
                .is_some()
            {
                continue;
            }
            let mut application = CallableApplication::default();
            for (ordinal, start) in starts.iter().copied().enumerate() {
                let end = starts
                    .get(ordinal + 1)
                    .copied()
                    .unwrap_or(substitutions.len());
                let group = substitutions[start..end]
                    .iter()
                    .map(|(parameter, argument)| {
                        Ok((*parameter, application.type_reference(self, *argument)?))
                    })
                    .collect::<Result<Vec<_>, &'static str>>()?;
                application.types.extend(group);
            }
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
            let Some(applied) = CallableApplication::default().nominal(
                self,
                trait_symbol,
                requirement,
                &arguments,
            )?
            else {
                continue;
            };
            let arguments = self
                .type_reference_table
                .insert_type_reference_handles(arguments);
            self.applied_callable_signatures
                .insert(AppliedCallableSignature {
                    owner: AppliedCallableOwner::Trait {
                        declaration: trait_symbol,
                        arguments,
                    },
                    signature: applied,
                });
        }
        Ok(())
    }
}
