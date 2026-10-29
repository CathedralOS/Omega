//! Keep the supplying declaration's regions separate from receiving binders.
//! Explicit trait lifetime arguments and regions inside type operands use the
//! same supplying coordinates, so repeated arguments retain their equality.

use super::instantiation;
use crate::capture::PackageReviewInput;
use crate::capture::calling::rejected;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::name::Identifier;
use typed_trees::trait_definition::{AppliedTraitScope, TraitDefinition};
use typed_trees::types::TypeReferenceHandle;

pub(super) struct ApplicationScope {
    pub binders: Vec<Identifier>,
    pub trait_arguments: Vec<Identifier>,
    pub supplying_count: usize,
    owner: SymbolHandle,
    regions: Option<
        Vec<(
            typed_trees::type_identity::LifetimeParameterIdentity,
            Identifier,
        )>,
    >,
}

impl ApplicationScope {
    pub fn new(
        compilation: &PackageReviewInput<'_>,
        root: &TraitDefinition,
        arguments: &[TypeReferenceHandle],
        owner: SymbolHandle,
    ) -> Result<Self, Vec<Diagnostic>> {
        if owner == root.symbol {
            return Ok(Self {
                binders: root.lifetime_parameters.clone(),
                trait_arguments: root.lifetime_parameters.clone(),
                supplying_count: 0,
                owner,
                regions: None,
            });
        }
        let conformance = compilation
            .conformance_by_symbol(owner)
            .ok_or_else(|| rejected("calling application lost its supplying conformance"))?;
        if conformance.trait_symbol != root.symbol
            || compilation
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                != arguments
            || AppliedTraitScope::from_conformance(compilation, conformance).is_none()
        {
            return Err(rejected(
                "calling application differs from its supplying conformance",
            ));
        }
        let supplying = conformance
            .lifetime_parameters
            .iter()
            .enumerate()
            .map(|(ordinal, _)| {
                Identifier::generated(format!("$calling-application-region-{ordinal}"))
            })
            .collect::<Vec<_>>();
        let trait_arguments = conformance
            .trait_lifetime_arguments
            .iter()
            .map(|ordinal| {
                supplying
                    .get(*ordinal as usize)
                    .cloned()
                    .ok_or_else(|| rejected("calling trait lifetime lost its supplying coordinate"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let supplying_count = supplying.len();
        let mut binders = root.lifetime_parameters.clone();
        binders.extend(supplying);
        Ok(Self {
            binders,
            trait_arguments,
            supplying_count,
            owner,
            regions: None,
        })
    }

    pub fn requirement(
        compilation: &PackageReviewInput<'_>,
        projected: &TypedTrees,
        root: &TraitDefinition,
        application: &AppliedTraitScope<'_>,
        arguments: &[TypeReferenceHandle],
        local_arguments: &[TypeReferenceHandle],
    ) -> Result<Self, Vec<Diagnostic>> {
        use typed_trees::references::TypedReferences;
        struct Owners<'a> {
            program: &'a TypedTrees,
            owners: Vec<SymbolHandle>,
            seen: Vec<TypeReferenceHandle>,
        }
        impl Extend<SymbolHandle> for Owners<'_> {
            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
        }
        impl TypedReferences for Owners<'_> {
            fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
                if self.seen.contains(&reference) {
                    return false;
                }
                self.seen.push(reference);
                let owner = self
                    .program
                    .type_reference_table
                    .operand_lifetime_owner(reference);
                if owner.is_valid() && !self.owners.contains(&owner) {
                    self.owners.push(owner);
                }
                true
            }
        }
        let mut owners = Owners {
            program: projected,
            owners: vec![application.argument_owner],
            seen: Vec::new(),
        };
        for argument in arguments.iter().chain(local_arguments) {
            typed_trees::references::visit_type_reference(projected, &mut owners, *argument);
        }
        let mut regions = Vec::new();
        let mut binders = root.lifetime_parameters.clone();
        let mut supplying_count = 0;
        for owner in owners.owners {
            let mut owner = compilation.lifetime_declaration_owner(owner);
            let mut active = Vec::new();
            while owner.is_valid() {
                if active.contains(&owner) {
                    return Err(rejected("native supplying lifetime scope is cyclic"));
                }
                active.push(owner);
                let names = declaration_lifetimes(compilation, owner);
                for (ordinal, _) in names.iter().enumerate() {
                    let identity = typed_trees::type_identity::LifetimeParameterIdentity {
                        declaration: owner,
                        ordinal,
                    };
                    if regions.iter().any(|(existing, _)| *existing == identity) {
                        continue;
                    }
                    let normalized = if owner == root.symbol {
                        root.lifetime_parameters[ordinal].clone()
                    } else {
                        let name = Identifier::generated(format!(
                            "$calling-application-region-{supplying_count}"
                        ));
                        supplying_count += 1;
                        binders.push(name.clone());
                        name
                    };
                    regions.push((identity, normalized));
                }
                owner = compilation
                    .lifetime_declaration_owner(compilation.declaration_scope_parent(owner));
            }
        }
        let trait_arguments = application.with_identity_scopes(compilation, |scopes| {
            root.lifetime_parameters.iter().map(|name| {
                let identity = typed_trees::type_identity::applied_lifetime_parameter_identity_with_supplier(
                    compilation, root.symbol, application.argument_owner, name.as_str(), scopes,
                ).map(|(identity, _)| identity)
                .ok_or_else(|| rejected("native trait lifetime lost its exact supplying identity"))?;
                regions.iter().find(|(candidate, _)| *candidate == identity).map(|(_, name)| name.clone())
                    .ok_or_else(|| rejected("native trait lifetime has no admitted supplying coordinate"))
            }).collect::<Result<Vec<_>, Vec<Diagnostic>>>()
        })?;
        Ok(Self {
            binders,
            trait_arguments,
            supplying_count,
            owner: application.argument_owner,
            regions: Some(regions),
        })
    }

