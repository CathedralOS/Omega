//! Join each accepted service ID to its receiver application and checked source declaration.
use super::declarations::{nominal_identity, unavailable};
use super::input::PolicySource;
use diagnostics::Diagnostic;
use review_evidence::record::{PackageReviewServiceIdentity, PackageReviewTypeIdentity};
use semantic_vocabulary::ServiceId;
use source_compiler::representations::source_state::trait_definition::TraitDefinition;
use terminal_psi::TerminalModule;

#[derive(Clone)]
pub(super) struct ReviewedService {
    pub id: ServiceId,
    pub owner: symbols::SymbolHandle,
    pub identity: PackageReviewServiceIdentity,
}

/// Every published service carries its receiver application and the exact owning
/// declaration in the Terminal module. Join the latter to the checked source.
pub(super) fn identities(
    source: &source_compiler::representations::source_state::TypedTrees,
    module: &TerminalModule,
    exact_sources: &[(source_text::SourceId, [u8; 32])],
    purpose: source_provenance::DependencyScope,
) -> Result<Vec<ReviewedService>, Vec<Diagnostic>> {
    use source_compiler::representations::source_state::service_declarations::exact_identity;
    use std::collections::BTreeMap;
    if module.interfaces.services.is_empty() {
        return Ok(Vec::new());
    }
    let mut declarations = BTreeMap::<String, Vec<&TraitDefinition>>::new();
    for slot in source.traits().iter().filter(|slot| {
        slot.is_boundary
            && source
                .symbols
                .symbol_is_in_dependency_scope(slot.symbol, purpose)
    }) {
        if let Ok(id) = exact_identity(source, slot.symbol, exact_sources) {
            declarations.entry(id).or_default().push(slot);
        }
    }
    // Terminal schemas use the pair (physical declaration, receiver
    // application). A Build nomination can retain the same receiver through
    // its own declaration. Join the Product source before selecting its schema.
    let mut surfaces = BTreeMap::new();
    for schema in &module.interfaces.service_schemas {
        let key = module.identities.text(schema.declaration)
            .ok_or_else(|| unavailable("the checked schema's physical declaration"))?;
        if declarations.contains_key(key) && surfaces.insert(schema.application, schema).is_some() {
            return Err(unavailable("one Product declaration per service application"));
        }
    }
    module
        .interfaces
        .services
        .iter()
        .map(|service| {
            let schema = surfaces
                .get(&service.identity)
                .ok_or_else(|| unavailable("the published service's complete receiver schema"))?;
            let application = module
                .identities
                .text(service.identity)
                .ok_or_else(|| unavailable("a service's published application"))?;
            if schema.surface.root_application_identity() != Some(application) {
                return Err(unavailable(
                    "the same receiver on the service and its checked schema",
                ));
            }
            let key = module
                .identities
                .text(schema.declaration)
                .ok_or_else(|| unavailable("the published service's physical declaration"))?;
            let Some([owner]) = declarations.get(key).map(Vec::as_slice) else {
                return Err(unavailable(
                    "a unique original declaration of the published service",
                ));
            };
            let applied =
                !owner.type_parameters.is_empty() || !owner.lifetime_parameters.is_empty();
            Ok(ReviewedService {
                id: service.id,
                owner: owner.symbol,
                identity: PackageReviewServiceIdentity {
                    declaration: nominal_identity(source, exact_sources, owner.symbol)?,
                    application: applied.then(|| PackageReviewTypeIdentity {
                        canonical: application.to_owned(),
                    }),
                },
            })
        })
        .collect()
}

pub(super) fn reviewed_services(
    identities: &[ReviewedService],
    row: &[ServiceId],
) -> Result<Vec<PackageReviewServiceIdentity>, Vec<Diagnostic>> {
    let mut projected = row
        .iter()
        .map(|service| reviewed_service(identities, *service))
        .collect::<Result<Vec<_>, _>>()?;
    projected.sort();
    projected.dedup();
    Ok(projected)
}

