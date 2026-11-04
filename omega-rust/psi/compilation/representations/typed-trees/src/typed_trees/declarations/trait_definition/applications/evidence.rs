//! Exact public requirements selected by source evidence binders.
//!
//! A child belongs to one authored binder and complete inherited application.
//! This shared read-only join supplies logical contracts, never a realization
//! or admission. Checking and occurrence queries must not reconstruct it from
//! a same-spelled template or from the selected implementation's private body.

use super::AppliedTraitScope;
use crate::{TypedTrees, machine::Machine, signature::StateSignature};
use symbols::SymbolHandle;

#[derive(Clone)]
pub struct GenericBoundRequirement<'program> {
    pub signature: &'program StateSignature,
    pub application: AppliedTraitScope<'program>,
}

impl TypedTrees {
    /// Find which declared public method a local evidence child represents.
    /// Symbol creation gives the binder's children the same order as its
    /// declared, refined and inherited methods; validate the complete list
    /// before using the selected child's position.
    pub fn evidence_requirement_declaration(
        &self,
        target: SymbolHandle,
    ) -> Result<Option<&StateSignature>, String> {
        let selected = self.symbols.get(target);
        if selected.kind != symbols::SymbolKind::State
            || !selected.parent.is_valid()
            || self.symbols.get(selected.parent).kind != symbols::SymbolKind::ConformanceParameter
        {
            return Ok(None);
        }
        let binder = selected.parent;
        let owner = self.symbols.get(binder).parent;
        let bounds = self
            .declaration_conformance_bounds(owner)
            .ok_or("evidence child lost its declaring bound owner")?;
        let mut bounds = bounds.iter().filter(|bound| bound.binder == binder);
        let bound = bounds
            .next()
            .ok_or("evidence child lost its exact authored bound")?;
        if bounds.next().is_some() {
            return Err("evidence child has multiple authored bounds".into());
        }
        let mut requirements = Vec::new();
        collect_requirement_namespace(
            self,
            bound.trait_symbol,
            &mut Vec::new(),
            &mut requirements,
        )?;
        let children = self
            .symbols
            .child_handles(binder)
            .ok_or("evidence binder lost its requirement children")?
            .collect::<Vec<_>>();
        if children.len() != requirements.len()
            || children
                .iter()
                .any(|child| self.symbols.get(*child).kind != symbols::SymbolKind::State)
        {
            return Err("evidence namespace disagrees with its declared requirements".into());
        }
        let ordinal = children
            .iter()
            .position(|child| *child == target)
            .ok_or("evidence requirement is not an exact binder child")?;
        Ok(Some(requirements[ordinal]))
    }

    /// Align a provider's separate evidence suffix with the public member's
    /// inline evidence positions. Both source orders remain intact within
    /// their own telescope; callers still check kinds, bounds and contracts.
    pub fn requirement_callable_parameters(
        &self,
        requirement: &StateSignature,
        provider: &[crate::data::TypeParameter],
    ) -> Vec<crate::data::TypeParameter> {
        use crate::data::TypeParameterKind;
        let mut ordinary = provider
            .iter()
            .filter(|parameter| parameter.kind != TypeParameterKind::Implementation);
        let mut evidence = provider
            .iter()
            .filter(|parameter| parameter.kind == TypeParameterKind::Implementation);
        let mut aligned = Vec::with_capacity(provider.len());
        for required in self.requirement_invocation_contract_parameters(requirement) {
            let selected = if required.kind == TypeParameterKind::Implementation {
                evidence.next()
            } else {
                ordinary.next()
            };
            if let Some(parameter) = selected {
                aligned.push(parameter.clone());
            }
        }
        aligned.extend(ordinary.cloned());
        aligned.extend(evidence.cloned());
        aligned
    }