    pub fn supplied_region(
        &self,
        compilation: &PackageReviewInput<'_>,
        owner: SymbolHandle,
        name: &Identifier,
    ) -> Result<Identifier, Vec<Diagnostic>> {
        let identity = typed_trees::type_identity::lifetime_parameter_identity(
            compilation,
            owner,
            name.as_str(),
        )
        .ok_or_else(|| rejected("native member lifetime has no exact supplying identity"))?;
        self.regions
            .as_ref()
            .and_then(|regions| regions.iter().find(|(candidate, _)| *candidate == identity))
            .map(|(_, name)| name.clone())
            .ok_or_else(|| rejected("native member lifetime has no admitted supplying coordinate"))
    }

    pub fn instantiate_argument(
        &self,
        compilation: &PackageReviewInput<'_>,
        scratch: &mut TypedTrees,
        reference: TypeReferenceHandle,
    ) -> Result<TypeReferenceHandle, Vec<Diagnostic>> {
        if let Some(regions) = &self.regions {
            return instantiation::instantiate_captured_application(
                scratch,
                compilation,
                reference,
                self.owner,
                regions,
            );
        }
        let offset = self.binders.len() - self.supplying_count;
        let binders = if self.supplying_count == 0 {
            self.binders.as_slice()
        } else {
            &self.binders[offset..]
        };
        instantiation::instantiate_application(scratch, compilation, reference, self.owner, binders)
    }

    pub fn coordinate(&self, name: &Identifier) -> Result<u32, Vec<Diagnostic>> {
        self.binders
            .iter()
            .position(|binder| binder == name)
            .and_then(|ordinal| u32::try_from(ordinal).ok())
            .ok_or_else(|| rejected("calling lifetime has no containing telescope coordinate"))
    }
}

fn declaration_lifetimes(
    compilation: &PackageReviewInput<'_>,
    owner: SymbolHandle,
) -> Vec<Identifier> {
    compilation
        .machine_by_symbol(owner)
        .map(|owner| owner.lifetime_parameters.clone())
        .or_else(|| {
            compilation
                .conformance_by_symbol(owner)
                .map(|owner| owner.lifetime_parameters.clone())
        })
        .or_else(|| {
            compilation
                .trait_definition_by_symbol(owner)
                .map(|owner| owner.lifetime_parameters.clone())
        })
        .or_else(|| {
            compilation
                .data_definition_by_symbol(owner)
                .map(|owner| owner.lifetime_parameters.clone())
        })
        .or_else(|| {
            compilation
                .trait_machine_signature(owner)
                .map(|(_, signature)| signature.lifetime_parameters.clone())
        })
        .unwrap_or_default()
}