pub(super) fn reviewed_service(
    identities: &[ReviewedService],
    service: ServiceId,
) -> Result<PackageReviewServiceIdentity, Vec<Diagnostic>> {
    let index = identities
        .binary_search_by_key(&service, |row| row.id)
        .map_err(|_| unavailable("a service without its original applied declaration"))?;
    Ok(identities[index].identity.clone())
}

impl<'a> PolicySource<'a> {
    /// A bare service reference carries no authority over other applications.
    pub(super) fn unapplied_service(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<PackageReviewServiceIdentity, Vec<Diagnostic>> {
        let slot = self
            .typed()
            .trait_definition_by_symbol(symbol)
            .filter(|slot| {
                slot.is_boundary
                    && slot.type_parameters.is_empty()
                    && slot.lifetime_parameters.is_empty()
            })
            .ok_or_else(|| {
                unavailable("a boundary service's required static and lifetime application")
            })?;
        Ok(self.nominal(slot.symbol)?.into())
    }
    pub(super) fn reviewed_requirement_service(
        &self,
        service: &source_compiler::publication::RequirementService,
    ) -> Result<PackageReviewServiceIdentity, Vec<Diagnostic>> {
        Ok(PackageReviewServiceIdentity {
            declaration: self.nominal(service.declaration)?,
            application: service
                .application
                .as_ref()
                .map(|canonical| PackageReviewTypeIdentity {
                    canonical: canonical.clone(),
                }),
        })
    }
    pub(super) fn effect_identity(
        &self,
        scope: &source_compiler::representations::source_state::trait_definition::AppliedTraitScope<
            '_,
        >,
    ) -> Result<PackageReviewServiceIdentity, Vec<Diagnostic>> {
        let declaration = self.nominal(scope.declaration.symbol)?;
        if scope.declaration.type_parameters.is_empty()
            && scope.declaration.lifetime_parameters.is_empty()
        {
            return Ok(declaration.into());
        }
        let canonical = scope
            .portable_application_identity(self.typed())
            .and_then(|identity| self.typed().type_identity_text(identity))
            .ok_or_else(|| unavailable("a service's normalized static and lifetime inputs"))?;
        Ok(PackageReviewServiceIdentity {
            declaration,
            application: Some(PackageReviewTypeIdentity { canonical }),
        })
    }
    pub(super) fn schema_effect_scope(
        &self,
        symbol: symbols::SymbolHandle,
        schema: &build_providers::plans::ServiceSchema,
    ) -> Result<
        source_compiler::representations::source_state::trait_definition::AppliedTraitScope<'a>,
        Vec<Diagnostic>,
    > {
        use source_compiler::representations::source_state::trait_definition::AppliedTraitScope;
        let source = self.typed();
        let declaration = source
            .trait_definition_by_symbol(symbol)
            .filter(|slot| slot.is_boundary)
            .ok_or_else(|| unavailable("the owned declaration of a selected service"))?;
        if declaration.type_parameters.is_empty() && declaration.lifetime_parameters.is_empty() {
            return Ok(AppliedTraitScope::abstract_scope(declaration));
        }
        let requested = schema
            .root_application_identity()
            .ok_or_else(|| unavailable("a selected service's complete receiver application"))?;
        for root in source.applied_trait_roots() {
            let mut selected = None;
            root.try_visit(source, &mut |scope| {
                if scope.declaration.symbol == symbol
                    && scope
                        .portable_application_identity(source)
                        .and_then(|id| source.type_identity_text(id))
                        .as_deref()
                        == Some(requested)
                {
                    selected = Some(scope.clone());
                }
                Ok(())
            })
            .map_err(unavailable)?;
            if let Some(scope) = selected {
                return Ok(scope);
            }
        }
        Err(unavailable(
            "the typed source of the selected service application",
        ))
    }
}