    /// One logical callable telescope includes the machine's separately stored
    /// evidence suffix. These are views of existing declaration binders, never
    /// new parameters or executable argument lanes.
    pub fn machine_callable_parameters(
        &self,
        machine: &Machine,
    ) -> Vec<crate::data::TypeParameter> {
        let mut parameters = self.machine_type_parameters(machine).to_vec();
        for bound in &machine.conformance_bounds {
            if !parameters
                .iter()
                .any(|parameter| parameter.symbol == bound.binder)
            {
                parameters.push(crate::data::TypeParameter {
                    symbol: bound.binder,
                    name: bound.binder_name.clone(),
                    kind: crate::data::TypeParameterKind::Implementation,
                    ..Default::default()
                });
            }
        }
        parameters
    }

    /// The exact evidence contracts owned by a callable declaration. A closed
    /// requirement retains these contracts beside its erased executable arity.
    pub fn callable_conformance_bounds(
        &self,
        owner: SymbolHandle,
    ) -> Option<&[crate::machine::GenericConformanceBound]> {
        if let Some(application) = self.closed_requirement_application(owner) {
            return Some(&application.signature.conformance_bounds);
        }
        if let Some(machine) = self.machine_by_symbol(owner) {
            return Some(&machine.conformance_bounds);
        }
        if let Some((_, signature)) = self.trait_machine_signature(owner) {
            return Some(&signature.conformance_bounds);
        }
        self.retained_static_machine_contract(owner)
            .map(|contract| contract.signature().conformance_bounds.as_slice())
    }

    /// Read the evidence promises owned by this declaration, including callable
    /// contracts nested in another declaration's parameters.
    fn declaration_conformance_bounds(
        &self,
        owner: SymbolHandle,
    ) -> Option<&[crate::machine::GenericConformanceBound]> {
        self.callable_conformance_bounds(owner)
            .or_else(|| {
                self.trait_definition_by_symbol(owner)
                    .map(|declaration| declaration.conformance_bounds.as_slice())
            })
            .or_else(|| {
                self.unique_data_definition_by_symbol(owner)
                    .map(|declaration| declaration.conformance_bounds.as_slice())
            })
            .or_else(|| {
                self.type_aliases()
                    .iter()
                    .find(|declaration| declaration.symbol == owner)
                    .map(|declaration| declaration.conformance_bounds.as_slice())
            })
    }

    /// Prefer the invocation's retained lexical application. Its enclosing
    /// declaration is an exact fallback owner, never an ambient map search.
    pub fn callable_conformance_bound(
        &self,
        owner: SymbolHandle,
        binder: SymbolHandle,
    ) -> Option<&crate::machine::GenericConformanceBound> {
        if !binder.is_valid()
            || self.symbols.get(binder).kind != symbols::SymbolKind::ConformanceParameter
        {
            return None;
        }
        if let Some(bound) = self
            .callable_conformance_bounds(owner)
            .and_then(|bounds| bounds.iter().find(|bound| bound.binder == binder))
        {
            return Some(bound);
        }
        let parent = self.symbols.get(binder).parent;
        self.declaration_conformance_bounds(parent)
            .and_then(|bounds| bounds.iter().find(|bound| bound.binder == binder))
    }

