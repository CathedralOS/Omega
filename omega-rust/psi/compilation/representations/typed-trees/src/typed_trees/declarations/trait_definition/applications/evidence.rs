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
        self.callable_conformance_bounds(parent)
            .or_else(|| {
                self.trait_definition_by_symbol(parent)
                    .map(|declaration| declaration.conformance_bounds.as_slice())
            })
            .or_else(|| {
                self.unique_data_definition_by_symbol(parent)
                    .map(|declaration| declaration.conformance_bounds.as_slice())
            })
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
        if !receiver.is_valid()
            || self.symbols.get(receiver).kind != symbols::SymbolKind::ConformanceParameter
            || !machine
                .conformance_bounds
                .iter()
                .any(|bound| bound.binder == receiver)
        {
            return Ok(None);
        }
        let failure = || {
            format!(
                "machine `{}` conformance-evidence call has no unique exact requirement",
                machine.name
            )
        };
        let mut bounds = machine
            .conformance_bounds
            .iter()
            .filter(|bound| bound.binder == receiver);
        let bound = bounds
            .next()
            .ok_or_else(|| format!("{}: its receiver has no authored bound", failure()))?;
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
        let carrier = self
            .trait_definition_by_symbol(bound.trait_symbol)
            .ok_or_else(failure)?;
        let root = crate::trait_definition::AppliedTraitScope::new(
            self,
            carrier,
            crate::trait_definition::TraitApplicationArguments {
                lexical_owner: machine.symbol,
                enclosing_data_applications: &[],
                arguments: &bound.arguments,
                lifetimes: &[],
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
                    signature.name.as_str() == self.symbols.name(target)
                };
                if !matches {
                    continue;
                }
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
                "{}: its selected name has no reachable authored requirement",
                failure()
            )
        })
    }
}