    pub fn conformance_target_requirement<'program>(
        &'program self,
        machine: &'program Machine,
        target: SymbolHandle,
    ) -> Result<Option<GenericBoundRequirement<'program>>, String> {
        if !target.is_valid() {
            return Ok(None);
        }
        for bound in &machine.conformance_bounds {
            let binder = bound.binder;
            if self
                .symbols
                .child_handles(binder)
                .is_some_and(|mut children| children.any(|child| child == target))
            {
                return self.conformance_bound_requirement(machine, binder, target);
            }
        }
        Ok(None)
    }

    pub fn conformance_bound_requirement<'program>(
        &'program self,
        machine: &'program Machine,
        receiver: SymbolHandle,
        target: SymbolHandle,
    ) -> Result<Option<GenericBoundRequirement<'program>>, String> {
        self.evidence_bound_requirement(machine.symbol, receiver, target)
    }

    /// Resolve the namespace of an authored implementation parameter in its
    /// declaring telescope. The owner and child are exact declarations;
    /// no global name or selected private implementation supplies the bound.
    pub fn evidence_bound_requirement<'program>(
        &'program self,
        owner: SymbolHandle,
        receiver: SymbolHandle,
        target: SymbolHandle,
    ) -> Result<Option<GenericBoundRequirement<'program>>, String> {
        let Some(bounds) = self.declaration_conformance_bounds(owner) else {
            return Ok(None);
        };
        if !receiver.is_valid()
            || self.symbols.get(receiver).kind != symbols::SymbolKind::ConformanceParameter
            || !bounds.iter().any(|bound| bound.binder == receiver)
        {
            return Ok(None);
        }
        let failure = || {
            format!(
                "declaration `{}` conformance-evidence call has no unique exact requirement",
                self.symbols.name(owner)
            )
        };
        let mut bounds = bounds.iter().filter(|bound| bound.binder == receiver);
        let bound = bounds.next().ok_or_else(failure)?;
        if bounds.next().is_some() {
            return Err(format!(
                "{}: its receiver has multiple authored bounds",
                failure()
            ));
        }
        if !target.is_valid() || self.symbols.get(target).kind != symbols::SymbolKind::State {
            return Err(format!(
                "{}: its selected target is not a requirement state",
                failure()
            ));
        }
        let prepared = self.closed_requirement_application(target);
        if prepared.is_none()
            && !self
                .symbols
                .child_handles(receiver)
                .is_some_and(|mut children| children.any(|child| child == target))
        {
            return Err(format!(
                "{}: its selected target is not owned by the receiver",
                failure()
            ));
        }
        let authored_requirement = if prepared.is_none() {
            Some(
                self.evidence_requirement_declaration(target)?
                    .ok_or_else(failure)?
                    .symbol,
            )
        } else {
            None
        };
        let carrier = self
            .trait_definition_by_symbol(bound.trait_symbol)
            .ok_or_else(failure)?;
        let root = crate::trait_definition::AppliedTraitScope::new(
            self,
            carrier,
            crate::trait_definition::TraitApplicationArguments {
                lexical_owner: owner,
                enclosing_data_applications: &[],
                arguments: &bound.arguments,
                lifetimes: &bound.lifetime_arguments,
            },
        )
        .ok_or_else(|| {
            format!(
                "{}: its complete trait application cannot be represented",
                failure()
            )
        })?;
        let mut selected: Option<GenericBoundRequirement<'program>> = None;
        let mut ambiguous = false;
        root.visit(self, &mut |application| {
            for signature in self.trait_machine_signatures(application.declaration) {
                let matches = if let Some(prepared) = &prepared {
                    prepared.template == signature.symbol
                        && self.requirement_application_matches_scope(target, application)
                } else {
                    Some(signature.symbol) == authored_requirement
                };
                if !matches {
                    continue;
                }
                let signature = prepared
                    .as_ref()
                    .map_or(signature, |prepared| prepared.signature);
                if let Some(known) = &selected {
                    ambiguous |= known.signature.symbol != signature.symbol
                        || !known.application.same_application(self, application);
                    continue;
                }
                selected = Some(GenericBoundRequirement {
                    signature,
                    application: application.clone(),
                });
            }
        });
        if ambiguous {
            return Err(format!(
                "{}: reachable declarations disagree on the requirement application",
                failure()
            ));
        }
        selected.map(Some).ok_or_else(|| {
            format!(
                "{}: its selected child has no reachable authored requirement",
                failure()
            )
        })
    }
}

fn collect_requirement_namespace<'program>(
    program: &'program TypedTrees,
    trait_symbol: SymbolHandle,
    visited: &mut Vec<SymbolHandle>,
    output: &mut Vec<&'program StateSignature>,
) -> Result<(), String> {
    if visited.contains(&trait_symbol) {
        return Ok(());
    }
    let declaration = program
        .trait_definition_by_symbol(trait_symbol)
        .ok_or("evidence namespace lost a resolved trait declaration")?;
    visited.push(trait_symbol);
    output.extend(program.trait_machine_signatures(declaration));
    for parent in declaration
        .refines
        .iter()
        .chain(program.trait_requirements(declaration))
    {
        collect_requirement_namespace(program, parent.symbol, visited, output)?;
    }
    Ok(())
}
